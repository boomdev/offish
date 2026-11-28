//! Read-only snapshots for explicit conversation attachments.

pub mod history;

use super::search::{resolved_location, rules};
use super::{Workspace, descriptor, document, identity, is_markdown, open, snapshot_file};
use serde::Serialize;
use std::{
    fs::{self, File, Metadata},
    io::{self, Read},
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};

const FILE_BYTES: u64 = 4096;
const FILES: usize = 8;
const ENTRIES: usize = 256;
const DEPTH: usize = 8;
const PATH_DEPTH: usize = 32;

#[derive(Clone, Serialize)]
pub struct Snippet {
    pub path: String,
    pub text: String,
}

pub struct ContextReader {
    private: Vec<PathBuf>,
    private_ids: Vec<(u64, u64)>,
    selection_id: Option<(u64, u64)>,
    entries_left: usize,
    pub files: Vec<Snippet>,
    pub limited: bool,
}

impl ContextReader {
    pub fn new(private_paths: &[PathBuf]) -> Self {
        let private: Vec<_> = private_paths
            .iter()
            .filter_map(|path| resolved_location(path))
            .collect();
        let private_ids = private
            .iter()
            .filter_map(|path| fs::metadata(path).ok().map(|metadata| identity(&metadata)))
            .collect();
        Self {
            private,
            private_ids,
            selection_id: None,
            entries_left: ENTRIES,
            files: Vec::new(),
            limited: false,
        }
    }

    pub fn selection_document(
        &mut self,
        workspace: &Workspace,
        path: &str,
    ) -> io::Result<super::Document> {
        if !is_markdown(Path::new(path)) {
            return Err(io::Error::other("Choose a saved Markdown selection."));
        }
        let (file, _, root) = self.resolve(workspace, path, false)?;
        let (metadata, bytes) = snapshot_file(&file)?;
        self.check_handle(&file, &root, &metadata, false)?;
        self.selection_id = Some(identity(&metadata));
        Ok(document(&metadata, bytes))
    }

    pub fn mention(
        &mut self,
        workspace: &Workspace,
        root_index: usize,
        path: &str,
        directory: bool,
    ) -> io::Result<()> {
        let (handle, mut rules, root) = self.resolve(workspace, path, directory)?;
        if directory {
            self.folder(&handle, &root, root_index, path, &mut rules, 0);
            Ok(())
        } else {
            self.snippet(&handle, &root, root_index, path)
        }
    }

    fn resolve(
        &self,
        workspace: &Workspace,
        path: &str,
        directory: bool,
    ) -> io::Result<(File, Vec<rules::DirectoryRules>, PathBuf)> {
        let mut handle = workspace.root.try_clone()?;
        let root = fs::read_link(descriptor(&handle))?;
        if root.components().any(|part| part.as_os_str() == ".git") {
            return Err(excluded());
        }
        self.check_handle(&handle, &root, &handle.metadata()?, true)?;
        let mut rules = Vec::new();
        let mut prefix = String::new();
        if path.is_empty() && directory {
            return Ok((handle, rules, root));
        }
        if path.len() > 4096 {
            return Err(excluded());
        }
        let mut names = path.split('/').peekable();
        while let Some(name) = names.next() {
            if !visible_name(name) || rules.len() == PATH_DEPTH {
                return Err(excluded());
            }
            rules.push(rules::DirectoryRules::read(&handle, &prefix).map_err(|_| excluded())?);
            prefix = join(&prefix, name);
            let is_dir = names.peek().is_some() || directory;
            if rules::excluded(&rules, &prefix, name, is_dir) {
                return Err(excluded());
            }
            handle = open(&descriptor(&handle).join(name), is_dir).map_err(|_| excluded())?;
            self.check_handle(&handle, &root, &handle.metadata()?, is_dir)?;
        }
        Ok((handle, rules, root))
    }

    fn check_handle(
        &self,
        file: &File,
        root: &Path,
        metadata: &Metadata,
        directory: bool,
    ) -> io::Result<()> {
        let location = fs::read_link(descriptor(file))?;
        if !location.starts_with(root)
            || self
                .private
                .iter()
                .any(|private| location.starts_with(private))
            || self.private_ids.contains(&identity(metadata))
            || if directory {
                !metadata.is_dir()
            } else {
                !metadata.is_file() || metadata.nlink() != 1
            }
        {
            return Err(excluded());
        }
        Ok(())
    }

