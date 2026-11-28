//! Host-mediated conversation turns. Providers receive only approved snapshots.

use crate::{
    markdown::{PROFILE_VERSION, SourceEdit, SourceEdits, SourceMap, front_matter_range},
    workspace::{
        context::{
            ContextReader, Snippet,
            history::{HistoryStore, Message, Outcome},
        },
        review::{Hunk, Proposal},
    },
    workspaces::ActiveWorkspace,
};
use serde::{Deserialize, Serialize};
use std::{
    io,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(test)]
mod tests;

const PROMPT_BYTES: usize = 2048;
const SELECTION_BYTES: usize = 4096;
const MENTIONS: usize = 6;
static TURN_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    prompt: String,
    selection: Selection,
    mentions: Vec<Mention>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Selection {
    path: String,
    base: String,
    from: usize,
    to: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Mention {
    path: String,
    directory: bool,
}

#[derive(Clone, Serialize)]
pub struct Prepared {
    id: String,
    prompt: String,
    selection: Captured,
    files: Vec<Snippet>,
    limited: bool,
}

#[derive(Clone, Serialize)]
struct Captured {
    path: String,
    base: String,
    from: usize,
    to: usize,
    text: String,
}

struct ProviderContext<'a> {
    prompt: &'a str,
    selection: &'a Captured,
    files: &'a [Snippet],
}

trait Adapter {
    fn start(&self, context: ProviderContext<'_>) -> Box<dyn Iterator<Item = Event> + Send>;
}

enum Event {
    Text(String),
    Candidate(SourceEdit),
}

struct Fixture;

impl Adapter for Fixture {
    fn start(&self, context: ProviderContext<'_>) -> Box<dyn Iterator<Item = Event> + Send> {
        let reference_count = context
            .files
            .iter()
            .filter(|file| !file.text.is_empty())
            .count();
        let replacement = match context.selection.text.as_str() {
            "Keep the first release on the current schedule."
            | "Review the first release schedule after the pilot."
                if context
                    .files
                    .iter()
                    .any(|file| file.text.contains("The pilot date moves to 14 December.")) =>
            {
                "Plan for the pilot on 14 December.".into()
            }
            "Keep the first release on the current schedule." => {
                "Review the first release schedule after the pilot.".into()
            }
            selection if context.prompt.to_ascii_lowercase().contains("uppercase") => {
                selection.to_uppercase()
            }
            selection => format!("Revised for the pilot: {selection}"),
        };
        let events = [
            Event::Text("The offline provider prepared a revision for your selection. ".into()),
            Event::Text(format!(
                "Received {reference_count} nonempty text references. "
            )),
            Event::Text("Checking the edit against the saved document.".into()),
            Event::Candidate(SourceEdit {
                from: context.selection.from,
                to: context.selection.to,
                expected: context.selection.text.clone(),
                replacement,
            }),
        ];
        Box::new(events.into_iter())
    }
}

struct Running {
    selection: Captured,
    message: Message,
    stream: Box<dyn Iterator<Item = Event> + Send>,
}

#[derive(Serialize)]
pub struct Staged {
    pub path: String,
    pub proposal: Proposal,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Update {
    Delta {
        text: String,
    },
    Done {
        message: Message,
        staged: Option<Staged>,
    },
    Revoked,
}

#[derive(Default)]
pub struct Conversation(Phase);

#[derive(Default)]
enum Phase {
    #[default]
    Idle,
    AwaitingApproval(Prepared),
    Streaming(Box<Running>),
}

impl Conversation {
    pub fn prepare(
        &mut self,
        workspace: &ActiveWorkspace,
        private: &[PathBuf],
        request: Request,
    ) -> io::Result<Prepared> {
        if matches!(self.0, Phase::Streaming(_)) {
            return Err(io::Error::other("Finish or cancel the active turn first."));
        }
        self.0 = Phase::Idle;
        let prompt = request.prompt.trim();
        if prompt.is_empty() || prompt.len() > PROMPT_BYTES || prompt.contains('\0') {
            return Err(io::Error::other("Enter a message of up to 2 KiB."));
        }
        if request.mentions.len() > MENTIONS {
            return Err(io::Error::other("Use at most six file or folder mentions."));
        }
        let Selection {
            path,
            base,
            from,
            to,
        } = request.selection;
        if from >= to || to - from > SELECTION_BYTES {
            return Err(io::Error::other(
                "Select a short passage in the saved Markdown document.",
            ));
        }
        let mut reader = ContextReader::new(private);
        let (_, root, relative) = workspace.route(&path)?;
        let source = checked_source(&mut reader, root, relative, &base)?;
        let range = SourceMap::new(&source)?.byte_range(from..to)?;
        if range.len() > SELECTION_BYTES
            || source[range.clone()].trim().is_empty()
            || front_matter_range(&source).is_some_and(|opaque| range.start < opaque.end)
        {
            return Err(io::Error::other(
                "Choose up to 4 KiB of document text after any front matter.",
            ));
        }
        let selection = Captured {
            path,
            base,
            from: range.start,
            to: range.end,
            text: source[range].into(),
        };
        // Capture explicit files before allocating the remaining budget to folders.
        for mention in request
            .mentions
            .iter()
            .filter(|mention| !mention.directory)
            .chain(request.mentions.iter().filter(|mention| mention.directory))
        {
            let (index, root, relative) = workspace.route(&mention.path)?;
            reader.mention(root, index, relative, mention.directory)?;
        }
        let time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let serial = TURN_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let prepared = Prepared {
            id: format!("{}-{time}-{serial}", std::process::id()),
            prompt: prompt.into(),
            selection,
            files: reader.files,
            limited: reader.limited,
        };
        self.0 = Phase::AwaitingApproval(prepared.clone());
        Ok(prepared)
    }

