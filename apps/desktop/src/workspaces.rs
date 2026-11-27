//! Local workspace settings and root routing for the desktop.
use crate::workspace::{Entry, Workspace};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, DirBuilder, File, OpenOptions},
    io::{self, Write},
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct SavedWorkspace {
    pub id: u64,
    pub roots: Vec<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_name: Option<String>,
}

impl SavedWorkspace {
    pub fn name(&self) -> String {
        if let Some(name) = &self.custom_name {
            return name.clone();
        }
        let Some(root) = self.roots.first() else {
            return "Workspace".into();
        };
        let first = root_label(root);
        if self.roots.len() == 1 {
            first
        } else {
            format!("{first} + {}", self.roots.len() - 1)
        }
    }
}

fn root_label(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

#[derive(Default, Deserialize, Serialize)]
pub struct Catalog {
    pub last: Option<u64>,
    pub saved: Vec<SavedWorkspace>,
}

impl Catalog {
    pub fn recent(&self) -> Option<&SavedWorkspace> {
        self.saved.iter().find(|entry| Some(entry.id) == self.last)
    }

    pub fn reopen(&mut self, id: u64) -> io::Result<SavedWorkspace> {
        let entry = self
            .saved
            .iter()
            .find(|entry| entry.id == id)
            .ok_or_else(|| io::Error::other("This saved workspace is unavailable."))?;
        self.last = Some(id);
        Ok(entry.clone())
    }

    pub fn rename(&mut self, id: u64, name: &str) -> io::Result<String> {
        let name = name.trim();
        if name.is_empty() || name.chars().any(char::is_control) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Enter a workspace name on a single line.",
            ));
        }
        let entry = self
            .saved
            .iter_mut()
            .find(|entry| entry.id == id)
            .ok_or_else(|| io::Error::other("This saved workspace is unavailable."))?;
        entry.custom_name = Some(name.into());
        Ok(entry.name())
    }

    pub fn add_roots(
        &mut self,
        current: &SavedWorkspace,
        roots: Vec<PathBuf>,
    ) -> io::Result<SavedWorkspace> {
        let entry = self
            .saved
            .iter_mut()
            .find(|entry| entry.id == current.id)
            .ok_or_else(|| io::Error::other("This saved workspace is unavailable."))?;
        if !entry.roots.starts_with(&current.roots) {
            return Err(io::Error::other(
                "This workspace changed on disk. Reopen it before adding folders.",
            ));
        }
        for root in roots {
            if !entry.roots.contains(&root) {
                entry.roots.push(root);
            }
        }
        self.last = Some(entry.id);
        Ok(entry.clone())
    }

    pub fn remember(&mut self, roots: Vec<PathBuf>) -> io::Result<SavedWorkspace> {
        let entry = if let Some(entry) = self.saved.iter().find(|entry| entry.roots == roots) {
            entry.clone()
        } else {
            let id = self
                .saved
                .iter()
                .map(|entry| entry.id)
                .max()
                .unwrap_or(0)
                .checked_add(1)
                .ok_or_else(|| io::Error::other("Workspace ID limit reached."))?;
            let entry = SavedWorkspace {
                id,
                roots,
                custom_name: None,
            };
            self.saved.push(entry.clone());
            entry
        };
        self.last = Some(entry.id);
        Ok(entry)
    }
}

pub struct CatalogStore(PathBuf);

impl CatalogStore {
    pub fn new(directory: PathBuf) -> Self {
        Self(directory)
    }

    pub fn load(&self) -> io::Result<Catalog> {
        let path = self.0.join("workspaces.json");
        let file = match OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)
        {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Catalog::default()),
            Err(error) => return Err(error),
        };
        Ok(serde_json::from_reader(file)?)
    }

    pub fn update<T>(&self, update: impl FnOnce(&mut Catalog) -> io::Result<T>) -> io::Result<T> {
        DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&self.0)?;
        let lock = OpenOptions::new()
            .write(true)
            .create(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(self.0.join("workspaces.lock"))?;
        lock.lock()?;
        let mut catalog = self.load()?;
        let result = update(&mut catalog)?;
        let staging = self.0.join("workspaces.json.pending");
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&staging)?;
        serde_json::to_writer_pretty(&mut file, &catalog)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        fs::rename(&staging, self.0.join("workspaces.json"))?;
        File::open(&self.0)?.sync_all()?;
        Ok(result)
    }
}

