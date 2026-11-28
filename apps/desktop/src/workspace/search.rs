pub(super) mod rules;

use super::{Workspace, descriptor, identity, is_markdown, open};
use regex::{Regex, RegexBuilder};
use serde::Serialize;
use std::{
    collections::HashMap,
    fs::{self, File, Metadata},
    io::{self, Read},
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const FILE_LIMIT: u64 = 2 * 1024 * 1024;
const CACHE_BYTES: usize = 64 * 1024 * 1024;
const CACHE_FILES: usize = 20_000;
const WALK_LIMIT: usize = 100_000;
const READ_LIMIT: u64 = 256 * 1024 * 1024;
const RESULT_LIMIT: usize = 250;

pub struct SearchRoot {
    index: usize,
    directory: File,
}

impl SearchRoot {
    pub fn new(index: usize, workspace: &Workspace) -> io::Result<Self> {
        Ok(Self {
            index,
            directory: workspace.root.try_clone()?,
        })
    }
}

#[derive(Serialize)]
pub struct SearchResult {
    pub path: String,
    pub kind: &'static str,
    pub line: usize,
    pub column: usize,
    pub matched: String,
    pub preview: String,
}

#[derive(Default, Serialize)]
pub struct SearchReport {
    pub results: Vec<SearchResult>,
    pub files: usize,
    pub skipped: usize,
    pub limited: bool,
    pub more_matches: bool,
    pub scanned_at: u64,
}

#[derive(Eq, PartialEq)]
struct Stamp {
    id: (u64, u64),
    size: u64,
    modified: (i64, i64),
    changed: (i64, i64),
}

impl From<&Metadata> for Stamp {
    fn from(metadata: &Metadata) -> Self {
        Self {
            id: identity(metadata),
            size: metadata.len(),
            modified: (metadata.mtime(), metadata.mtime_nsec()),
            changed: (metadata.ctime(), metadata.ctime_nsec()),
        }
    }
}

struct CachedFile {
    stamp: Stamp,
    text: Option<String>,
}

#[derive(Default)]
pub struct SearchIndex {
    cache: HashMap<String, CachedFile>,
}

impl SearchIndex {
    pub fn search(
        &mut self,
        roots: &[SearchRoot],
        private_paths: &[PathBuf],
        query: &str,
        rebuild: bool,
    ) -> io::Result<SearchReport> {
        let query = query.trim();
        if query.len() > 512 || query.contains(['\r', '\n', '\0']) {
            return Err(io::Error::other(
                "Enter a single line of up to 512 UTF-8 bytes.",
            ));
        }
        let needle = if query.is_empty() {
            None
        } else {
            Some(
                RegexBuilder::new(&regex::escape(query))
                    .case_insensitive(true)
                    .build()
                    .map_err(io::Error::other)?,
            )
        };
        let private: Vec<_> = private_paths
            .iter()
            .filter_map(|path| resolved_location(path))
            .collect();
        let private_ids = private
            .iter()
            .filter_map(|path| fs::metadata(path).ok())
            .map(|metadata| identity(&metadata))
            .collect();
        let previous = std::mem::take(&mut self.cache);
        let mut scan = Scan {
            previous: if rebuild { HashMap::new() } else { previous },
            cache: &mut self.cache,
            cached_bytes: 0,
            entries_left: WALK_LIMIT,
            bytes_left: READ_LIMIT,
            private_ids,
            private_relative: Vec::new(),
            needle,
            report: SearchReport::default(),
        };
        for root in roots {
            let path = match fs::read_link(descriptor(&root.directory)) {
                Ok(path) => path,
                Err(_) => {
                    scan.report.skipped += 1;
                    continue;
                }
            };
            if private.iter().any(|excluded| path.starts_with(excluded))
                || path
                    .components()
                    .any(|component| component.as_os_str() == ".git")
            {
                scan.report.skipped += 1;
                continue;
            }
            scan.private_relative = private
                .iter()
                .filter_map(|excluded| {
                    excluded
                        .strip_prefix(&path)
                        .ok()?
                        .to_str()
                        .map(str::to_owned)
                })
                .collect();
            scan.directory(&root.directory, root.index, "", &mut Vec::new(), 0);
        }
        scan.report.scanned_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Ok(scan.report)
    }
}

pub(super) fn resolved_location(path: &Path) -> Option<PathBuf> {
    // Exclude app storage even if it is created after the scan starts.
    path.ancestors().find_map(|ancestor| {
        let canonical = ancestor.canonicalize().ok()?;
        Some(canonical.join(path.strip_prefix(ancestor).ok()?))
    })
}

struct Scan<'a> {
    previous: HashMap<String, CachedFile>,
    cache: &'a mut HashMap<String, CachedFile>,
    cached_bytes: usize,
    entries_left: usize,
    bytes_left: u64,
    private_ids: Vec<(u64, u64)>,
    private_relative: Vec<String>,
    needle: Option<Regex>,
    report: SearchReport,
}

