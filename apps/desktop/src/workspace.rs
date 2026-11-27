use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    ffi::{CStr, CString},
    fs::{self, DirBuilder, File, Metadata, OpenOptions},
    io::{self, Read, Write},
    os::{
        fd::AsRawFd,
        unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
    },
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const LIMIT: u64 = 16 * 1024 * 1024;
static SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn invalid(error: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, error)
}

fn descriptor(file: &File) -> PathBuf {
    PathBuf::from(format!("/proc/self/fd/{}", file.as_raw_fd()))
}

fn open(path: &Path, directory: bool) -> io::Result<File> {
    let mut flags = libc::O_NOFOLLOW | libc::O_NONBLOCK;
    if directory {
        flags |= libc::O_DIRECTORY;
    }
    OpenOptions::new().read(true).custom_flags(flags).open(path)
}

#[derive(Debug, Serialize)]
pub struct Entry {
    pub name: String,
    pub kind: &'static str,
}

#[derive(Debug, Serialize)]
pub struct Document {
    pub revision: String,
    pub text: Option<String>,
    pub reason: Option<&'static str>,
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Save {
    Saved { revision: String },
    Conflict { disk: Document },
}

pub struct Workspace {
    root: File,
    recovery: Option<PathBuf>,
    pub name: String,
}

impl Workspace {
    pub fn open(root: &Path) -> io::Result<Self> {
        let path = root.canonicalize()?;
        let directory = open(&path, true)?;
        Ok(Self {
            root: directory,
            recovery: None,
            name: path.display().to_string(),
        })
    }

    pub fn with_recovery(mut self, path: PathBuf) -> Self {
        self.recovery = Some(path);
        self
    }

    fn recovery_directory(&self) -> io::Result<Option<File>> {
        let Some(path) = &self.recovery else {
            return Ok(None);
        };
        // Check the intended location before creating anything in the opened folder.
        let root = fs::read_link(descriptor(&self.root))?;
        let mut ancestor = path.as_path();
        while !ancestor.exists() {
            ancestor = ancestor
                .parent()
                .ok_or_else(|| invalid("Invalid recovery directory."))?;
        }
        let location = ancestor
            .canonicalize()?
            .join(path.strip_prefix(ancestor).map_err(io::Error::other)?);
        if location.starts_with(&root) {
            return Err(invalid(
                "Private recovery storage is inside this workspace. Open a narrower folder.",
            ));
        }
        DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&location)?;
        let directory = open(&location, true)?;
        let metadata = directory.metadata()?;
        if metadata.uid() != unsafe { libc::geteuid() } || metadata.mode() & 0o077 != 0 {
            return Err(invalid(
                "Recovery directory must be private to the current user.",
            ));
        }
        Ok(Some(directory))
    }

    fn journal_name(&self, directory: &File, name: &Path) -> io::Result<String> {
        // An overlapping workspace can reach the same document. Key the journal
        // by its containing directory identity and filename.
        let metadata = directory.metadata()?;
        let key = format!("{}:{}:{}", metadata.dev(), metadata.ino(), name.display());
        Ok(format!("{}.json", checksum(key.as_bytes())))
    }

    pub fn recovery_draft(&self, path: &str) -> io::Result<Option<String>> {
        let (parent, name) = self.parent(path)?;
        let Some(directory) = self.recovery_directory()? else {
            return Ok(None);
        };
        let target = descriptor(&parent).join(&name);
        let journal = descriptor(&directory).join(self.journal_name(&parent, &name)?);
        let Some(pending) = read_pending(&journal)? else {
            return Ok(None);
        };
        match snapshot(&target) {
            Ok((_, bytes)) if bytes == pending.text.as_bytes() => Ok(None),
            Ok(_) => Ok(Some(pending.text)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Some(pending.text)),
            Err(error) => Err(error),
        }
    }

    pub fn discard_recovery(&self, path: &str, draft: &str) -> io::Result<()> {
        let (parent, name) = self.parent(path)?;
        let parent = open(&descriptor(&parent).join("."), true)?;
        parent.lock()?;
        if let Some(directory) = self.recovery_directory()? {
            let journal = descriptor(&directory).join(self.journal_name(&parent, &name)?);
            if read_pending(&journal)?.is_some_and(|pending| pending.text == draft) {
                fs::remove_file(journal)?;
                directory.sync_all()?;
            }
        }
        Ok(())
    }

    fn directory(&self, path: &Path) -> io::Result<File> {
        let mut current = self.root.try_clone()?;
        for part in path.components() {
            let Component::Normal(name) = part else {
                return Err(invalid("Choose a path inside the workspace."));
            };
            current = open(&descriptor(&current).join(name), true)?;
        }
        Ok(current)
    }

