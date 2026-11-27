#![cfg(target_os = "linux")]

use offish::workspace::search::{SearchIndex, SearchRoot};
use offish::workspace::{Save, Workspace};
use offish::workspaces::{ActiveWorkspace, CatalogStore, canonical_roots};
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
fn search_respects_root_boundaries_nested_rules_and_private_storage() {
    let fixture = Fixture::new();
    let outside = Fixture::new();
    let root = fixture.0.join("one");
    let second = fixture.0.join("two");
    let private = root.join("app-config");
    for dir in [
        &root,
        &second,
        &private,
        &root.join("nested"),
        &root.join(".git"),
        &root.join("scratch"),
    ] {
        fs::create_dir(dir).unwrap();
    }
    fs::write(
        root.join(".gitignore"),
        "*.tmp\n/scratch/\n!keep.tmp\n**/log?.txt\n",
    )
    .unwrap();
    fs::write(root.join(".offishignore"), "*.[xy]\n\\#hidden.txt\n").unwrap();
    fs::write(root.join("nested/.gitignore"), "!again.tmp\n").unwrap();
    for path in [
        "brief.md",
        "keep.tmp",
        "nested/again.tmp",
        "scratch/notes.md",
        "gone.tmp",
        "nested/gone.tmp",
        "nested/log1.txt",
        "app-config/secrets.txt",
        ".git/metadata",
        "excluded.x",
        "#hidden.txt",
        ".offish-save-session",
    ] {
        fs::write(root.join(path), "a Finding").unwrap();
    }
    fs::write(second.join("brief.md"), "A finding").unwrap();
    fs::write(outside.0.join("outside.md"), "a Finding").unwrap();
    symlink(&outside.0, root.join("symlink-dir")).unwrap();
    symlink(outside.0.join("outside.md"), root.join("symlink.md")).unwrap();

    let roots = [(&root, 0), (&second, 2), (&private, 3)]
        .map(|(path, index)| SearchRoot::new(index, &Workspace::open(path).unwrap()).unwrap());
    let mut search = SearchIndex::default();
    for rebuild in [false, true] {
        let report = search
            .search(&roots, std::slice::from_ref(&private), "FINDING", rebuild)
            .unwrap();
        let paths: Vec<_> = report.results.iter().map(|hit| hit.path.as_str()).collect();
        assert_eq!(
            paths,
            [
                "root-0/brief.md",
                "root-0/keep.tmp",
                "root-0/nested/again.tmp",
                "root-2/brief.md"
            ]
        );
        assert!(!report.limited);
    }

    fs::write(root.join("nested/.offishignore"), "/again.tmp\n").unwrap();
    let refreshed = search.search(&roots, &[private], "finding", false).unwrap();
    assert!(
        !refreshed
            .results
            .iter()
            .any(|hit| hit.path.ends_with("/again.tmp"))
    );
}

#[test]
fn search_updates_metadata_cache_and_reports_editor_coordinates() {
    let fixture = Fixture::new();
    let path = fixture.0.join("brief.md");
    fs::write(&path, "\u{feff}😀 NÉEDLE\r\nsecond line\n").unwrap();
    fs::write(fixture.0.join("invalid.md"), [0xff, 0xfe]).unwrap();
    fs::write(fixture.0.join("nul.md"), "needle\0").unwrap();
    let root = Workspace::open(&fixture.0).unwrap();
    let roots = [SearchRoot::new(0, &root).unwrap()];
    let mut search = SearchIndex::default();
    let first = search.search(&roots, &[], "néedle", false).unwrap();
    assert_eq!(first.results.len(), 1);
    assert_eq!(first.skipped, 2);
    assert_eq!((first.results[0].line, first.results[0].column), (1, 3));
    assert_eq!(first.results[0].matched, "NÉEDLE");

    fs::write(&path, "first\r\n😀 NÉEDLE NÉEDLE\r\n").unwrap();
    let updated = search.search(&roots, &[], "néedle", false).unwrap();
    assert_eq!(
        updated
            .results
            .iter()
            .map(|hit| (hit.line, hit.column))
            .collect::<Vec<_>>(),
        [(2, 3), (2, 10)]
    );
    fs::remove_file(&path).unwrap();
    assert!(
        search
            .search(&roots, &[], "néedle", false)
            .unwrap()
            .results
            .is_empty()
    );
    fs::write(fixture.0.join("new.md"), "néedle\n".repeat(300)).unwrap();
    let limited = search.search(&roots, &[], "néedle", true).unwrap();
    assert_eq!(limited.results.len(), 250);
    assert!(limited.more_matches);
}

