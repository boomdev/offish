#![cfg(target_os = "linux")]

use offish::workspace::{Save, Workspace};
use std::{
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt, symlink},
    path::PathBuf,
    sync::{
        Arc, Barrier,
        atomic::{AtomicU64, Ordering},
    },
};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        loop {
            let dir = std::env::temp_dir().join(format!(
                "offish-tests-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&dir) {
                Ok(()) => return Self(dir),
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(err) => panic!("{err}"),
            }
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn workspace_lists_fixture_and_refuses_outside_paths() {
    let root = Fixture::new();
    let outside = Fixture::new();
    fs::write(outside.0.join("private.md"), "private").unwrap();
    fs::copy(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../plans/research/phase-0/fixtures/pilot/brief.md"
        ),
        root.0.join("brief.md"),
    )
    .unwrap();
    symlink(&outside.0, root.0.join("linked-folder")).unwrap();
    symlink(outside.0.join("private.md"), root.0.join("linked.md")).unwrap();
    let workspace = Workspace::open(&root.0).unwrap();
    assert_eq!(workspace.list("").unwrap()[0].kind, "markdown");
    assert!(
        workspace
            .read("brief.md")
            .unwrap()
            .text
            .unwrap()
            .contains("日本語")
    );
    for path in [
        "../private.md",
        "/etc/hosts",
        "linked.md",
        "linked-folder/private.md",
    ] {
        assert!(workspace.read(path).is_err(), "{path}");
    }
    assert!(workspace.list("linked-folder").is_err());
    assert!(workspace.save("linked.md", "", "rewrite").is_err());
    assert_eq!(fs::read(outside.0.join("private.md")).unwrap(), b"private");
}

#[test]
fn no_op_and_bounded_utf8_edits_preserve_unrelated_bytes_and_mode() {
    let root = Fixture::new();
    let path = root.0.join("sample.md");
    let workspace = Workspace::open(&root.0).unwrap();
    for original in [
        "# café\nEdit me\n日本語",
        "\u{feff}# café\r\nEdit me\r\nالعربية\r\n",
    ] {
        fs::write(&path, original).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        let metadata = fs::metadata(&path).unwrap();
        let base = workspace.read("sample.md").unwrap();
        assert!(matches!(
            workspace
                .save("sample.md", &base.revision, original)
                .unwrap(),
            Save::Saved { .. }
        ));
        assert_eq!(fs::metadata(&path).unwrap().ino(), metadata.ino());
        let changed = original.replace("Edit me", "Edited");
        let Save::Saved { revision } = workspace
            .save("sample.md", &base.revision, &changed)
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(fs::read(&path).unwrap(), changed.as_bytes());
        assert_eq!(fs::metadata(&path).unwrap().mode() & 0o777, 0o640);
        assert_eq!(workspace.read("sample.md").unwrap().revision, revision);
    }
}

#[test]
fn stale_save_returns_current_disk_and_leaves_it_untouched() {
    let root = Fixture::new();
    let path = root.0.join("brief.md");
    fs::write(&path, "original").unwrap();
    let workspace = Workspace::open(&root.0).unwrap();
    let base = workspace.read("brief.md").unwrap();
    fs::write(&path, "external").unwrap();
    let Save::Conflict { disk } = workspace.save("brief.md", &base.revision, "draft").unwrap()
    else {
        panic!()
    };
    assert_eq!(disk.text.as_deref(), Some("external"));
    assert_eq!(fs::read(&path).unwrap(), b"external");
}

#[test]
fn independent_workspace_handles_serialize_saves_against_the_same_base() {
    let root = Fixture::new();
    fs::write(root.0.join("brief.md"), "original").unwrap();
    let first = Workspace::open(&root.0).unwrap();
    let second = Workspace::open(&root.0).unwrap();
    let base = first.read("brief.md").unwrap().revision;
    let start = Arc::new(Barrier::new(2));
    let save = |workspace: Workspace, draft| {
        let base = base.clone();
        let start = start.clone();
        std::thread::spawn(move || {
            start.wait();
            workspace.save("brief.md", &base, draft).unwrap()
        })
    };
    let a = save(first, "first");
    let b = save(second, "second");
    let outcomes = [a.join().unwrap(), b.join().unwrap()];
    assert_eq!(
        outcomes
            .iter()
            .filter(|value| matches!(value, Save::Saved { .. }))
            .count(),
        1
    );
    assert_eq!(
        outcomes
            .iter()
            .filter(|value| matches!(value, Save::Conflict { .. }))
            .count(),
        1
    );
}

#[test]
fn invalid_utf8_and_unsupported_edit_targets_remain_unchanged() {
    let root = Fixture::new();
    fs::write(root.0.join("bad.md"), [0xff]).unwrap();
    fs::write(root.0.join("reference.txt"), "reference").unwrap();
    let workspace = Workspace::open(&root.0).unwrap();
    let bad = workspace.read("bad.md").unwrap();
    assert!(bad.text.is_none());
    assert!(workspace.save("bad.md", &bad.revision, "edit").is_err());
    let reference = workspace.read("reference.txt").unwrap();
    assert!(
        workspace
            .save("reference.txt", &reference.revision, "edit")
            .is_err()
    );
    assert_eq!(fs::read(root.0.join("bad.md")).unwrap(), [0xff]);
}

#[test]
fn explorer_creates_entries_without_replacing_existing_files() {
    let root = Fixture::new();
    let workspace = Workspace::open(&root.0).unwrap();
    workspace.create_entry("", "Notes", true).unwrap();
    workspace.create_entry("Notes", "brief.md", false).unwrap();
    assert_eq!(
        workspace.read("Notes/brief.md").unwrap().text.as_deref(),
        Some("")
    );
    fs::write(root.0.join("Notes/brief.md"), "original").unwrap();
    assert!(workspace.create_entry("Notes", "brief.md", false).is_err());
    assert!(workspace.create_entry("", "Notes", true).is_err());
    for name in ["../escaped.md", "sub/file", ".", "..", "", ".offish-save-x"] {
        assert!(workspace.create_entry("", name, false).is_err(), "{name}");
    }
    symlink("/tmp", root.0.join("outside")).unwrap();
    assert!(
        workspace
            .create_entry("outside", "escaped.md", false)
            .is_err()
    );
    assert_eq!(
        fs::read(root.0.join("Notes/brief.md")).unwrap(),
        b"original"
    );
}

#[test]
fn explorer_rename_keeps_identity_and_refuses_collisions_and_links() {
    let root = Fixture::new();
    let workspace = Workspace::open(&root.0).unwrap();
    fs::create_dir(root.0.join("Notes")).unwrap();
    fs::write(root.0.join("Notes/brief.md"), "original").unwrap();
    fs::write(root.0.join("Notes/occupied.md"), "occupied").unwrap();
    let base = workspace.read("Notes/brief.md").unwrap();
    assert!(
        workspace
            .rename_entry("Notes/brief.md", "occupied.md")
            .is_err()
    );
    assert!(
        workspace
            .rename_entry("Notes/brief.md", "../outside.md")
            .is_err()
    );
    assert_eq!(
        fs::read(root.0.join("Notes/occupied.md")).unwrap(),
        b"occupied"
    );
    workspace
        .rename_entry("Notes/brief.md", "révisé.md")
        .unwrap();
    workspace.rename_entry("Notes", "Archive").unwrap();
    let after = workspace.read("Archive/révisé.md").unwrap();
    assert_eq!(after.revision, base.revision);
    assert!(matches!(
        workspace
            .save("Archive/révisé.md", &base.revision, "draft")
            .unwrap(),
        Save::Saved { .. }
    ));
    assert_eq!(
        fs::read(root.0.join("Archive/révisé.md")).unwrap(),
        b"draft"
    );
    symlink("Archive/révisé.md", root.0.join("link")).unwrap();
    assert!(workspace.rename_entry("link", "renamed").is_err());
    assert!(
        fs::symlink_metadata(root.0.join("link"))
            .unwrap()
            .is_symlink()
    );
}

#[test]
fn explorer_moves_within_workspace_and_keeps_the_open_revision() {
    let root = Fixture::new();
    let workspace = Workspace::open(&root.0).unwrap();
    fs::create_dir(root.0.join("Notes")).unwrap();
    fs::create_dir(root.0.join("Archive")).unwrap();
    fs::write(root.0.join("Notes/brief.md"), "original").unwrap();
    fs::write(root.0.join("Archive/brief.md"), "occupied").unwrap();
    let base = workspace.read("Notes/brief.md").unwrap();

    assert!(workspace.move_entry("Notes/brief.md", "Archive").is_err());
    assert!(workspace.move_entry("Notes", "Notes").is_err());
    assert!(workspace.move_entry("Notes/brief.md", "..").is_err());
    assert_eq!(
        fs::read(root.0.join("Archive/brief.md")).unwrap(),
        b"occupied"
    );
    workspace.move_entry("Notes/brief.md", "").unwrap();
    assert_eq!(workspace.read("brief.md").unwrap().revision, base.revision);
    assert!(matches!(
        workspace
            .save("brief.md", &base.revision, "unsaved draft")
            .unwrap(),
        Save::Saved { .. }
    ));
    assert!(!root.0.join("Notes/brief.md").exists());
}

#[test]
fn explorer_duplicates_independent_trees_and_never_publishes_failed_copies() {
    let root = Fixture::new();
    let workspace = Workspace::open(&root.0).unwrap();
    fs::create_dir_all(root.0.join("Notes/sub")).unwrap();
    let original = root.0.join("Notes/sub/brief.md");
    fs::write(&original, "original").unwrap();
    workspace.duplicate_entry("Notes", "Copy").unwrap();
    workspace
        .duplicate_entry("Notes/sub/brief.md", "duplicate.md")
        .unwrap();
    let copied = root.0.join("Copy/sub/brief.md");
    assert_eq!(fs::read(&copied).unwrap(), b"original");
    assert_eq!(
        fs::read(root.0.join("Notes/sub/duplicate.md")).unwrap(),
        b"original"
    );
    assert_ne!(
        fs::metadata(&original).unwrap().ino(),
        fs::metadata(&copied).unwrap().ino()
    );
    fs::write(copied, "copy edit").unwrap();
    assert_eq!(fs::read(&original).unwrap(), b"original");
    assert!(workspace.duplicate_entry("Notes", "Copy").is_err());
    symlink("brief.md", root.0.join("Notes/sub/link")).unwrap();
    assert!(workspace.duplicate_entry("Notes", "Broken").is_err());
    assert!(!root.0.join("Broken").exists());
    assert_eq!(fs::read_dir(&root.0).unwrap().count(), 2);
}

#[test]
fn explorer_trash_survives_reopen_and_restores_without_replacing() {
    let root = Fixture::new();
    let private = Fixture::new();
    fs::set_permissions(&private.0, fs::Permissions::from_mode(0o700)).unwrap();
    let workspace = Workspace::open(&root.0)
        .unwrap()
        .with_recovery(private.0.clone());
    fs::create_dir(root.0.join("Notes")).unwrap();
    fs::write(root.0.join("Notes/brief.md"), "original").unwrap();
    let base = workspace.read("Notes/brief.md").unwrap();
    workspace.trash_entry("Notes").unwrap();
    assert!(!root.0.join("Notes").exists());
    drop(workspace);
    let reopened = Workspace::open(&root.0)
        .unwrap()
        .with_recovery(private.0.clone());
    let items = reopened.list_trash().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].path, "Notes");
    fs::create_dir(root.0.join("Notes")).unwrap();
    assert!(reopened.restore_entry(&items[0].id).is_err());
    assert!(root.0.join("Notes").is_dir());
    assert_eq!(reopened.list_trash().unwrap().len(), 1);
    fs::remove_dir(root.0.join("Notes")).unwrap();
    reopened.restore_entry(&items[0].id).unwrap();
    assert!(reopened.list_trash().unwrap().is_empty());
    assert_eq!(
        reopened.read("Notes/brief.md").unwrap().revision,
        base.revision
    );
    reopened.trash_entry("Notes/brief.md").unwrap();
    let file = reopened.list_trash().unwrap().pop().unwrap();
    assert_eq!(file.path, "Notes/brief.md");
    reopened.restore_entry(&file.id).unwrap();
    assert_eq!(
        reopened.read("Notes/brief.md").unwrap().revision,
        base.revision
    );
}