    fn parent(&self, path: &str) -> io::Result<(File, PathBuf)> {
        let path = Path::new(path);
        let name = path.file_name().ok_or_else(|| invalid("Choose a file."))?;
        let directory = self.directory(path.parent().unwrap_or(Path::new("")))?;
        Ok((directory, name.into()))
    }

    pub fn list(&self, path: &str) -> io::Result<Vec<Entry>> {
        let directory = self.directory(Path::new(path))?;
        let mut entries = Vec::new();
        for item in fs::read_dir(descriptor(&directory))? {
            let item = item?;
            let Ok(name) = item.file_name().into_string() else {
                continue;
            };
            if name.starts_with(".offish-save-") {
                continue;
            }
            let file_type = item.file_type()?;
            let kind = if file_type.is_dir() {
                "folder"
            } else if file_type.is_file() && is_markdown(Path::new(&name)) {
                "markdown"
            } else {
                "reference"
            };
            entries.push(Entry { name, kind });
        }
        entries.sort_by(|a, b| (a.kind != "folder", &a.name).cmp(&(b.kind != "folder", &b.name)));
        Ok(entries)
    }

    pub fn read(&self, path: &str) -> io::Result<Document> {
        let (directory, name) = self.parent(path)?;
        let (metadata, bytes) = snapshot(&descriptor(&directory).join(&name))?;
        Ok(document(&metadata, bytes))
    }

    pub fn save(&self, path: &str, base: &str, text: &str) -> io::Result<Save> {
        self.save_with_hook(path, base, text, |_| Ok(()))
    }

    fn save_with_hook(
        &self,
        path: &str,
        base: &str,
        text: &str,
        mut hook: impl FnMut(SaveStep) -> io::Result<()>,
    ) -> io::Result<Save> {
        if !is_markdown(Path::new(path))
            || text.len() as u64 > LIMIT
            || text.as_bytes().contains(&0)
        {
            return Err(invalid("Only Markdown files up to 16 MiB are editable."));
        }
        let (directory, name) = self.parent(path)?;
        // Open an independent descriptor for the lock, including for root files.
        let directory = open(&descriptor(&directory).join("."), true)?;
        directory.lock()?;
        let target = descriptor(&directory).join(&name);
        let (metadata, bytes) = snapshot(&target)?;
        if revision(&metadata, &bytes) != base {
            return Ok(Save::Conflict {
                disk: document(&metadata, bytes),
            });
        }
        if std::str::from_utf8(&bytes).is_err() || bytes.contains(&0) {
            return Err(invalid("This file cannot be edited as UTF-8 Markdown."));
        }
        if text.as_bytes() == bytes {
            return Ok(Save::Saved {
                revision: base.into(),
            });
        }
        if metadata.nlink() != 1 {
            return Err(invalid(
                "Saving files with multiple hard links is unsupported.",
            ));
        }

        let recovery = self.recovery_directory()?;
        let journal = recovery
            .as_ref()
            .map(|recovery| {
                self.journal_name(&directory, &name)
                    .map(|name| descriptor(recovery).join(name))
            })
            .transpose()?;
        if let Some(journal) = &journal {
            if let Some(pending) = read_pending(journal)? {
                if pending.text != text && pending.text.as_bytes() != bytes {
                    return Err(invalid(
                        "Review the previous interrupted save before replacing its draft.",
                    ));
                }
            }
        }
        let (mut staged, stage) = stage_file(&directory)?;
        hook(SaveStep::StageWrite)?;
        staged.write_all(text.as_bytes())?;
        let source = open(&target, false)?;
        if source.metadata()?.ino() != metadata.ino() || source.metadata()?.dev() != metadata.dev()
        {
            let (current, bytes) = snapshot(&target)?;
            return Ok(Save::Conflict {
                disk: document(&current, bytes),
            });
        }
        preserve_metadata(&source, &staged, &metadata)?;
        staged.sync_all()?;
        if let (Some(recovery), Some(journal)) = (&recovery, &journal) {
            let (mut file, pending) = stage_file(recovery)?;
            serde_json::to_writer(&mut file, &Pending { text: text.into() })?;
            file.sync_all()?;
            pending.replace(journal)?;
            recovery.sync_all()?;
        }
        hook(SaveStep::JournalSynced)?;
        let (current, bytes) = snapshot(&target)?;
        if revision(&current, &bytes) != base {
            return Ok(Save::Conflict {
                disk: document(&current, bytes),
            });
        }
        if (current.ctime(), current.ctime_nsec()) != (metadata.ctime(), metadata.ctime_nsec()) {
            return Err(io::Error::other(
                "File metadata changed while saving. Try again.",
            ));
        }
        // A non-cooperating external writer can still race this final check.
        stage.replace(&target)?;
        hook(SaveStep::Replaced)?;
        directory.sync_all()?;
        if let (Some(recovery), Some(journal)) = (&recovery, &journal) {
            // If cleanup fails, reading the journal can still recognize the committed bytes.
            if fs::remove_file(journal).is_ok() {
                let _ = recovery.sync_all();
            }
        }
        Ok(Save::Saved {
            revision: revision(&staged.metadata()?, text.as_bytes()),
        })
    }
}

