//! Host-mediated conversation turns. Providers receive only approved snapshots.

use crate::{
    markdown::{PROFILE_VERSION, SourceEdit, SourceEdits, SourceMap, front_matter_range},
    workspace::{
        context::{
            ContextReader, Snippet,
            history::{HistoryStore, Message, Outcome, Provider},
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

pub mod codex;
#[cfg(test)]
mod tests;

const PROMPT_BYTES: usize = 2048;
const DOCUMENT_BYTES: usize = 48 * 1024;
const REPLACEMENT_BYTES: usize = 64 * 1024;
const MENTIONS: usize = 6;
static TURN_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    #[serde(default)]
    provider: Provider,
    prompt: String,
    selection: Option<Selection>,
    #[serde(default)]
    mode: Mode,
    #[serde(default)]
    settings: codex::Settings,
    mentions: Vec<Mention>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Selection {
    path: String,
    base: String,
    from: Option<usize>,
    to: Option<usize>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Mention {
    path: String,
    directory: bool,
}

#[derive(Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Mode {
    Ask,
    #[default]
    Edit,
}

#[derive(Clone, Serialize)]
pub struct Prepared {
    provider: Provider,
    id: String,
    prompt: String,
    selection: Option<Captured>,
    mode: Mode,
    settings: codex::Settings,
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
    whole: bool,
    #[serde(skip)]
    crlf: bool,
}

struct ProviderContext<'a> {
    prompt: &'a str,
    selection: Option<&'a Captured>,
    mode: Mode,
    settings: &'a codex::Settings,
    files: &'a [Snippet],
}

trait Adapter {
    fn start(&self, context: ProviderContext<'_>) -> Box<dyn Iterator<Item = Event> + Send>;
}

enum Event {
    Pending,
    Complete,
    Text(String),
    Candidate(SourceEdit),
    Failed(String),
}

struct Fixture;

impl Adapter for Fixture {
    fn start(&self, context: ProviderContext<'_>) -> Box<dyn Iterator<Item = Event> + Send> {
        let reference_count = context
            .files
            .iter()
            .filter(|file| !file.text.is_empty())
            .count();
        let Some(selection) = context.selection.filter(|_| context.mode == Mode::Edit) else {
            let answer = if let Some(document) = context.selection {
                format!(
                    "The approved passage contains {} words. I received {reference_count} text references.",
                    document.text.split_whitespace().count()
                )
            } else {
                format!("I received {reference_count} text references.")
            };
            return Box::new([Event::Text(answer), Event::Complete].into_iter());
        };
        let replacement = match selection.text.as_str() {
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
                from: selection.from,
                to: selection.to,
                expected: selection.text.clone(),
                replacement,
            }),
        ];
        Box::new(events.into_iter())
    }
}

struct Running {
    selection: Option<Captured>,
    mode: Mode,
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
    Waiting,
    Delta {
        text: String,
    },
    Done {
        message: Message,
        staged: Option<Staged>,
    },
    Failed {
        message: Message,
        error: String,
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
        let mut reader = ContextReader::new(private);
        let selection = request
            .selection
            .map(|selection| capture(workspace, &mut reader, selection, request.mode))
            .transpose()?;
        if request.mode == Mode::Edit && selection.is_none() {
            return Err(io::Error::other(
                "Open a saved Markdown document to request an edit.",
            ));
        }
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
            provider: request.provider,
            mode: request.mode,
            settings: request.settings,
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
        self.approve_with_account(id, workspace, private, history, None)
    }

    pub fn approve_with_account(
        &mut self,
        id: &str,
        workspace: &ActiveWorkspace,
        private: &[PathBuf],
        history: &HistoryStore,
        accounts: Option<&codex::auth::AccountStore>,
    ) -> io::Result<Message> {
        let Phase::AwaitingApproval(prepared) = &self.0 else {
            return Err(io::Error::other("The context approval has expired."));
        };
        match prepared.provider {
            Provider::Offline => self.approve_with(id, workspace, private, history, &Fixture),
            Provider::Codex => {
                let store =
                    accounts.ok_or_else(|| io::Error::other("ChatGPT sign-in is unavailable."))?;
                self.approve_with(
                    id,
                    workspace,
                    private,
                    history,
                    &codex::Codex(store.clone()),
                )
            }
        }
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
        if let Some(selection) = &prepared.selection {
            selection.current_source(workspace, private)?;
        }
        let message = Message {
            provider: prepared.provider,
            id: prepared.id,
            prompt: prepared.prompt,
            answer: String::new(),
            references: prepared.files.len(),
            outcome: Outcome::Interrupted,
        };
        history.append(message.clone())?;
        let stream = adapter.start(ProviderContext {
            prompt: &message.prompt,
            selection: prepared.selection.as_ref(),
            mode: prepared.mode,
            settings: &prepared.settings,
            files: &prepared.files,
        });
        self.0 = Phase::Streaming(Box::new(Running {
            selection: prepared.selection,
            mode: prepared.mode,
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
        if matches!(event, Some(Event::Pending)) {
            return Ok(Update::Waiting);
        }
        if let Some(Event::Failed(error)) = event {
            let Phase::Streaming(mut turn) = std::mem::take(&mut self.0) else {
                unreachable!()
            };
            turn.message.outcome = Outcome::Failed;
            history.finish(&turn.message)?;
            return Ok(Update::Failed {
                message: turn.message,
                error,
            });
        }
        if let Some(Event::Text(text)) = event {
            if turn.message.answer.len().saturating_add(text.len()) <= 4096 {
                turn.message.answer.push_str(&text);
                return Ok(Update::Delta { text });
            }
            return self.finish(history, None, false);
        }
        let answered = matches!(event, Some(Event::Complete)) && turn.mode == Mode::Ask;
        let staged = match event {
            Some(Event::Candidate(edit)) if turn.mode == Mode::Edit => turn
                .selection
                .as_ref()
                .and_then(|selection| selection.stage(id, edit, workspace, private).ok()),
            _ => None,
        };
        self.finish(history, staged, answered)
    }

    fn finish(
        &mut self,
        history: &HistoryStore,
        staged: Option<Staged>,
        answered: bool,
    ) -> io::Result<Update> {
        let Phase::Streaming(mut turn) = std::mem::take(&mut self.0) else {
            unreachable!()
        };
        turn.message.outcome = if staged.is_some() || answered {
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
        mut edit: SourceEdit,
        workspace: &ActiveWorkspace,
        private: &[PathBuf],
    ) -> io::Result<Staged> {
        if edit.from != self.from
            || edit.to != self.to
            || edit.expected != self.text
            || edit.replacement.len() > REPLACEMENT_BYTES
            || edit.expected == edit.replacement
        {
            return Err(io::Error::other(
                "The edit falls outside the approved selection.",
            ));
        }
        let source = self.current_source(workspace, private)?;
        if self.whole
            && edit.from == source.len()
            && !source.ends_with('\n')
            && front_matter_range(&source).is_some_and(|metadata| metadata.end == edit.from)
            && !edit.replacement.starts_with(['\r', '\n'])
        {
            edit.replacement
                .insert_str(0, if self.crlf { "\r\n" } else { "\n" });
        }
        let (_, _, relative) = workspace.route(&self.path)?;
        let edit = if self.whole { trim_edit(edit) } else { edit };
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
                    id: if self.whole { "document" } else { "selection" }.into(),
                    edit,
                }],
            },
        })
    }
}