#[test]
#[ignore = "run explicitly for the P1-T06 timing fixture"]
fn search_timing_fixture() {
    use std::time::Instant;

    let fixture = Fixture::new();
    let content = "Local workspace search fixture. Topic: example.\n".repeat(20);
    let with_match = format!("{content}needle\n");
    for i in 0..10_000 {
        let text = if i % 100 == 0 { &with_match } else { &content };
        fs::write(fixture.0.join(format!("file-{i:05}.md")), text).unwrap();
    }
    let workspace = Workspace::open(&fixture.0).unwrap();
    let roots = [SearchRoot::new(0, &workspace).unwrap()];
    let mut lists = Vec::new();
    for _ in 0..20 {
        let start = Instant::now();
        let listed = Workspace::open(&fixture.0).unwrap().list("").unwrap();
        lists.push(start.elapsed());
        assert_eq!(listed.len(), 10_000);
    }
    lists.sort_unstable();
    let mut search = SearchIndex::default();
    let start = Instant::now();
    let cold = search.search(&roots, &[], "absent", true).unwrap();
    assert_eq!(cold.files, 10_000);
    let cold_duration = start.elapsed();
    let mut warm = Vec::new();
    for i in 0..20 {
        let (query, matches) = if i % 2 == 0 {
            ("needle", 100)
        } else {
            ("absent", 0)
        };
        let start = Instant::now();
        let report = search.search(&roots, &[], query, false).unwrap();
        warm.push(start.elapsed());
        assert_eq!(report.files, 10_000);
        assert_eq!(report.results.len(), matches);
        assert!(!report.limited);
    }
    warm.sort_unstable();
    eprintln!(
        "10,000 files. Cold cache {cold_duration:?}. Warm p50 {:?}, p95 {:?}.",
        warm[9], warm[18]
    );
    eprintln!(
        "Backend root open and list p50 {:?}, p95 {:?}.",
        lists[9], lists[18],
    );
    if let Ok(status) = fs::read_to_string("/proc/self/status") {
        if let Some(peak) = status.lines().find(|line| line.starts_with("VmHWM:")) {
            eprintln!("Backend test process peak RSS {peak}");
        }
    }
}

#[test]
fn workspace_catalog_restores_order_and_keeps_saved_entries_when_closed() {
    let fixture = Fixture::new();
    let first = fixture.0.join("first");
    let second = fixture.0.join("second");
    let alias = fixture.0.join("alias");
    fs::create_dir(&first).unwrap();
    fs::create_dir(&second).unwrap();
    symlink(&first, &alias).unwrap();
    let config = fixture.0.join("config");
    let store = CatalogStore::new(config.clone());
    assert!(store.load().unwrap().saved.is_empty());

    let first_roots = canonical_roots(vec![first.clone(), alias]).unwrap();
    assert_eq!(first_roots, [first.canonicalize().unwrap()]);
    let original = store
        .update(|catalog| catalog.remember(first_roots))
        .unwrap();
    let roots = canonical_roots(vec![second.clone(), first.clone()]).unwrap();
    let added = store
        .update(|catalog| catalog.add_roots(&original, roots.clone()))
        .unwrap();
    assert_eq!(added.id, original.id);
    assert_eq!(
        added.roots,
        [
            first.canonicalize().unwrap(),
            second.canonicalize().unwrap()
        ]
    );
    let other = store
        .update(|catalog| catalog.remember(vec![second.canonicalize()?]))
        .unwrap();

    let reopened = CatalogStore::new(config);
    assert_eq!(reopened.load().unwrap().recent(), Some(&other));
    assert_eq!(
        reopened.update(|catalog| catalog.reopen(added.id)).unwrap(),
        added
    );
    assert_eq!(reopened.load().unwrap().recent(), Some(&added));
    reopened
        .update(|catalog| {
            catalog.last = None;
            Ok(())
        })
        .unwrap();
    let catalog = reopened.load().unwrap();
    assert!(catalog.recent().is_none());
    assert_eq!(catalog.saved, [added, other]);
}

#[test]
fn workspace_rename_persists_without_changing_identity_or_roots() {
    let fixture = Fixture::new();
    let first = fixture.0.join("first");
    let second = fixture.0.join("second");
    fs::create_dir(&first).unwrap();
    fs::create_dir(&second).unwrap();
    let config = fixture.0.join("config");
    let store = CatalogStore::new(config.clone());
    let original = store
        .update(|catalog| catalog.remember(canonical_roots(vec![first.clone()])?))
        .unwrap();
    // Existing settings omit the optional name and retain the inferred label.
    assert_eq!(store.load().unwrap().recent().unwrap().name(), "first");
    assert!(
        !fs::read_to_string(config.join("workspaces.json"))
            .unwrap()
            .contains("custom_name")
    );

    assert_eq!(
        store
            .update(|catalog| catalog.rename(original.id, "  Notes & Plans  "))
            .unwrap(),
        "Notes & Plans"
    );
    for invalid in ["   ", "split\nname"] {
        assert!(
            store
                .update(|catalog| catalog.rename(original.id, invalid))
                .is_err()
        );
    }
    let reopened = CatalogStore::new(config);
    let recent = reopened.load().unwrap().recent().unwrap().clone();
    assert_eq!(recent.id, original.id);
    assert_eq!(recent.roots, original.roots);
    assert_eq!(recent.name(), "Notes & Plans");
    let added = reopened
        .update(|catalog| catalog.add_roots(&recent, canonical_roots(vec![second])?))
        .unwrap();
    assert_eq!(added.name(), "Notes & Plans");
    let saved = reopened
        .update(|catalog| catalog.reopen(original.id))
        .unwrap();
    assert_eq!(saved, added);
}