    fn folder(
        &mut self,
        directory: &File,
        root: &Path,
        root_index: usize,
        relative: &str,
        rules: &mut Vec<rules::DirectoryRules>,
        depth: usize,
    ) {
        if depth >= DEPTH || self.entries_left == 0 || self.files.len() == FILES {
            self.limited = true;
            return;
        }
        let (Ok(local), Ok(entries)) = (
            rules::DirectoryRules::read(directory, relative),
            fs::read_dir(descriptor(directory)),
        ) else {
            self.limited = true;
            return;
        };
        let mut entries: Vec<_> = entries.take(self.entries_left + 1).collect();
        if entries.len() > self.entries_left {
            entries.pop();
            self.limited = true;
        }
        self.entries_left -= entries.len();
        entries.sort_by_cached_key(|entry| entry.as_ref().ok().map(|entry| entry.file_name()));
        rules.push(local);
        for entry in entries {
            if self.files.len() == FILES {
                self.limited = true;
                break;
            }
            let Ok(entry) = entry else {
                self.limited = true;
                continue;
            };
            let Ok(name) = entry.file_name().into_string() else {
                continue;
            };
            let Ok(kind) = entry.file_type() else {
                self.limited = true;
                continue;
            };
            if !visible_name(&name) || !(kind.is_dir() || kind.is_file()) {
                continue;
            }
            let path = join(relative, &name);
            if rules::excluded(rules, &path, &name, kind.is_dir()) {
                continue;
            }
            let Ok(handle) = open(&descriptor(directory).join(&name), kind.is_dir()) else {
                self.limited = true;
                continue;
            };
            if kind.is_dir() {
                if handle
                    .metadata()
                    .is_ok_and(|metadata| self.check_handle(&handle, root, &metadata, true).is_ok())
                {
                    self.folder(&handle, root, root_index, &path, rules, depth + 1);
                }
            } else if self.snippet(&handle, root, root_index, &path).is_err() {
                self.limited = true;
            }
        }
        rules.pop();
    }

    fn snippet(
        &mut self,
        file: &File,
        root: &Path,
        root_index: usize,
        relative: &str,
    ) -> io::Result<()> {
        let path = format!("root-{root_index}/{relative}");
        if self.files.iter().any(|item| item.path == path) {
            return Ok(());
        }
        let metadata = file.metadata()?;
        self.check_handle(file, root, &metadata, false)?;
        if Some(identity(&metadata)) == self.selection_id {
            return Err(io::Error::other(
                "The active document can only be shared through its selection.",
            ));
        }
        if self.files.len() == FILES || metadata.len() > FILE_BYTES {
            return Err(io::Error::other(
                "References are limited to eight text files of 4 KiB each.",
            ));
        }
        let mut bytes = Vec::new();
        file.take(FILE_BYTES + 1).read_to_end(&mut bytes)?;
        let after = file.metadata()?;
        self.check_handle(file, root, &after, false)?;
        if bytes.len() as u64 > FILE_BYTES
            || bytes.len() as u64 != after.len()
            || metadata.ctime() != after.ctime()
            || metadata.ctime_nsec() != after.ctime_nsec()
            || metadata.mtime() != after.mtime()
            || metadata.mtime_nsec() != after.mtime_nsec()
        {
            return Err(io::Error::other(
                "A reference changed while being read or exceeded the limit.",
            ));
        }
        let text = String::from_utf8(bytes)
            .ok()
            .filter(|text| !text.contains('\0'))
            .ok_or_else(|| io::Error::other("References must be UTF-8 text."))?;
        self.files.push(Snippet { path, text });
        Ok(())
    }
}

fn excluded() -> io::Error {
    io::Error::other("This entry is unavailable for conversation context.")
}

fn visible_name(name: &str) -> bool {
    !name.is_empty() && !name.starts_with('.') && !name.chars().any(char::is_control)
}

fn join(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.into()
    } else {
        format!("{parent}/{name}")
    }
}