impl Scan<'_> {
    fn directory(
        &mut self,
        directory: &File,
        root: usize,
        relative: &str,
        rules: &mut Vec<rules::DirectoryRules>,
        depth: usize,
    ) {
        if depth >= 128 || self.entries_left == 0 || self.bytes_left == 0 {
            self.report.limited = true;
            return;
        }
        let Ok(entries) = fs::read_dir(descriptor(directory)) else {
            self.report.skipped += 1;
            return;
        };
        let Ok(local_rules) = rules::DirectoryRules::read(directory, relative) else {
            // An unreadable policy could contain exclusions for this subtree.
            self.report.skipped += 1;
            return;
        };
        rules.push(local_rules);
        let mut entries: Vec<_> = entries
            .take(self.entries_left + 1)
            .filter_map(|entry| match entry {
                Ok(entry) => Some(entry),
                Err(_) => {
                    self.report.skipped += 1;
                    None
                }
            })
            .collect();
        entries.sort_by_cached_key(|entry| entry.file_name());
        for entry in entries {
            if self.entries_left == 0 || self.bytes_left == 0 {
                self.report.limited = true;
                break;
            }
            self.entries_left -= 1;
            let Ok(name) = entry.file_name().into_string() else {
                self.report.skipped += 1;
                continue;
            };
            if matches!(name.as_str(), ".git" | ".gitignore" | ".offishignore")
                || name.starts_with(".offish-save-")
            {
                continue;
            }
            let Ok(kind) = entry.file_type() else {
                self.report.skipped += 1;
                continue;
            };
            if !kind.is_dir() && !kind.is_file() {
                continue;
            }
            let path = if relative.is_empty() {
                name.clone()
            } else {
                format!("{relative}/{name}")
            };
            if self.private_relative.contains(&path)
                || rules::excluded(rules, &path, &name, kind.is_dir())
            {
                continue;
            }
            let handle = descriptor(directory).join(&name);
            if kind.is_dir() {
                // Open each child from its already authorized parent without following links.
                match open(&handle, true)
                    .and_then(|child| Ok((identity(&child.metadata()?), child)))
                {
                    Ok((id, child)) if !self.private_ids.contains(&id) => {
                        self.directory(&child, root, &path, rules, depth + 1)
                    }
                    Ok(_) => (),
                    Err(_) => self.report.skipped += 1,
                }
            } else if self.file(&handle, root, &path).is_err() {
                self.report.skipped += 1;
            }
        }
        rules.pop();
    }

    fn file(&mut self, path: &Path, root: usize, relative: &str) -> io::Result<()> {
        let file = open(path, false)?;
        let metadata = file.metadata()?;
        if !metadata.is_file() || metadata.len() > FILE_LIMIT {
            self.report.skipped += 1;
            return Ok(());
        }
        let key = format!("root-{root}/{relative}");
        let stamp = Stamp::from(&metadata);
        let entry = match self.previous.remove(&key) {
            Some(entry) if entry.stamp == stamp => entry,
            _ => {
                if self.bytes_left < stamp.size {
                    self.bytes_left = 0;
                    self.report.limited = true;
                    return Ok(());
                }
                let mut bytes = Vec::new();
                (&file)
                    .take(FILE_LIMIT.min(self.bytes_left) + 1)
                    .read_to_end(&mut bytes)?;
                self.bytes_left = self.bytes_left.saturating_sub(bytes.len() as u64);
                if bytes.len() as u64 > FILE_LIMIT || Stamp::from(&file.metadata()?) != stamp {
                    return Err(io::Error::other("File changed during the scan."));
                }
                let text = String::from_utf8(bytes)
                    .ok()
                    .filter(|text| !text.contains('\0'));
                CachedFile { stamp, text }
            }
        };
        let bytes = if let Some(text) = &entry.text {
            self.report.files += 1;
            self.matches(&key, relative, text);
            text.len()
        } else {
            // Remember unreadable-as-text files while their filesystem stamp holds.
            self.report.skipped += 1;
            0
        };
        if self.cache.len() < CACHE_FILES && self.cached_bytes + bytes <= CACHE_BYTES {
            self.cached_bytes += bytes;
            self.cache.insert(key, entry);
        }
        Ok(())
    }

    fn matches(&mut self, key: &str, relative: &str, text: &str) {
        let Some(needle) = &self.needle else { return };
        if self.report.more_matches {
            return;
        }
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        if !needle.is_match(text) {
            return;
        }
        let kind = if is_markdown(Path::new(relative)) {
            "markdown"
        } else {
            "reference"
        };
        for (index, line) in text.lines().flat_map(|line| line.split('\r')).enumerate() {
            let mut previous_start = 0;
            let mut column = 0;
            for found in needle.find_iter(line) {
                if self.report.results.len() == RESULT_LIMIT {
                    self.report.more_matches = true;
                    return;
                }
                let before = &line[..found.start()];
                let after = &line[found.end()..];
                column += line[previous_start..found.start()].encode_utf16().count();
                previous_start = found.start();
                self.report.results.push(SearchResult {
                    path: key.into(),
                    kind,
                    line: index + 1,
                    column,
                    matched: found.as_str().into(),
                    preview: preview(before, found.as_str(), after),
                });
            }
        }
    }
}

fn preview(before: &str, matched: &str, after: &str) -> String {
    let start = before
        .char_indices()
        .rev()
        .nth(60)
        .map(|(index, ch)| index + ch.len_utf8());
    let end = after.char_indices().nth(90).map(|(index, _)| index);
    format!(
        "{}{}{}{}{}",
        if start.is_some() { "…" } else { "" },
        &before[start.unwrap_or(0)..],
        matched,
        &after[..end.unwrap_or(after.len())],
        if end.is_some() { "…" } else { "" }
    )
}
