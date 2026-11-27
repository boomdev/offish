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