#[test]
fn multiple_roots_route_same_named_files_and_keep_open_handles_on_append() {
    let fixture = Fixture::new();
    let first = fixture.0.join("first");
    let second = fixture.0.join("second");
    fs::create_dir(&first).unwrap();
    fs::create_dir(&second).unwrap();
    fs::write(first.join("brief.md"), "first root").unwrap();
    fs::write(second.join("brief.md"), "second root").unwrap();
    let store = CatalogStore::new(fixture.0.join("config"));
    let recovery = fixture.0.join("recovery");
    let original = store
        .update(|catalog| catalog.remember(canonical_roots(vec![first.clone()])?))
        .unwrap();
    let mut active = ActiveWorkspace::open(original.clone(), &recovery);
    let added = store
        .update(|catalog| catalog.add_roots(&original, canonical_roots(vec![second.clone()])?))
        .unwrap();
    fs::rename(&first, fixture.0.join("previous-first")).unwrap();
    symlink(&second, &first).unwrap();
    active.extend(added.clone(), &recovery);

    for (index, expected) in ["first root", "second root"].into_iter().enumerate() {
        let path = format!("root-{index}/brief.md");
        let (_, root, relative) = active.route(&path).unwrap();
        assert_eq!(root.read(relative).unwrap().text.as_deref(), Some(expected));
    }
    assert_eq!(
        active
            .selected_file(&second.canonicalize().unwrap().join("brief.md"))
            .unwrap()
            .as_deref(),
        Some("root-1/brief.md")
    );
    assert!(active.route("root-2/brief.md").is_err());
    let (_, root, outside) = active.route("root-0/../second/brief.md").unwrap();
    assert!(root.read(outside).is_err());

    let reopened = ActiveWorkspace::open(added, &recovery);
    let entries = reopened.root_entries();
    assert_eq!(entries[0].kind, "unavailable");
    assert_eq!(entries[1].label.as_deref(), Some("second"));
    assert!(reopened.route("root-0/brief.md").is_err());
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

#[test]
fn native_file_selection_keeps_paths_workspace_relative_and_rejects_symlinks() {
    let root = Fixture::new();
    let other = Fixture::new();
    fs::create_dir(root.0.join("Notes")).unwrap();
    let file = root.0.join("Notes/brief.md");
    fs::write(&file, "brief").unwrap();
    let outside = other.0.join("reference.md");
    fs::write(&outside, "reference").unwrap();
    symlink(&outside, root.0.join("link.md")).unwrap();
    let workspace = Workspace::open(&root.0).unwrap();
    assert_eq!(
        workspace.selected_file(&file).unwrap().as_deref(),
        Some("Notes/brief.md")
    );
    assert!(workspace.selected_file(&outside).unwrap().is_none());
    assert!(workspace.selected_file(&root.0.join("link.md")).is_err());
}

#[test]
fn explorer_permanent_delete_leaves_no_restore_point_and_does_not_follow_links() {
    let root = Fixture::new();
    let private = Fixture::new();
    let outside = Fixture::new();
    fs::set_permissions(&private.0, fs::Permissions::from_mode(0o700)).unwrap();
    let workspace = Workspace::open(&root.0)
        .unwrap()
        .with_recovery(private.0.clone());
    fs::create_dir(root.0.join("Notes")).unwrap();
    fs::write(root.0.join("Notes/brief.md"), "discard").unwrap();
    let retained = outside.0.join("retained.md");
    fs::write(&retained, "keep").unwrap();
    symlink(&outside.0, root.0.join("Notes/link")).unwrap();

    workspace.delete_entry("Notes/brief.md").unwrap();
    assert!(!root.0.join("Notes/brief.md").exists());
    workspace.delete_entry("Notes").unwrap();
    assert!(!root.0.join("Notes").exists());
    assert!(workspace.list_trash().unwrap().is_empty());
    assert_eq!(fs::read(retained).unwrap(), b"keep");
    assert!(workspace.delete_entry("../outside.md").is_err());
}