pub struct ActiveWorkspace {
    pub record: SavedWorkspace,
    roots: Vec<Option<Workspace>>,
}

impl ActiveWorkspace {
    pub fn open(record: SavedWorkspace, recovery: &Path) -> Self {
        let roots = record
            .roots
            .iter()
            .map(|path| open_root(path, recovery))
            .collect();
        Self { record, roots }
    }

    pub fn extend(&mut self, record: SavedWorkspace, recovery: &Path) {
        // Saved root order is append-only. Keep existing handles and routing
        // keys so open tabs still refer to the same directories.
        for (index, path) in record.roots.iter().enumerate() {
            if let Some(root) = self.roots.get_mut(index) {
                if root.is_none() {
                    *root = open_root(path, recovery);
                }
            } else {
                self.roots.push(open_root(path, recovery));
            }
        }
        self.record = record;
    }

    pub fn root_entries(&self) -> Vec<Entry> {
        self.record
            .roots
            .iter()
            .enumerate()
            .map(|(index, path)| Entry {
                name: format!("root-{index}"),
                kind: if self.roots[index].is_some() {
                    "root"
                } else {
                    "unavailable"
                },
                label: Some(root_label(path)),
                hint: Some(path.display().to_string()),
            })
            .collect()
    }

    pub fn route<'a>(&self, path: &'a str) -> io::Result<(usize, &Workspace, &'a str)> {
        let (key, relative) = path.split_once('/').unwrap_or((path, ""));
        let index = key
            .strip_prefix("root-")
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|index| key == format!("root-{index}"))
            .ok_or_else(|| io::Error::other("Choose an entry inside an open root folder."))?;
        let root = self
            .roots
            .get(index)
            .and_then(Option::as_ref)
            .ok_or_else(|| io::Error::other("This workspace root is unavailable."))?;
        Ok((index, root, relative))
    }

    pub fn available_roots(&self) -> impl Iterator<Item = (usize, &Workspace)> {
        self.roots
            .iter()
            .enumerate()
            .filter_map(|(index, root)| Some((index, root.as_ref()?)))
    }

    pub fn selected_file(&self, path: &Path) -> io::Result<Option<String>> {
        // Choose the deepest root when roots overlap.
        let mut candidates: Vec<_> = self.available_roots().collect();
        candidates
            .sort_by_key(|(_, root)| std::cmp::Reverse(Path::new(&root.name).components().count()));
        for (index, root) in candidates {
            if let Some(relative) = root.selected_file(path)? {
                return Ok(Some(format!("root-{index}/{relative}")));
            }
        }
        Ok(None)
    }
}

fn open_root(path: &Path, recovery: &Path) -> Option<Workspace> {
    // A saved path that now resolves elsewhere stays unavailable.
    let workspace = Workspace::open(path).ok()?;
    if Path::new(&workspace.name) != path {
        return None;
    }
    Some(workspace.with_recovery(recovery.into()))
}

pub fn canonical_roots(paths: Vec<PathBuf>) -> io::Result<Vec<PathBuf>> {
    let mut roots = Vec::new();
    for path in paths {
        let path = path.canonicalize()?;
        if !path.is_dir() || path.to_str().is_none() {
            return Err(io::Error::other("Choose a local folder with a UTF-8 path."));
        }
        if !roots.contains(&path) {
            roots.push(path);
        }
    }
    if roots.is_empty() {
        return Err(io::Error::other("Choose at least one root folder."));
    }
    Ok(roots)
}
