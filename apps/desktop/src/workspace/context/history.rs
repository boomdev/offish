//! Private, bounded conversation history, separate from workspace documents.

use super::super::{descriptor, open, stage_file};
use super::resolved_location;
use crate::workspaces::ActiveWorkspace;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, DirBuilder, File},
    io::{self, Read},
    os::unix::fs::{DirBuilderExt, MetadataExt},
    path::{Path, PathBuf},
};

const HISTORY_LIMIT: usize = 20;
const HISTORY_BYTES: u64 = 256 * 1024;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Interrupted,
    Cancelled,
    Complete,
    Stale,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub prompt: String,
    pub answer: String,
    pub references: usize,
    pub outcome: Outcome,
}

pub struct HistoryStore {
    path: PathBuf,
    workspace_id: u64,
}

impl HistoryStore {
    pub fn new(path: &Path, workspace: &ActiveWorkspace) -> io::Result<Self> {
        let location = resolved_location(path).ok_or_else(storage_error)?;
        for (_, root) in workspace.available_roots() {
            let root = fs::read_link(descriptor(&root.root)).map_err(|_| storage_error())?;
            if location.starts_with(&root) || root.starts_with(&location) {
                return Err(io::Error::other(
                    "Open a narrower workspace to keep conversation history outside its folders.",
                ));
            }
        }
        Ok(Self {
            path: location,
            workspace_id: workspace.record.id,
        })
    }

    pub fn load(&self) -> io::Result<Vec<Message>> {
        let (_directory, target) = self.lock()?;
        Self::read(&target)
    }

    pub fn append(&self, message: Message) -> io::Result<()> {
        self.update(|messages| {
            messages.push(message);
            if messages.len() > HISTORY_LIMIT {
                messages.remove(0);
            }
        })
    }

    pub fn finish(&self, completed: &Message) -> io::Result<()> {
        self.update(|messages| {
            if let Some(saved) = messages
                .iter_mut()
                .find(|message| message.id == completed.id)
            {
                saved.clone_from(completed);
            }
        })
    }

    pub fn clear(&self) -> io::Result<()> {
        let (directory, target) = self.lock()?;
        match fs::remove_file(target) {
            Ok(()) => directory.sync_all().map_err(|_| storage_error()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            _ => Err(storage_error()),
        }
    }

    fn lock(&self) -> io::Result<(File, PathBuf)> {
        DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&self.path)
            .map_err(|_| storage_error())?;
        let directory = open(&self.path, true).map_err(|_| storage_error())?;
        let metadata = directory.metadata().map_err(|_| storage_error())?;
        if metadata.uid() != unsafe { libc::geteuid() } || metadata.mode() & 0o077 != 0 {
            return Err(storage_error());
        }
        directory.lock().map_err(|_| storage_error())?;
        // A process killed during replacement may leave a staging file.
        // Writers serialize under this lock and the directory contains only chat.
        for entry in fs::read_dir(descriptor(&directory)).map_err(|_| storage_error())? {
            let entry = entry.map_err(|_| storage_error())?;
            if entry
                .file_name()
                .as_encoded_bytes()
                .starts_with(b".offish-save-")
            {
                fs::remove_file(entry.path()).map_err(|_| storage_error())?;
            }
        }
        let target = descriptor(&directory).join(format!("{}.json", self.workspace_id));
        Ok((directory, target))
    }

    fn read(target: &Path) -> io::Result<Vec<Message>> {
        let file = match open(target, false) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            _ => return Err(storage_error()),
        };
        let metadata = file.metadata().map_err(|_| storage_error())?;
        if !metadata.is_file()
            || metadata.len() > HISTORY_BYTES
            || metadata.mode() & 0o077 != 0
            || metadata.uid() != unsafe { libc::geteuid() }
            || metadata.nlink() != 1
        {
            return Err(storage_error());
        }
        let messages: Vec<Message> =
            serde_json::from_reader(file.take(HISTORY_BYTES + 1)).map_err(|_| storage_error())?;
        if messages.len() > HISTORY_LIMIT {
            return Err(storage_error());
        }
        Ok(messages)
    }

    fn update(&self, update: impl FnOnce(&mut Vec<Message>)) -> io::Result<()> {
        let (directory, target) = self.lock()?;
        let mut messages = Self::read(&target)?;
        update(&mut messages);
        if messages.is_empty() {
            return Ok(());
        }
        let (file, stage) = stage_file(&directory).map_err(|_| storage_error())?;
        serde_json::to_writer(&file, &messages).map_err(|_| storage_error())?;
        file.sync_all().map_err(|_| storage_error())?;
        stage.replace(&target).map_err(|_| storage_error())?;
        directory.sync_all().map_err(|_| storage_error())
    }
}

fn storage_error() -> io::Error {
    io::Error::other("Private conversation history is unavailable.")
}
