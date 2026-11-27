//! A private index maps moved document names to their existing checkpoint stores.

use super::*;

const INDEX: &str = "review-locations.json";
const VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct Relocation {
    source: String,
    destination: String,
    store: String,
    identity: (u64, u64),
}

impl Relocation {
    fn matches(&self, metadata: &Metadata) -> bool {
        metadata.is_file() && metadata.nlink() == 1 && identity(metadata) == self.identity
    }
}

#[derive(Serialize, Deserialize)]
struct Index {
    version: u32,
    next_store: u64,
    // An explicit empty binding hides a legacy store whose document moved away.
    bindings: BTreeMap<String, Option<String>>,
    moving: Vec<Relocation>,
}

impl Default for Index {
    fn default() -> Self {
        Self {
            version: VERSION,
            next_store: 1,
            bindings: BTreeMap::new(),
            moving: Vec::new(),
        }
    }
}

struct Registry {
    private: File,
    _lock: File,
    index: Index,
}

enum Location {
    Store(String),
    Empty,
    Unresolved,
}

impl Registry {
    fn open(private: &File) -> io::Result<Self> {
        // Keep the lock inode stable while replacing the index atomically.
        // Callers already hold the relevant workspace directory locks.
        let root = descriptor(private);
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(root.join("review-locations.lock"))?;
        if !lock.metadata()?.is_file() {
            return Err(invalid("Invalid checkpoint location lock."));
        }
        lock.lock()?;
        let index = match open(&root.join(INDEX), false) {
            Ok(file) => serde_json::from_reader::<_, Index>(file)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => Index::default(),
            Err(error) => return Err(error),
        };
        if index.version != VERSION {
            return Err(invalid("Checkpoint locations use an unsupported version."));
        }
        Ok(Self {
            private: private.try_clone()?,
            _lock: lock,
            index,
        })
    }

    fn has_index(private: &File) -> io::Result<bool> {
        match fs::symlink_metadata(descriptor(private).join(INDEX)) {
            Ok(_) => Ok(true),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error),
        }
    }

    fn write(&self) -> io::Result<()> {
        write_private(&self.private, INDEX, |file| {
            Ok(serde_json::to_writer(file, &self.index)?)
        })
    }

    fn find_move(&self, key: &str) -> Option<usize> {
        self.index
            .moving
            .iter()
            .position(|item| item.source == key || item.destination == key)
    }

    fn finish(&mut self, index: usize, moved: bool) -> io::Result<()> {
        let item = self.index.moving.remove(index);
        if moved {
            self.index.bindings.insert(item.source, None);
            self.index
                .bindings
                .insert(item.destination, Some(item.store));
        }
        self.write()
    }

    fn locate(&mut self, target: &LockedTarget, key: &str) -> io::Result<Location> {
        if let Some(index) = self.find_move(key) {
            let item = &self.index.moving[index];
            match fs::symlink_metadata(target.target()) {
                Ok(metadata) if item.matches(&metadata) => {}
                Ok(_) => return Ok(Location::Unresolved),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    return Ok(Location::Unresolved);
                }
                Err(error) => return Err(error),
            }
            let moved = item.destination == key;
            target.directory.sync_all()?;
            self.finish(index, moved)?;
        }
        Ok(match self.index.bindings.get(key) {
            Some(Some(store)) => Location::Store(store.clone()),
            Some(None) => Location::Empty,
            None => Location::Store(key.into()),
        })
    }

    fn store(
        &mut self,
        target: &LockedTarget,
        key: &str,
        create: bool,
    ) -> io::Result<Option<Store>> {
        match self.locate(target, key)? {
            Location::Store(name) => Store::open(&self.private, &name, create),
            Location::Empty if create => {
                let name = format!("{key}-{}", self.index.next_store);
                self.index.next_store = self
                    .index
                    .next_store
                    .checked_add(1)
                    .ok_or_else(|| invalid("Checkpoint store ID limit reached."))?;
                let store = Store::open(&self.private, &name, true)?;
                self.index.bindings.insert(key.into(), Some(name));
                self.write()?;
                Ok(store)
            }
            Location::Unresolved if create => Err(invalid(
                "Open the moved document before starting another review at this location.",
            )),
            Location::Empty | Location::Unresolved => Ok(None),
        }
    }
}

pub(super) fn lookup(
    private: &File,
    target: &LockedTarget,
    key: &str,
    create: bool,
) -> io::Result<Option<Store>> {
    // The directory lock serializes a first move involving this location.
    if !Registry::has_index(private)? {
        return Store::open(private, key, create);
    }
    Registry::open(private)?.store(target, key, create)
}

pub(in crate::workspace) struct ReviewMove {
    registry: Registry,
    index: usize,
}

impl ReviewMove {
    pub(in crate::workspace) fn finish(
        mut self,
        destination: Option<&LockedTarget>,
    ) -> io::Result<()> {
        if let Some(destination) = destination {
            let metadata = fs::symlink_metadata(destination.target())?;
            if !self.registry.index.moving[self.index].matches(&metadata) {
                return Err(invalid(
                    "The destination file changed during checkpoint relocation.",
                ));
            }
        }
        self.registry.finish(self.index, destination.is_some())
    }
}

impl Workspace {
    pub(in crate::workspace) fn prepare_review_move(
        &self,
        private: &File,
        source: &LockedTarget,
        destination: &LockedTarget,
        metadata: &Metadata,
    ) -> io::Result<Option<ReviewMove>> {
        let source_key = self.journal_name(&source.directory, &source.name)?;
        let destination_key = self.journal_name(&destination.directory, &destination.name)?;
        if !Registry::has_index(private)? && Store::open(private, &source_key, false)?.is_none() {
            return Ok(None);
        }
        let mut registry = Registry::open(private)?;
        let Location::Store(name) = registry.locate(source, &source_key)? else {
            return Ok(None);
        };
        let Some(store) = Store::open(private, &name, false)? else {
            return Ok(None);
        };
        let mut review = store.load()?;
        if review.pending.is_some() {
            let disk = disk_revision(source)?;
            require_resolved(store.reconcile(source, &mut review, disk.as_deref())?)?;
        }
        if review.checkpoints.is_empty() {
            return Ok(None);
        }
        match fs::symlink_metadata(destination.target()) {
            Ok(_) => return Err(io::ErrorKind::AlreadyExists.into()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        if metadata.nlink() != 1 {
            return Err(invalid(
                "Checkpoint relocation requires a file with one directory entry.",
            ));
        }
        if registry.find_move(&destination_key).is_some() {
            return Err(invalid(
                "The destination is reserved by an interrupted checkpoint move.",
            ));
        }
        let index = registry.index.moving.len();
        registry.index.moving.push(Relocation {
            source: source_key,
            destination: destination_key,
            store: name,
            identity: identity(metadata),
        });
        registry.write()?;
        Ok(Some(ReviewMove { registry, index }))
    }
}
