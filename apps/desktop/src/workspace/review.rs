//! Revision-bound Markdown proposals and private checkpoint recovery.

use super::*;
use crate::markdown::SourceEdit;
use std::collections::BTreeSet;

mod locations;

#[cfg(test)]
mod tests;

const HISTORY_LIMIT: usize = 20;
const JOURNAL_VERSION: u32 = 1;

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Proposal {
    pub operation_id: String,
    pub path: String,
    pub base_revision: String,
    pub profile: u32,
    pub hunks: Vec<Hunk>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Hunk {
    pub id: String,
    pub edit: SourceEdit,
}

impl Proposal {
    fn batch(&self, hunks: impl Iterator<Item = SourceEdit>) -> SourceEdits {
        SourceEdits {
            profile: self.profile,
            edits: hunks.collect(),
        }
    }

    fn select(&self, source: &str, selected: &[String]) -> io::Result<(String, Vec<Hunk>)> {
        let selected_ids: BTreeSet<_> = selected.iter().collect();
        if self.operation_id.trim().is_empty()
            || selected.is_empty()
            || selected_ids.len() != selected.len()
        {
            return Err(invalid(
                "Choose distinct hunks from an identified proposal.",
            ));
        }
        let mut ids = BTreeSet::new();
        for hunk in &self.hunks {
            if hunk.id.trim().is_empty()
                || !ids.insert(&hunk.id)
                || hunk.edit.expected == hunk.edit.replacement
            {
                return Err(invalid(
                    "A proposal needs distinct hunk IDs and effective edits.",
                ));
            }
        }
        if !selected_ids.is_subset(&ids) {
            return Err(invalid("The proposal does not contain the selected hunk."));
        }
        self.batch(self.hunks.iter().map(|hunk| hunk.edit.clone()))
            .apply(source, LIMIT as usize)?;
        let output = self
            .batch(
                self.hunks
                    .iter()
                    .filter(|hunk| selected_ids.contains(&hunk.id))
                    .map(|hunk| hunk.edit.clone()),
            )
            .apply(source, LIMIT as usize)?;
        if output == source {
            return Err(invalid("The selected hunks have no combined effect."));
        }

        let mut remaining = Vec::new();
        let (mut original_cursor, mut updated_cursor) = (0, 0);
        for hunk in &self.hunks {
            let edit = &hunk.edit;
            let from = updated_cursor + (edit.from - original_cursor);
            if selected_ids.contains(&hunk.id) {
                original_cursor = edit.to;
                updated_cursor = from + edit.replacement.len();
            } else {
                remaining.push(Hunk {
                    id: hunk.id.clone(),
                    edit: SourceEdit {
                        from,
                        to: from + edit.expected.len(),
                        ..edit.clone()
                    },
                });
            }
        }
        self.batch(remaining.iter().map(|hunk| hunk.edit.clone()))
            .apply(&output, LIMIT as usize)?;
        Ok((output, remaining))
    }
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Acceptance {
    Accepted {
        revision: String,
        checkpoint_id: u64,
        remaining: Option<Proposal>,
    },
    Conflict {
        disk: Document,
    },
}

#[derive(Serialize, Deserialize)]
struct Head {
    revision: String,
    chain: u64,
}

#[derive(Serialize, Deserialize)]
struct Checkpoint {
    id: u64,
    chain: Option<u64>,
    operation_id: String,
    hunk_ids: Vec<String>,
    before: String,
}

#[derive(Serialize, Deserialize)]
struct PendingReview {
    id: u64,
    before: String,
    after: String,
    staged_name: PathBuf,
    action: ReviewAction,
}

impl PendingReview {
    fn discard_stage(&self, locked: &LockedTarget) -> io::Result<()> {
        let Some(name) = self.staged_name.file_name() else {
            return Err(invalid("Review journal has an invalid staging path."));
        };
        if self.staged_name.as_os_str() != name
            || !name.as_encoded_bytes().starts_with(b".offish-save-")
        {
            return Err(invalid("Review journal has an invalid staging path."));
        }
        let stage = descriptor(&locked.directory).join(name);
        match snapshot(&stage) {
            Ok((metadata, bytes)) if revision(&metadata, &bytes) == self.after => {
                fs::remove_file(stage)?;
                locked.directory.sync_all()
            }
            Ok(_) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }
}

#[derive(Serialize, Deserialize)]
enum ReviewAction {
    Accept {
        operation_id: String,
        hunk_ids: Vec<String>,
        chain: u64,
    },
    Restore {
        checkpoint_id: u64,
        chain: u64,
    },
}

#[derive(Serialize, Deserialize)]
struct Journal {
    version: u32,
    next_id: u64,
    head: Option<Head>,
    checkpoints: Vec<Checkpoint>,
    pending: Option<PendingReview>,
}

impl Default for Journal {
    fn default() -> Self {
        Self {
            version: JOURNAL_VERSION,
            next_id: 1,
            head: None,
            checkpoints: Vec::new(),
            pending: None,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewRecovery {
    Clean,
    OriginalOnDisk,
    Committed,
    Conflict,
}

#[derive(Debug, Serialize)]
pub struct ReviewHistory {
    pub recovery: ReviewRecovery,
    pub interrupted_id: Option<u64>,
    pub checkpoints: Vec<CheckpointInfo>,
}

#[derive(Debug, Serialize)]
pub struct CheckpointInfo {
    pub id: u64,
    pub operation_id: String,
    pub hunk_ids: Vec<String>,
    pub restorable: bool,
}

struct Store(File);

impl Store {
    fn open(private: &File, name: &str, create: bool) -> io::Result<Option<Self>> {
        if name.is_empty()
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-'))
        {
            return Err(invalid("Invalid checkpoint store name."));
        }
        let path = descriptor(private).join(format!("review-{name}"));
        if create {
            match DirBuilder::new().mode(0o700).create(&path) {
                Ok(()) => private.sync_all()?,
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
        match open(&path, true) {
            Ok(directory) => {
                let metadata = directory.metadata()?;
                if metadata.uid() != unsafe { libc::geteuid() } || metadata.mode() & 0o077 != 0 {
                    return Err(invalid(
                        "Review storage must be private to the current user.",
                    ));
                }
                Ok(Some(Self(directory)))
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound && !create => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn replace(
        &self,
        name: &str,
        write: impl FnOnce(&mut File) -> io::Result<()>,
    ) -> io::Result<()> {
        write_private(&self.0, name, write)
    }

    fn load(&self) -> io::Result<Journal> {
        match open(&descriptor(&self.0).join("state.json"), false) {
            Ok(file) => {
                let journal: Journal = serde_json::from_reader(file)?;
                if journal.version != JOURNAL_VERSION {
                    return Err(invalid("This review journal uses an unsupported version."));
                }
                Ok(journal)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Journal::default()),
            Err(error) => Err(error),
        }
    }

    fn write(&self, journal: &Journal) -> io::Result<()> {
        self.replace("state.json", |file| {
            Ok(serde_json::to_writer(file, journal)?)
        })
    }

    fn begin(
        &self,
        state: &mut Journal,
        pending: PendingReview,
        preimage: &[u8],
    ) -> io::Result<()> {
        state.next_id = pending
            .id
            .checked_add(1)
            .ok_or_else(|| invalid("Checkpoint ID limit reached."))?;
        self.replace(&format!("{}.md", pending.id), |file| {
            file.write_all(preimage)
        })?;
        state.pending = Some(pending);
        self.write(state)
    }

    fn prune(&self, state: &Journal) -> io::Result<()> {
        let mut removed = false;
        for entry in fs::read_dir(descriptor(&self.0))? {
            let entry = entry?;
            let name = entry.file_name();
            let Some(id) = name
                .to_str()
                .and_then(|name| name.strip_suffix(".md"))
                .and_then(|id| id.parse::<u64>().ok())
            else {
                continue;
            };
            if state
                .pending
                .as_ref()
                .is_some_and(|pending| pending.id == id)
                || state
                    .checkpoints
                    .iter()
                    .any(|checkpoint| checkpoint.id == id)
            {
                continue;
            }
            fs::remove_file(entry.path())?;
            removed = true;
        }
        if removed {
            self.0.sync_all()?;
        }
        Ok(())
    }

    fn preimage(&self, id: u64, before: &str) -> io::Result<String> {
        let (_, bytes) = snapshot(&descriptor(&self.0).join(format!("{id}.md")))?;
        let digest = checksum(&bytes);
        if before.rsplit(':').next() != Some(&digest) {
            return Err(invalid(
                "The checkpoint preimage failed its integrity check.",
            ));
        }
        String::from_utf8(bytes).map_err(io::Error::other)
    }

    fn reconcile(
        &self,
        locked: &LockedTarget,
        state: &mut Journal,
        disk: Option<&str>,
    ) -> io::Result<ReviewRecovery> {
        if let Some(pending) = state.pending.as_ref() {
            if disk == Some(pending.after.as_str()) {
                // Publish history only after the replaced document entry is durable.
                locked.directory.sync_all()?;
                let pending = state.pending.take().expect("pending review");
                let chain = match pending.action {
                    ReviewAction::Accept {
                        operation_id,
                        hunk_ids,
                        chain,
                    } => {
                        state.keep(Checkpoint {
                            id: pending.id,
                            chain: Some(chain),
                            operation_id,
                            hunk_ids,
                            before: pending.before,
                        });
                        chain
                    }
                    ReviewAction::Restore {
                        checkpoint_id,
                        chain,
                    } => {
                        state
                            .checkpoints
                            .retain(|checkpoint| checkpoint.id < checkpoint_id);
                        chain
                    }
                };
                state.head = Some(Head {
                    revision: pending.after,
                    chain,
                });
                self.write(state)?;
                self.prune(state)?;
                return Ok(ReviewRecovery::Committed);
            }
            if disk == Some(pending.before.as_str()) {
                pending.discard_stage(locked)?;
                state.pending = None;
                self.write(state)?;
                self.prune(state)?;
                return Ok(ReviewRecovery::OriginalOnDisk);
            }
            return Ok(ReviewRecovery::Conflict);
        }
        if state
            .head
            .as_ref()
            .is_some_and(|head| disk != Some(head.revision.as_str()))
        {
            state.head = None;
            self.write(state)?;
        }
        self.prune(state)?;
        Ok(ReviewRecovery::Clean)
    }
}

fn write_private(
    directory: &File,
    name: &str,
    write: impl FnOnce(&mut File) -> io::Result<()>,
) -> io::Result<()> {
    let (mut file, stage) = stage_file(directory)?;
    write(&mut file)?;
    file.sync_all()?;
    stage.replace(&descriptor(directory).join(name))?;
    directory.sync_all()
}

impl Journal {
    fn keep(&mut self, checkpoint: Checkpoint) {
        self.checkpoints.push(checkpoint);
        if self.checkpoints.len() > HISTORY_LIMIT {
            self.checkpoints.remove(0);
        }
    }
}

impl Workspace {
    fn review_store(
        &self,
        locked: &LockedTarget,
        private: &File,
        create: bool,
    ) -> io::Result<Option<Store>> {
        let name = self.journal_name(&locked.directory, &locked.name)?;
        locations::lookup(private, locked, &name, create)
    }

    pub(super) fn check_review_pending(
        &self,
        locked: &LockedTarget,
        private: &File,
        base: &str,
    ) -> io::Result<()> {
        if let Some(store) = self.review_store(locked, private, false)? {
            let mut state = store.load()?;
            require_resolved(store.reconcile(locked, &mut state, Some(base))?)?;
        }
        Ok(())
    }

    pub fn accept_proposal(
        &self,
        proposal: &Proposal,
        selected: &[String],
    ) -> io::Result<Acceptance> {
        self.accept_with_hook(proposal, selected, |_| Ok(()))
    }

    fn accept_with_hook(
        &self,
        proposal: &Proposal,
        selected: &[String],
        hook: impl FnMut(SaveStep) -> io::Result<()>,
    ) -> io::Result<Acceptance> {
        let locked = self.locked_markdown(&proposal.path)?;
        let captured = ReviewSnapshot::read(&locked)?;
        if captured.revision != proposal.base_revision {
            return Ok(Acceptance::Conflict {
                disk: captured.into_document(),
            });
        }
        let source = std::str::from_utf8(&captured.bytes).map_err(io::Error::other)?;
        let (output, remaining) = proposal.select(source, selected)?;
        let private = self.review_private()?;
        let store = self
            .review_store(&locked, &private, true)?
            .expect("created review store");
        let mut state = store.load()?;
        require_resolved(store.reconcile(&locked, &mut state, Some(&captured.revision))?)?;
        self.clear_committed_journal(&locked.directory, &locked.name, &private)?;
        let checkpoint_id = state.next_id;
        let chain = state.head.as_ref().map_or(checkpoint_id, |head| head.chain);
        let action = ReviewAction::Accept {
            operation_id: proposal.operation_id.clone(),
            hunk_ids: selected.into(),
            chain,
        };
        match replace_with_checkpoint(
            &locked, &store, &mut state, &captured, &output, action, hook,
        )? {
            Save::Conflict { disk } => Ok(Acceptance::Conflict { disk }),
            Save::Saved { revision } => Ok(Acceptance::Accepted {
                checkpoint_id,
                remaining: (!remaining.is_empty()).then(|| Proposal {
                    operation_id: proposal.operation_id.clone(),
                    path: proposal.path.clone(),
                    base_revision: revision.clone(),
                    profile: proposal.profile,
                    hunks: remaining,
                }),
                revision,
            }),
        }
    }

    pub fn review_history(&self, path: &str) -> io::Result<ReviewHistory> {
        let locked = self.locked_markdown(path)?;
        let private = self.review_private()?;
        let Some(store) = self.review_store(&locked, &private, false)? else {
            return Ok(ReviewHistory {
                recovery: ReviewRecovery::Clean,
                interrupted_id: None,
                checkpoints: Vec::new(),
            });
        };
        let disk = disk_revision(&locked)?;
        let mut state = store.load()?;
        let recovery = store.reconcile(&locked, &mut state, disk.as_deref())?;
        let chain = state
            .head
            .as_ref()
            .filter(|_| recovery != ReviewRecovery::Conflict)
            .map(|head| head.chain);
        let checkpoints = state
            .checkpoints
            .into_iter()
            .map(|checkpoint| CheckpointInfo {
                id: checkpoint.id,
                operation_id: checkpoint.operation_id,
                hunk_ids: checkpoint.hunk_ids,
                restorable: chain.is_some() && checkpoint.chain == chain,
            })
            .collect();
        Ok(ReviewHistory {
            recovery,
            interrupted_id: state.pending.map(|pending| pending.id),
            checkpoints,
        })
    }

    pub fn restore_checkpoint(&self, path: &str, base: &str, id: u64) -> io::Result<Save> {
        self.restore_with_hook(path, base, id, |_| Ok(()))
    }

    fn restore_with_hook(
        &self,
        path: &str,
        base: &str,
        id: u64,
        hook: impl FnMut(SaveStep) -> io::Result<()>,
    ) -> io::Result<Save> {
        let locked = self.locked_markdown(path)?;
        let captured = ReviewSnapshot::read(&locked)?;
        if captured.revision != base {
            return Ok(Save::Conflict {
                disk: captured.into_document(),
            });
        }
        let private = self.review_private()?;
        let store = self
            .review_store(&locked, &private, false)?
            .ok_or_else(|| invalid("No review history."))?;
        let mut state = store.load()?;
        require_resolved(store.reconcile(&locked, &mut state, Some(base))?)?;
        let checkpoint = state
            .checkpoints
            .iter()
            .find(|checkpoint| checkpoint.id == id)
            .ok_or_else(|| invalid("Checkpoint unavailable."))?;
        let chain = state
            .head
            .as_ref()
            .map(|head| head.chain)
            .filter(|chain| Some(*chain) == checkpoint.chain)
            .ok_or_else(|| invalid("The document changed outside this checkpoint history."))?;
        let output = store.preimage(checkpoint.id, &checkpoint.before)?;
        let action = ReviewAction::Restore {
            checkpoint_id: id,
            chain,
        };
        self.clear_committed_journal(&locked.directory, &locked.name, &private)?;
        replace_with_checkpoint(
            &locked, &store, &mut state, &captured, &output, action, hook,
        )
    }

    pub fn review_preimage(&self, path: &str, id: u64) -> io::Result<String> {
        let locked = self.locked_markdown(path)?;
        let private = self.review_private()?;
        let store = self
            .review_store(&locked, &private, false)?
            .ok_or_else(|| invalid("No review history."))?;
        let state = store.load()?;
        let before = state
            .pending
            .as_ref()
            .filter(|pending| pending.id == id)
            .map(|pending| &pending.before)
            .or_else(|| {
                state
                    .checkpoints
                    .iter()
                    .find(|checkpoint| checkpoint.id == id)
                    .map(|checkpoint| &checkpoint.before)
            })
            .ok_or_else(|| invalid("Review preimage unavailable."))?;
        store.preimage(id, before)
    }

    /// Clear an explicitly reviewed interruption and retain its preimage outside the restore chain.
    pub fn keep_disk_after_review_conflict(
        &self,
        path: &str,
        id: u64,
        expected: Option<&str>,
    ) -> io::Result<()> {
        let locked = self.locked_markdown(path)?;
        let private = self.review_private()?;
        let store = self
            .review_store(&locked, &private, false)?
            .ok_or_else(|| invalid("No interrupted review."))?;
        let disk = disk_revision(&locked)?;
        if disk.as_deref() != expected {
            return Err(invalid(
                "The disk revision changed. Review the conflict again.",
            ));
        }
        let mut state = store.load()?;
        if store.reconcile(&locked, &mut state, disk.as_deref())? != ReviewRecovery::Conflict
            || state
                .pending
                .as_ref()
                .is_none_or(|pending| pending.id != id)
        {
            return Err(invalid("This interrupted review is no longer current."));
        }
        let pending = state.pending.take().expect("confirmed interrupted review");
        pending.discard_stage(&locked)?;
        let (operation_id, hunk_ids) = match pending.action {
            ReviewAction::Accept {
                operation_id,
                hunk_ids,
                ..
            } => (operation_id, hunk_ids),
            ReviewAction::Restore { checkpoint_id, .. } => {
                let operation_id = state
                    .checkpoints
                    .iter()
                    .find(|checkpoint| checkpoint.id == checkpoint_id)
                    .ok_or_else(|| invalid("Interrupted restore checkpoint unavailable."))?
                    .operation_id
                    .clone();
                (operation_id, Vec::new())
            }
        };
        state.head = None;
        state.keep(Checkpoint {
            id,
            chain: None,
            operation_id,
            hunk_ids,
            before: pending.before,
        });
        store.write(&state)?;
        store.prune(&state)
    }

    pub fn delete_checkpoint(&self, path: &str, id: u64) -> io::Result<()> {
        let locked = self.locked_markdown(path)?;
        let private = self.review_private()?;
        let Some(store) = self.review_store(&locked, &private, false)? else {
            return Ok(());
        };
        let disk = disk_revision(&locked)?;
        let mut state = store.load()?;
        require_resolved(store.reconcile(&locked, &mut state, disk.as_deref())?)?;
        state.checkpoints.retain(|checkpoint| checkpoint.id != id);
        store.write(&state)?;
        store.prune(&state)
    }

    fn review_private(&self) -> io::Result<File> {
        self.recovery_directory()?
            .ok_or_else(|| invalid("Private recovery storage is required for checkpoints."))
    }
}

fn disk_revision(locked: &LockedTarget) -> io::Result<Option<String>> {
    match snapshot(&locked.target()) {
        Ok((metadata, bytes)) => Ok(Some(revision(&metadata, &bytes))),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn require_resolved(state: ReviewRecovery) -> io::Result<()> {
    if state == ReviewRecovery::Conflict {
        return Err(invalid(
            "Resolve the interrupted review before writing this document.",
        ));
    }
    Ok(())
}

struct ReviewSnapshot {
    metadata: Metadata,
    bytes: Vec<u8>,
    revision: String,
}

impl ReviewSnapshot {
    fn read(locked: &LockedTarget) -> io::Result<Self> {
        let (metadata, bytes) = snapshot(&locked.target())?;
        Ok(Self {
            revision: revision(&metadata, &bytes),
            metadata,
            bytes,
        })
    }

    fn into_document(self) -> Document {
        document(&self.metadata, self.bytes)
    }
}

fn replace_with_checkpoint(
    locked: &LockedTarget,
    store: &Store,
    state: &mut Journal,
    captured: &ReviewSnapshot,
    after: &str,
    action: ReviewAction,
    mut hook: impl FnMut(SaveStep) -> io::Result<()>,
) -> io::Result<Save> {
    let mut action = Some(action);
    let result = locked.replace(
        &captured.revision,
        &captured.metadata,
        after,
        |step, postimage, staged_name| {
            if step == SaveStep::JournalSynced {
                store.begin(
                    state,
                    PendingReview {
                        id: state.next_id,
                        before: captured.revision.clone(),
                        after: postimage.into(),
                        staged_name: staged_name.into(),
                        action: action.take().expect("journal prepared once"),
                    },
                    &captured.bytes,
                )?;
            }
            hook(step)
        },
    )?;
    if let Save::Saved { revision } = &result {
        store.reconcile(locked, state, Some(revision))?;
    }
    Ok(result)
}