    pub fn approve(
        &mut self,
        id: &str,
        workspace: &ActiveWorkspace,
        private: &[PathBuf],
        history: &HistoryStore,
    ) -> io::Result<Message> {
        self.approve_with(id, workspace, private, history, &Fixture)
    }

    fn approve_with(
        &mut self,
        id: &str,
        workspace: &ActiveWorkspace,
        private: &[PathBuf],
        history: &HistoryStore,
        adapter: &dyn Adapter,
    ) -> io::Result<Message> {
        if !matches!(&self.0, Phase::AwaitingApproval(prepared) if prepared.id == id) {
            return Err(io::Error::other("The context approval has expired."));
        }
        let Phase::AwaitingApproval(prepared) = std::mem::take(&mut self.0) else {
            unreachable!()
        };
        prepared.selection.current_source(workspace, private)?;
        let message = Message {
            id: prepared.id,
            prompt: prepared.prompt,
            answer: String::new(),
            references: prepared.files.len(),
            outcome: Outcome::Interrupted,
        };
        history.append(message.clone())?;
        let stream = adapter.start(ProviderContext {
            prompt: &message.prompt,
            selection: &prepared.selection,
            files: &prepared.files,
        });
        self.0 = Phase::Streaming(Box::new(Running {
            selection: prepared.selection,
            message: message.clone(),
            stream,
        }));
        Ok(message)
    }

    pub fn poll(
        &mut self,
        id: &str,
        workspace: &ActiveWorkspace,
        private: &[PathBuf],
        history: &HistoryStore,
    ) -> io::Result<Update> {
        let Phase::Streaming(turn) = &mut self.0 else {
            return Ok(Update::Revoked);
        };
        if turn.message.id != id {
            return Ok(Update::Revoked);
        }
        let event = turn.stream.next();
        if let Some(Event::Text(text)) = event {
            if turn.message.answer.len().saturating_add(text.len()) <= 4096 {
                turn.message.answer.push_str(&text);
                return Ok(Update::Delta { text });
            }
            return self.finish(history, None);
        }
        let staged = match event {
            Some(Event::Candidate(edit)) => turn.selection.stage(id, edit, workspace, private).ok(),
            _ => None,
        };
        self.finish(history, staged)
    }

    fn finish(&mut self, history: &HistoryStore, staged: Option<Staged>) -> io::Result<Update> {
        let Phase::Streaming(mut turn) = std::mem::take(&mut self.0) else {
            unreachable!()
        };
        turn.message.outcome = if staged.is_some() {
            Outcome::Complete
        } else {
            Outcome::Stale
        };
        history.finish(&turn.message)?;
        Ok(Update::Done {
            message: turn.message,
            staged,
        })
    }

    pub fn cancel(&mut self, id: &str, history: &HistoryStore) -> io::Result<Option<Message>> {
        match &self.0 {
            Phase::AwaitingApproval(prepared) if prepared.id == id => {
                self.0 = Phase::Idle;
                Ok(None)
            }
            Phase::Streaming(turn) if turn.message.id == id => {
                let Phase::Streaming(mut turn) = std::mem::take(&mut self.0) else {
                    unreachable!()
                };
                turn.message.outcome = Outcome::Cancelled;
                history.finish(&turn.message)?;
                Ok(Some(turn.message))
            }
            _ => Ok(history.load()?.into_iter().find(|message| message.id == id)),
        }
    }

    pub fn revoke(&mut self) {
        self.0 = Phase::Idle;
    }
}

impl Captured {
    fn current_source(
        &self,
        workspace: &ActiveWorkspace,
        private: &[PathBuf],
    ) -> io::Result<String> {
        let (_, root, relative) = workspace.route(&self.path)?;
        checked_source(&mut ContextReader::new(private), root, relative, &self.base)
    }

    fn stage(
        &self,
        id: &str,
        edit: SourceEdit,
        workspace: &ActiveWorkspace,
        private: &[PathBuf],
    ) -> io::Result<Staged> {
        if edit.from != self.from
            || edit.to != self.to
            || edit.expected != self.text
            || edit.replacement.len() > 2 * SELECTION_BYTES
            || edit.expected == edit.replacement
        {
            return Err(io::Error::other(
                "The edit falls outside the approved selection.",
            ));
        }
        let source = self.current_source(workspace, private)?;
        let (_, _, relative) = workspace.route(&self.path)?;
        let batch = SourceEdits {
            profile: PROFILE_VERSION,
            edits: vec![edit],
        };
        batch.apply(&source, 16 * 1024 * 1024)?;
        let [edit] = batch.edits.try_into().expect("one selection edit");
        Ok(Staged {
            path: self.path.clone(),
            proposal: Proposal {
                operation_id: id.into(),
                path: relative.into(),
                base_revision: self.base.clone(),
                profile: PROFILE_VERSION,
                hunks: vec![Hunk {
                    id: "selection".into(),
                    edit,
                }],
            },
        })
    }
}

fn checked_source(
    reader: &mut ContextReader,
    root: &crate::workspace::Workspace,
    relative: &str,
    base: &str,
) -> io::Result<String> {
    let document = reader.selection_document(root, relative)?;
    if document.revision != base {
        return Err(io::Error::other(
            "The selection changed on disk. Select it again.",
        ));
    }
    document
        .text
        .ok_or_else(|| io::Error::other("Choose a saved UTF-8 Markdown document."))
}