fn capture(
    workspace: &ActiveWorkspace,
    reader: &mut ContextReader,
    selection: Selection,
    mode: Mode,
) -> io::Result<Captured> {
    let Selection {
        path,
        base,
        from,
        to,
    } = selection;
    let (_, root, relative) = workspace.route(&path)?;
    let source = checked_source(reader, root, relative, &base)?;
    let whole = from.is_none() && to.is_none();
    let metadata_end = front_matter_range(&source)
        .map(|range| range.end)
        .unwrap_or_else(|| if source.starts_with('\u{feff}') { 3 } else { 0 });
    let range = match (from, to) {
        (Some(from), Some(to)) if from < to => SourceMap::new(&source)?.byte_range(from..to)?,
        (None, None) if mode == Mode::Ask => 0..source.len(),
        (None, None) => {
            SourceMap::new(&source)?;
            metadata_end..source.len()
        }
        _ => return Err(io::Error::other("The selected passage is invalid.")),
    };
    if range.len() > DOCUMENT_BYTES {
        return Err(io::Error::other(
            "This context exceeds 48 KiB. Select a smaller passage.",
        ));
    }
    if mode == Mode::Edit && range.start < metadata_end {
        return Err(io::Error::other(
            "Choose document text after the front matter to edit.",
        ));
    }
    let text = source[range.clone()].to_owned();
    Ok(Captured {
        path,
        base,
        from: range.start,
        to: range.end,
        text,
        whole,
        crlf: source.contains("\r\n"),
    })
}

// Compact a full-body replacement into one exact source hunk. A CRLF
// contributes one source boundary even when only one byte differs.
fn trim_edit(mut edit: SourceEdit) -> SourceEdit {
    let before = edit.expected.as_bytes();
    let after = edit.replacement.as_bytes();
    let boundary = |text: &str, index: usize| {
        text.is_char_boundary(index)
            && !(index > 0
                && text.as_bytes().get(index) == Some(&b'\n')
                && text.as_bytes()[index - 1] == b'\r')
    };
    let mut prefix = before.iter().zip(after).take_while(|(a, b)| a == b).count();
    while !boundary(&edit.expected, prefix) || !boundary(&edit.replacement, prefix) {
        prefix -= 1;
    }
    let mut suffix = before[prefix..]
        .iter()
        .rev()
        .zip(after[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    while !boundary(&edit.expected, before.len() - suffix)
        || !boundary(&edit.replacement, after.len() - suffix)
    {
        suffix -= 1;
    }
    edit.from += prefix;
    edit.to -= suffix;
    edit.expected = edit.expected[prefix..before.len() - suffix].into();
    edit.replacement = edit.replacement[prefix..after.len() - suffix].into();
    edit
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
            "The document changed on disk. Prepare the context again.",
        ));
    }
    document
        .text
        .ok_or_else(|| io::Error::other("Choose a saved UTF-8 Markdown document."))
}