#[derive(PartialEq, Eq)]
enum SaveStep {
    StageWrite,
    JournalSynced,
    Replaced,
}

#[derive(Deserialize, Serialize)]
struct Pending {
    text: String,
}

fn read_pending(path: &Path) -> io::Result<Option<Pending>> {
    match open(path, false) {
        Ok(file) => Ok(Some(serde_json::from_reader(file)?)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn stage_file(directory: &File) -> io::Result<(File, Staged)> {
    let parent = descriptor(directory);
    loop {
        let path = parent.join(format!(
            ".offish-save-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
        {
            Ok(file) => return Ok((file, Staged(Some(path)))),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
}

struct Staged(Option<PathBuf>);
impl Staged {
    fn replace(mut self, target: &Path) -> io::Result<()> {
        fs::rename(self.0.as_ref().expect("active stage"), target)?;
        self.0 = None;
        Ok(())
    }
}
impl Drop for Staged {
    fn drop(&mut self) {
        if let Some(path) = &self.0 {
            let _ = fs::remove_file(path);
        }
    }
}

fn is_markdown(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown"))
}

fn snapshot(path: &Path) -> io::Result<(Metadata, Vec<u8>)> {
    let file = open(path, false)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(invalid("Choose an ordinary file."));
    }
    let mut bytes = Vec::new();
    (&file).take(LIMIT + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > LIMIT {
        return Err(invalid("File exceeds the 16 MiB editor limit."));
    }
    let after = file.metadata()?;
    if metadata.len() != after.len()
        || metadata.mtime() != after.mtime()
        || metadata.mtime_nsec() != after.mtime_nsec()
        || metadata.ctime() != after.ctime()
        || metadata.ctime_nsec() != after.ctime_nsec()
    {
        return Err(io::Error::other(
            "File changed while being read. Try again.",
        ));
    }
    Ok((after, bytes))
}

fn checksum(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut text = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        write!(&mut text, "{byte:02x}").expect("write to string");
    }
    text
}

fn revision(metadata: &Metadata, bytes: &[u8]) -> String {
    format!("{}:{}:{}", metadata.dev(), metadata.ino(), checksum(bytes))
}

fn document(metadata: &Metadata, bytes: Vec<u8>) -> Document {
    let revision = revision(metadata, &bytes);
    let text = String::from_utf8(bytes)
        .ok()
        .filter(|value| !value.contains('\0'));
    let reason = text
        .is_none()
        .then_some("This file is not editable UTF-8 text.");
    Document {
        revision,
        text,
        reason,
    }
}

fn preserve_metadata(source: &File, staged: &File, metadata: &Metadata) -> io::Result<()> {
    let fd = staged.as_raw_fd();
    let current = staged.metadata()?;
    if (current.uid(), current.gid()) != (metadata.uid(), metadata.gid()) {
        if unsafe { libc::fchown(fd, metadata.uid(), metadata.gid()) } != 0 {
            return Err(io::Error::last_os_error());
        }
    }
    let wanted = attributes(source)?;
    let inherited = attributes(staged)?;
    for name in inherited.keys() {
        if !wanted.contains_key(name) && unsafe { libc::fremovexattr(fd, name.as_ptr()) } != 0 {
            return Err(io::Error::last_os_error());
        }
    }
    for (name, value) in &wanted {
        if inherited.get(name) != Some(value) {
            set_attribute(staged, name, value)?;
        }
    }
    if unsafe { libc::fchmod(fd, metadata.mode() & 0o7777) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let result = staged.metadata()?;
    if (result.uid(), result.gid(), result.mode() & 0o7777)
        != (metadata.uid(), metadata.gid(), metadata.mode() & 0o7777)
        || attributes(staged)? != wanted
    {
        return Err(io::Error::other(
            "The filesystem could not preserve the file metadata.",
        ));
    }
    Ok(())
}

fn attributes(file: &File) -> io::Result<BTreeMap<CString, Vec<u8>>> {
    let fd = file.as_raw_fd();
    let names =
        read_attribute_bytes(|buffer, size| unsafe { libc::flistxattr(fd, buffer.cast(), size) })?;
    let mut result = BTreeMap::new();
    for name in names
        .split(|&byte| byte == 0)
        .filter(|name| !name.is_empty())
    {
        if !name.starts_with(b"user.") && name != b"system.posix_acl_access" {
            return Err(invalid(
                "The file has an unsupported security or filesystem attribute. The draft remains unsaved.",
            ));
        }
        let name = CString::new(name).map_err(io::Error::other)?;
        let value = read_attribute_bytes(|buffer, size| unsafe {
            libc::fgetxattr(fd, name.as_ptr(), buffer, size)
        })?;
        result.insert(name, value);
    }
    Ok(result)
}

fn read_attribute_bytes(read: impl Fn(*mut libc::c_void, usize) -> isize) -> io::Result<Vec<u8>> {
    let size = read(std::ptr::null_mut(), 0);
    if size < 0 {
        return Err(io::Error::last_os_error());
    }
    if size == 0 {
        return Ok(Vec::new());
    }
    let mut bytes = vec![0; size as usize];
    let count = read(bytes.as_mut_ptr().cast(), bytes.len());
    if count < 0 {
        return Err(io::Error::last_os_error());
    }
    bytes.truncate(count as usize);
    Ok(bytes)
}

fn set_attribute(file: &File, name: &CStr, value: &[u8]) -> io::Result<()> {
    let status = unsafe {
        libc::fsetxattr(
            file.as_raw_fd(),
            name.as_ptr(),
            value.as_ptr().cast(),
            value.len(),
            0,
        )
    };
    if status != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interrupted_save_recovers_the_draft_without_overwriting_disk() {
        const CHILD_ROOT: &str = "OFFISH_TEST_INTERRUPTED_SAVE_ROOT";
        const CHILD_STEP: &str = "OFFISH_TEST_INTERRUPTED_SAVE_STEP";
        if let Some(root) = std::env::var_os(CHILD_ROOT) {
            let root = PathBuf::from(root);
            let workspace = Workspace::open(&root.join("workspace"))
                .unwrap()
                .with_recovery(root.join("private"));
            let initial = workspace.read("brief.md").unwrap();
            workspace
                .save_with_hook("brief.md", &initial.revision, "draft", |step| {
                    let at = if std::env::var(CHILD_STEP).as_deref() == Ok("replaced") {
                        SaveStep::Replaced
                    } else {
                        SaveStep::JournalSynced
                    };
                    if at == step {
                        std::process::exit(73);
                    }
                    Ok(())
                })
                .unwrap();
            panic!("interruption hook was not called");
        }

        let base = std::env::temp_dir().join(format!("offish-recovery-{}", std::process::id()));
        fs::create_dir(&base).unwrap();
        let _cleanup = TestDirectory(&base);
        let root = base.join("workspace");
        let private = base.join("private");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("brief.md"), "original").unwrap();
        let child = |step| {
            std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "workspace::tests::interrupted_save_recovers_the_draft_without_overwriting_disk",
            ])
            .env(CHILD_ROOT, &base)
            .env(CHILD_STEP, step)
            .output()
            .unwrap()
        };
        let before = child("journal");
        assert_eq!(before.status.code(), Some(73), "{before:?}");
        assert_eq!(fs::read(root.join("brief.md")).unwrap(), b"original");
        let journal = fs::read_dir(&private).unwrap().next().unwrap().unwrap();
        assert_eq!(journal.metadata().unwrap().mode() & 0o777, 0o600);
        fs::write(root.join("brief.md"), "external").unwrap();
        let reopened = Workspace::open(&root).unwrap().with_recovery(private);
        assert_eq!(
            reopened.recovery_draft("brief.md").unwrap().as_deref(),
            Some("draft")
        );
        assert_eq!(reopened.list("").unwrap().len(), 1);
        let disk = reopened.read("brief.md").unwrap();
        assert_eq!(disk.text.as_deref(), Some("external"));
        assert!(
            reopened
                .save("brief.md", &disk.revision, "another draft")
                .is_err()
        );
        assert!(matches!(
            reopened.save("brief.md", &disk.revision, "draft").unwrap(),
            Save::Saved { .. }
        ));
        assert!(reopened.recovery_draft("brief.md").unwrap().is_none());
        assert_eq!(fs::read(root.join("brief.md")).unwrap(), b"draft");

        fs::write(root.join("brief.md"), "next original").unwrap();
        let replaced = child("replaced");
        assert_eq!(replaced.status.code(), Some(73), "{replaced:?}");
        assert_eq!(fs::read(root.join("brief.md")).unwrap(), b"draft");
        assert!(reopened.recovery_draft("brief.md").unwrap().is_none());
        let disk = reopened.read("brief.md").unwrap();
        assert!(matches!(
            reopened
                .save("brief.md", &disk.revision, "next edit")
                .unwrap(),
            Save::Saved { .. }
        ));
    }

    #[test]
    fn simulated_io_failures_preserve_disk_and_any_durable_draft() {
        let base = std::env::temp_dir().join(format!("offish-io-{}", std::process::id()));
        fs::create_dir(&base).unwrap();
        let _cleanup = TestDirectory(&base);
        let root = base.join("workspace");
        fs::create_dir(&root).unwrap();
        let path = root.join("brief.md");
        fs::write(&path, "original").unwrap();
        let workspace = Workspace::open(&root)
            .unwrap()
            .with_recovery(base.join("private"));
        let initial = workspace.read("brief.md").unwrap();

        for (stage, errno, draft) in [
            (SaveStep::StageWrite, libc::ENOSPC, None),
            (SaveStep::JournalSynced, libc::EACCES, Some("draft")),
        ] {
            let error = workspace
                .save_with_hook("brief.md", &initial.revision, "draft", |step| {
                    if step == stage {
                        return Err(io::Error::from_raw_os_error(errno));
                    }
                    Ok(())
                })
                .err()
                .expect("the injected failure must prevent saving");
            assert_eq!(error.raw_os_error(), Some(errno));
            assert_eq!(fs::read(&path).unwrap(), b"original");
            assert_eq!(
                workspace.recovery_draft("brief.md").unwrap().as_deref(),
                draft
            );
        }
    }

    #[test]
    fn ordinary_save_preserves_user_attributes_and_access_acl() {
        let base = std::env::temp_dir().join(format!("offish-metadata-{}", std::process::id()));
        fs::create_dir(&base).unwrap();
        let _cleanup = TestDirectory(&base);
        let target = base.join("brief.md");
        fs::write(&target, "original").unwrap();
        let before = open(&target, false).unwrap();
        set_attribute(&before, c"user.offish_test", b"metadata").unwrap();
        // Linux POSIX ACL xattr v2, including one named user followed by a mask.
        let uid = unsafe { libc::geteuid() };
        let mut acl = 2u32.to_le_bytes().to_vec();
        for (tag, permissions, id) in [
            (1u16, 6u16, u32::MAX),
            (2, 4, uid),
            (4, 0, u32::MAX),
            (16, 4, u32::MAX),
            (32, 0, u32::MAX),
        ] {
            acl.extend(tag.to_le_bytes());
            acl.extend(permissions.to_le_bytes());
            acl.extend(id.to_le_bytes());
        }
        set_attribute(&before, c"system.posix_acl_access", &acl).unwrap();
        let expected = attributes(&before).unwrap();
        let metadata = before.metadata().unwrap();
        let workspace = Workspace::open(&base).unwrap();
        let initial = workspace.read("brief.md").unwrap();
        assert!(matches!(
            workspace
                .save("brief.md", &initial.revision, "saved")
                .unwrap(),
            Save::Saved { .. }
        ));
        let after = open(&target, false).unwrap();
        assert_eq!(attributes(&after).unwrap(), expected);
        let result = after.metadata().unwrap();
        assert_eq!(
            (result.uid(), result.gid(), result.mode()),
            (metadata.uid(), metadata.gid(), metadata.mode())
        );
        assert_eq!(fs::read(target).unwrap(), b"saved");

        let plain = base.join("plain.md");
        fs::write(&plain, "plain").unwrap();
        assert!(
            attributes(&open(&plain, false).unwrap())
                .unwrap()
                .is_empty()
        );
        let parent = open(&base, true).unwrap();
        set_attribute(&parent, c"system.posix_acl_default", &acl).unwrap();
        let (inherited, _stage) = stage_file(&parent).unwrap();
        assert!(
            attributes(&inherited)
                .unwrap()
                .contains_key(c"system.posix_acl_access")
        );
        let initial = workspace.read("plain.md").unwrap();
        assert!(matches!(
            workspace
                .save("plain.md", &initial.revision, "new plain")
                .unwrap(),
            Save::Saved { .. }
        ));
        assert!(
            attributes(&open(&plain, false).unwrap())
                .unwrap()
                .is_empty()
        );
    }

    struct TestDirectory<'a>(&'a Path);
    impl Drop for TestDirectory<'_> {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(self.0);
        }
    }
}
