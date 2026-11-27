use super::*;

#[derive(Serialize)]
pub struct TrashEntry {
    pub id: String,
    pub path: String,
}

#[derive(Serialize, Deserialize)]
struct Origin {
    path: String,
}

impl Workspace {
    fn trash_directory(&self) -> io::Result<File> {
        let private = self
            .recovery_directory()?
            .ok_or_else(|| invalid("Private recovery storage is required for Trash."))?;
        let (device, inode) = identity(&self.root.metadata()?);
        let name = format!("trash-{device}-{inode}");
        let path = descriptor(&private).join(name);
        match DirBuilder::new().mode(0o700).create(&path) {
            Ok(()) => private.sync_all()?,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
        open(&path, true)
    }

    pub fn trash_entry(&self, path: &str) -> io::Result<Option<String>> {
        let (source, name) = self.parent(path)?;
        let name = checked_leaf(&name)?;
        let source = independent_directory(&source)?;
        source.lock()?;
        let metadata = ordinary_entry(&descriptor(&source).join(name))?;
        let trash = self.trash_directory()?;
        let stage = stage_directory(&trash)?;
        let slot = open(&stage.0, true)?;
        let mut origin = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(descriptor(&slot).join("origin.json"))?;
        serde_json::to_writer(&mut origin, &Origin { path: path.into() })?;
        origin.sync_all()?;
        // Publish the restore location before removing the workspace entry.
        slot.sync_all()?;
        trash.sync_all()?;
        if metadata.is_file() {
            if let Some(private) = self.recovery_directory()? {
                self.clear_committed_journal(&source, name, &private)?;
            }
        }
        rename_exclusive(&source, name, &slot, Path::new("entry")).map_err(trash_move_error)?;
        // The slot now owns user data. The staging guard must never remove it.
        stage.retain();
        let source_warning = entry_sync_warning(&source);
        let slot_warning = entry_sync_warning(&slot);
        Ok(source_warning.or(slot_warning))
    }

    pub fn list_trash(&self) -> io::Result<Vec<TrashEntry>> {
        let trash = self.trash_directory()?;
        let mut items = Vec::new();
        for entry in fs::read_dir(descriptor(&trash))? {
            let entry = entry?;
            let Ok(id) = entry.file_name().into_string() else {
                continue;
            };
            if !id.starts_with(".offish-save-") || !entry.file_type()?.is_dir() {
                continue;
            }
            let slot = open(&entry.path(), true)?;
            if !descriptor(&slot)
                .join("entry")
                .symlink_metadata()
                .is_ok_and(|entry| entry.is_file() || entry.is_dir())
            {
                continue;
            }
            let origin = read_origin(&slot)?;
            items.push(TrashEntry {
                id,
                path: origin.path,
            });
        }
        items.sort_by(|a, b| (&a.path, &a.id).cmp(&(&b.path, &b.id)));
        Ok(items)
    }

    pub fn restore_entry(&self, id: &str) -> io::Result<Option<String>> {
        if !id.starts_with(".offish-save-") || id.contains('/') {
            return Err(invalid("Invalid Trash entry."));
        }
        let trash = self.trash_directory()?;
        let slot = open(&descriptor(&trash).join(id), true)?;
        slot.lock()?;
        let origin = read_origin(&slot)?;
        let (destination, name) = self.parent(&origin.path)?;
        let name = checked_leaf(&name)?;
        let destination = independent_directory(&destination)?;
        destination.lock()?;
        let entry = ordinary_entry(&descriptor(&slot).join("entry"))?;
        if entry.is_file() {
            let private = self
                .recovery_directory()?
                .ok_or_else(|| invalid("Missing private recovery storage."))?;
            let journal = descriptor(&private).join(self.journal_name(&destination, name)?);
            if read_pending(&journal)?.is_some() {
                return Err(invalid("The original path has a recovery draft."));
            }
        }
        rename_exclusive(&slot, Path::new("entry"), &destination, name)
            .map_err(trash_move_error)?;
        let destination_warning = entry_sync_warning(&destination);
        let slot_warning = entry_sync_warning(&slot);
        let warning = destination_warning.or(slot_warning);
        if warning.is_none() {
            // Preserve the origin record until the destination is synced.
            let _ = fs::remove_file(descriptor(&slot).join("origin.json"));
            let _ = fs::remove_dir(descriptor(&trash).join(id));
            let _ = trash.sync_all();
        }
        Ok(warning)
    }
}

fn read_origin(directory: &File) -> io::Result<Origin> {
    let file = open(&descriptor(directory).join("origin.json"), false)?;
    serde_json::from_reader(file).map_err(io::Error::from)
}

fn trash_move_error(error: io::Error) -> io::Error {
    if error.raw_os_error() == Some(libc::EXDEV) {
        return io::Error::new(
            error.kind(),
            "Trash requires the workspace and recovery folder to share a filesystem that supports renaming between them.",
        );
    }
    error
}
