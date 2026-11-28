use super::*;
use crate::markdown::PROFILE_VERSION;
use std::{os::unix::fs::PermissionsExt, process::Command};

const PATH: &str = "brief.md";
const ORIGINAL: &str =
    "\u{feff}---\r\nstatus: draft\r\n---\r\n\r\nCafé 5 seats 😀.\r\n\r\n<!-- retain -->\r\n";
const PARTIAL: &str = "\u{feff}---\r\nstatus: draft\r\n---\r\n\r\nCafé 50 confirmed seats 😀.\r\n\r\n<!-- retain -->\r\n";
const FULL: &str = "\u{feff}---\r\nstatus: draft\r\n---\r\n\r\nCafé 50 confirmed seats 日本語.\r\n\r\n<!-- retain -->\r\n";

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "offish-review-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&path).unwrap();
        fs::create_dir(path.join("workspace")).unwrap();
        fs::write(path.join("workspace").join(PATH), ORIGINAL).unwrap();
        Self(path)
    }

    fn open(&self) -> Workspace {
        Workspace::open(&self.0.join("workspace"))
            .unwrap()
            .with_recovery(self.0.join("private"))
    }

    fn document(&self) -> PathBuf {
        self.0.join("workspace").join(PATH)
    }

    fn crash(&self, operation: &str, at: &str, code: i32) {
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "workspace::review::tests::crash_fixture"])
            .env("OFFISH_REVIEW_FIXTURE", &self.0)
            .env("OFFISH_REVIEW_OPERATION", operation)
            .env("OFFISH_REVIEW_STEP", at)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(code), "{output:?}");
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn hunk(source: &str, id: &str, expected: &str, replacement: &str) -> Hunk {
    let from = source.find(expected).unwrap();
    Hunk {
        id: id.into(),
        edit: SourceEdit {
            from,
            to: from + expected.len(),
            expected: expected.into(),
            replacement: replacement.into(),
        },
    }
}

fn proposal(workspace: &Workspace) -> Proposal {
    Proposal {
        operation_id: "pilot-review".into(),
        path: PATH.into(),
        base_revision: workspace.read(PATH).unwrap().revision,
        profile: PROFILE_VERSION,
        hunks: vec![
            hunk(ORIGINAL, "seats", "5", "50 confirmed"),
            hunk(ORIGINAL, "marker", "😀", "日本語"),
        ],
    }
}

fn commit(
    workspace: &Workspace,
    proposal: &Proposal,
    selected: &str,
) -> (String, u64, Option<Proposal>) {
    match workspace
        .accept_proposal(proposal, &[selected.into()])
        .unwrap()
    {
        Acceptance::Accepted {
            revision,
            checkpoint_id,
            remaining,
        } => (revision, checkpoint_id, remaining),
        Acceptance::Conflict { .. } => panic!("unexpected acceptance conflict"),
    }
}

#[test]
fn pilot_proposal_stages_two_exact_hunks_and_returns_restored_text() {
    let fixture = Fixture::new();
    let original = format!(
        "\u{feff}---\r\nstatus: draft\r\n---\r\n\r\nCafé 😀\r\n{}\r\n\r\n{}\r\n",
        PILOT_HUNKS[0].1, PILOT_HUNKS[1].1,
    );
    fs::write(fixture.document(), &original).unwrap();
    let workspace = fixture.open();
    let base = workspace.read(PATH).unwrap().revision;
    let proposal = workspace.pilot_proposal(PATH, &base).unwrap();
    assert_eq!(proposal.base_revision, base);
    assert_eq!(
        proposal
            .hunks
            .iter()
            .map(|hunk| hunk.id.as_str())
            .collect::<Vec<_>>(),
        ["schedule", "budget"]
    );
    assert_eq!(fs::read_to_string(fixture.document()).unwrap(), original);
    assert!(
        workspace
            .review_history(PATH)
            .unwrap()
            .checkpoints
            .is_empty()
    );
    let (revision, id, remaining) = commit(&workspace, &proposal, "schedule");
    let remaining = remaining.unwrap();
    let partial = fs::read_to_string(fixture.document()).unwrap();
    let edit = &remaining.hunks[0].edit;
    assert_eq!(&partial[edit.from..edit.to], "* Confirm the budget.");
    assert!(workspace.pilot_proposal(PATH, &base).is_err());
    let (restored, text) = workspace
        .restore_checkpoint_with_text(PATH, &revision, id)
        .unwrap();
    assert!(matches!(restored, Save::Saved { .. }));
    assert_eq!(text.as_deref(), Some(original.as_str()));
    assert_eq!(fs::read_to_string(fixture.document()).unwrap(), original);

    fs::write(
        fixture.document(),
        original + "\r\n* Confirm the budget.\r\n",
    )
    .unwrap();
    let duplicate_revision = workspace.read(PATH).unwrap().revision;
    assert!(workspace.pilot_proposal(PATH, &duplicate_revision).is_err());
}

#[test]
fn partial_acceptance_rebases_unicode_spans_and_restores_exact_source() {
    let fixture = Fixture::new();
    fs::set_permissions(fixture.document(), fs::Permissions::from_mode(0o640)).unwrap();
    let workspace = fixture.open();
    let original = proposal(&workspace);
    let (revision, first, remainder) = commit(&workspace, &original, "seats");
    assert_eq!(fs::read_to_string(fixture.document()).unwrap(), PARTIAL);
    let remainder = remainder.unwrap();
    assert_eq!(remainder.operation_id, original.operation_id);
    assert_eq!(remainder.base_revision, revision);
    assert_eq!(remainder.hunks[0].id, "marker");
    assert_eq!(remainder.hunks[0].edit.from, PARTIAL.find("😀").unwrap());
    assert!(matches!(
        workspace
            .accept_proposal(&original, &["marker".into()])
            .unwrap(),
        Acceptance::Conflict { .. }
    ));

    let reopened = fixture.open();
    let (revision, second, remainder) = commit(&reopened, &remainder, "marker");
    assert!(remainder.is_none());
    assert!(second > first);
    assert_eq!(fs::read_to_string(fixture.document()).unwrap(), FULL);
    assert_eq!(reopened.review_history(PATH).unwrap().checkpoints.len(), 2);
    assert!(matches!(
        reopened.restore_checkpoint(PATH, &revision, first).unwrap(),
        Save::Saved { .. }
    ));
    assert_eq!(fs::read(fixture.document()).unwrap(), ORIGINAL.as_bytes());
    assert_eq!(
        fixture.document().metadata().unwrap().permissions().mode() & 0o777,
        0o640
    );
    assert!(
        reopened
            .review_history(PATH)
            .unwrap()
            .checkpoints
            .is_empty()
    );
}

#[test]
fn malformed_proposals_and_external_changes_preserve_disk() {
    let fixture = Fixture::new();
    let workspace = fixture.open();
    let mut reviewed = proposal(&workspace);
    for invalid in [
        vec![],
        vec!["missing".into()],
        vec!["seats".into(), "seats".into()],
    ] {
        assert!(workspace.accept_proposal(&reviewed, &invalid).is_err());
    }
    reviewed.path = "../outside.md".into();
    assert!(
        workspace
            .accept_proposal(&reviewed, &["seats".into()])
            .is_err()
    );

    let mut invalid = proposal(&workspace);
    invalid.hunks[1].edit.from += 1;
    assert!(
        workspace
            .accept_proposal(&invalid, &["seats".into()])
            .is_err()
    );
    assert_eq!(fs::read_to_string(fixture.document()).unwrap(), ORIGINAL);
    assert!(
        workspace
            .review_history(PATH)
            .unwrap()
            .checkpoints
            .is_empty()
    );

    let (revision, first, _) = commit(&workspace, &proposal(&workspace), "seats");
    fs::write(fixture.document(), "External replacement\n").unwrap();
    assert!(matches!(
        workspace
            .restore_checkpoint(PATH, &revision, first)
            .unwrap(),
        Save::Conflict { .. }
    ));
    let current = workspace.read(PATH).unwrap().revision;
    assert!(workspace.restore_checkpoint(PATH, &current, first).is_err());
    fs::write(fixture.document(), PARTIAL).unwrap();
    assert!(!workspace.review_history(PATH).unwrap().checkpoints[0].restorable);
}

#[test]
fn keeps_latest_twenty_and_deletion_preserves_checkpoint_continuity() {
    let fixture = Fixture::new();
    let workspace = fixture.open();
    let mut checkpoints = Vec::new();
    for _ in 0..22 {
        let disk = workspace.read(PATH).unwrap();
        let source = disk.text.unwrap();
        let edit = SourceEdit {
            from: source.len(),
            to: source.len(),
            expected: String::new(),
            replacement: "More\r\n".into(),
        };
        let proposal = Proposal {
            operation_id: format!("append-{}", checkpoints.len()),
            path: PATH.into(),
            base_revision: disk.revision,
            profile: PROFILE_VERSION,
            hunks: vec![Hunk {
                id: "append".into(),
                edit,
            }],
        };
        checkpoints.push(commit(&workspace, &proposal, "append").1);
    }
    let reopened = fixture.open();
    let ids = |history: ReviewHistory| {
        history
            .checkpoints
            .into_iter()
            .map(|checkpoint| checkpoint.id)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        ids(reopened.review_history(PATH).unwrap()),
        checkpoints[2..]
    );
    reopened.delete_checkpoint(PATH, checkpoints[3]).unwrap();
    let current = reopened.read(PATH).unwrap().revision;
    reopened
        .restore_checkpoint(PATH, &current, checkpoints[4])
        .unwrap();
    assert_eq!(
        fs::read_to_string(fixture.document()).unwrap(),
        format!("{ORIGINAL}{}", "More\r\n".repeat(4))
    );
    assert_eq!(
        ids(reopened.review_history(PATH).unwrap()),
        checkpoints[2..3]
    );
}

#[test]
fn crash_fixture() {
    let Some(root) = std::env::var_os("OFFISH_REVIEW_FIXTURE") else {
        return;
    };
    let workspace = Workspace::open(&Path::new(&root).join("workspace"))
        .unwrap()
        .with_recovery(Path::new(&root).join("private"));
    let at = std::env::var("OFFISH_REVIEW_STEP").unwrap();
    let hook = |step| match (at.as_str(), step) {
        ("before", SaveStep::JournalSynced) => std::process::exit(73),
        ("after", SaveStep::Replaced) => std::process::exit(75),
        _ => Ok(()),
    };
    match std::env::var("OFFISH_REVIEW_OPERATION").unwrap().as_str() {
        "accept" => {
            workspace
                .accept_with_hook(&proposal(&workspace), &["seats".into()], hook)
                .unwrap();
        }
        "restore" => {
            let revision = workspace.read(PATH).unwrap().revision;
            let id = workspace.review_history(PATH).unwrap().checkpoints[0].id;
            workspace
                .restore_with_hook(PATH, &revision, id, hook)
                .unwrap();
        }
        "move" => {
            let (source, name) = workspace.parent(PATH).unwrap();
            let destination = workspace.directory(Path::new("moved")).unwrap();
            workspace
                .relocate(source, &name, destination, &name, |step| {
                    match (at.as_str(), step) {
                        ("before", MoveStep::Prepared) => std::process::exit(73),
                        ("after", MoveStep::Renamed) => std::process::exit(75),
                        _ => Ok(()),
                    }
                })
                .unwrap();
        }
        _ => panic!("invalid fixture operation"),
    }
    panic!("expected interrupted operation");
}

#[test]
fn process_exit_recovers_both_sides_of_acceptance_and_restore() {
    let fixture = Fixture::new();
    fixture.crash("accept", "before", 73);
    let reopened = fixture.open();
    let history = reopened.review_history(PATH).unwrap();
    assert_eq!(history.recovery, ReviewRecovery::OriginalOnDisk);
    assert!(history.checkpoints.is_empty());
    assert_eq!(fs::read_to_string(fixture.document()).unwrap(), ORIGINAL);
    assert_eq!(
        fs::read_dir(fixture.0.join("workspace")).unwrap().count(),
        1
    );

    fixture.crash("accept", "after", 75);
    let history = reopened.review_history(PATH).unwrap();
    assert_eq!(history.recovery, ReviewRecovery::Committed);
    assert_eq!(history.checkpoints[0].hunk_ids, ["seats"]);
    assert!(history.checkpoints[0].restorable);
    assert_eq!(fs::read_to_string(fixture.document()).unwrap(), PARTIAL);
    assert_eq!(
        fs::read_dir(fixture.0.join("workspace")).unwrap().count(),
        1
    );

    fixture.crash("restore", "before", 73);
    let history = reopened.review_history(PATH).unwrap();
    assert_eq!(history.recovery, ReviewRecovery::OriginalOnDisk);
    assert_eq!(history.checkpoints.len(), 1);
    assert_eq!(fs::read_to_string(fixture.document()).unwrap(), PARTIAL);

    fixture.crash("restore", "after", 75);
    let history = reopened.review_history(PATH).unwrap();
    assert_eq!(history.recovery, ReviewRecovery::Committed);
    assert!(history.checkpoints.is_empty());
    assert_eq!(fs::read_to_string(fixture.document()).unwrap(), ORIGINAL);
}

#[test]
fn interrupted_review_conflict_preserves_disk_and_private_preimage() {
    let fixture = Fixture::new();
    fixture.crash("accept", "after", 75);
    fs::write(fixture.document(), "Externally edited\n").unwrap();
    let workspace = fixture.open();
    let history = workspace.review_history(PATH).unwrap();
    assert_eq!(history.recovery, ReviewRecovery::Conflict);
    let id = history.interrupted_id.unwrap();
    let private = fixture.0.join("private");
    let store = fs::read_dir(&private)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert_eq!(
        private.metadata().unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(fs::read(store.join("1.md")).unwrap(), ORIGINAL.as_bytes());
    let current = workspace.read(PATH).unwrap().revision;
    assert!(workspace.save(PATH, &current, "Another write").is_err());
    assert!(
        workspace
            .keep_disk_after_review_conflict(PATH, id, Some("stale"))
            .is_err()
    );
    assert_eq!(workspace.review_preimage(PATH, id).unwrap(), ORIGINAL);
    workspace
        .keep_disk_after_review_conflict(PATH, id, Some(&current))
        .unwrap();
    assert_eq!(
        fs::read_to_string(fixture.document()).unwrap(),
        "Externally edited\n"
    );
    assert_eq!(workspace.review_preimage(PATH, id).unwrap(), ORIGINAL);
    let resolved = workspace.review_history(PATH).unwrap();
    assert_eq!(resolved.recovery, ReviewRecovery::Clean);
    assert!(!resolved.checkpoints[0].restorable);
    assert!(matches!(
        workspace.save(PATH, &current, "Another write").unwrap(),
        Save::Saved { .. }
    ));
}

#[test]
fn independent_handles_cannot_accept_twice_against_one_revision() {
    let fixture = Fixture::new();
    let first = fixture.open();
    let second = fixture.open();
    let first_proposal = proposal(&first);
    let second_proposal = proposal(&second);
    let barrier = std::sync::Barrier::new(2);
    let accepted = std::thread::scope(|scope| {
        let worker = scope.spawn(|| {
            barrier.wait();
            first
                .accept_proposal(&first_proposal, &["seats".into()])
                .unwrap()
        });
        barrier.wait();
        let second_result = second
            .accept_proposal(&second_proposal, &["marker".into()])
            .unwrap();
        [worker.join().unwrap(), second_result]
            .into_iter()
            .filter(|result| matches!(result, Acceptance::Accepted { .. }))
            .count()
    });
    assert_eq!(accepted, 1);
    assert_eq!(first.review_history(PATH).unwrap().checkpoints.len(), 1);
}

#[test]
fn checkpoints_follow_rename_and_move_with_an_independent_history_at_the_old_name() {
    let fixture = Fixture::new();
    let workspace = fixture.open();
    let (revision, checkpoint, _) = commit(&workspace, &proposal(&workspace), "seats");

    fs::create_dir(fixture.0.join("workspace/moved")).unwrap();
    assert!(
        workspace
            .rename_entry(PATH, "renamed.md")
            .unwrap()
            .is_none()
    );
    assert!(
        workspace
            .move_entry("renamed.md", "moved")
            .unwrap()
            .is_none()
    );
    workspace.rename_entry("moved", "renamed-folder").unwrap();
    fs::write(fixture.document(), ORIGINAL).unwrap();

    let reopened = fixture.open();
    assert!(
        reopened
            .review_history(PATH)
            .unwrap()
            .checkpoints
            .is_empty()
    );
    assert!(reopened.review_preimage(PATH, checkpoint).is_err());
    let (new_revision, new_checkpoint, _) = commit(&reopened, &proposal(&reopened), "marker");
    let moved = "renamed-folder/renamed.md";
    assert_eq!(reopened.read(moved).unwrap().revision, revision);
    assert_eq!(
        reopened.review_history(moved).unwrap().checkpoints[0].id,
        checkpoint
    );
    assert_eq!(
        reopened.review_preimage(moved, checkpoint).unwrap(),
        ORIGINAL
    );
    assert!(matches!(
        reopened
            .restore_checkpoint(moved, &revision, checkpoint)
            .unwrap(),
        Save::Saved { .. }
    ));
    assert_eq!(
        reopened.read(moved).unwrap().text.as_deref(),
        Some(ORIGINAL)
    );
    assert_eq!(reopened.read(PATH).unwrap().revision, new_revision);
    assert_eq!(
        reopened.review_history(PATH).unwrap().checkpoints[0].id,
        new_checkpoint
    );
}

#[test]
fn process_exit_before_or_after_move_preserves_checkpoint_lookup() {
    let fixture = Fixture::new();
    let workspace = fixture.open();
    fs::create_dir(fixture.0.join("workspace/moved")).unwrap();
    let moved = "moved/brief.md";
    let (revision, checkpoint, _) = commit(&workspace, &proposal(&workspace), "seats");

    fixture.crash("move", "before", 73);
    let reopened = fixture.open();
    assert!(
        reopened
            .review_history(moved)
            .unwrap()
            .checkpoints
            .is_empty()
    );
    assert_eq!(
        reopened.review_history(PATH).unwrap().checkpoints[0].id,
        checkpoint
    );

    fixture.crash("move", "after", 75);
    let restarted = fixture.open();
    fs::write(fixture.document(), "Replacement file\n").unwrap();
    assert!(
        restarted
            .review_history(PATH)
            .unwrap()
            .checkpoints
            .is_empty()
    );
    let current = restarted.review_history(moved).unwrap();
    assert_eq!(current.checkpoints[0].id, checkpoint);
    assert!(current.checkpoints[0].restorable);
    assert!(matches!(
        restarted
            .restore_checkpoint(moved, &revision, checkpoint)
            .unwrap(),
        Save::Saved { .. }
    ));
    assert_eq!(
        restarted.read(moved).unwrap().text.as_deref(),
        Some(ORIGINAL)
    );
    assert_eq!(
        restarted.read(PATH).unwrap().text.as_deref(),
        Some("Replacement file\n")
    );
}

#[test]
fn destination_collision_does_not_change_either_history() {
    let fixture = Fixture::new();
    let workspace = fixture.open();
    commit(&workspace, &proposal(&workspace), "seats");
    let mut other = proposal(&workspace);
    other.path = "other.md".into();
    fs::write(fixture.0.join("workspace/other.md"), ORIGINAL).unwrap();
    other.base_revision = workspace.read("other.md").unwrap().revision;
    commit(&workspace, &other, "marker");

    assert_eq!(
        workspace.rename_entry(PATH, "other.md").unwrap_err().kind(),
        io::ErrorKind::AlreadyExists
    );
    assert_eq!(
        workspace.review_history(PATH).unwrap().checkpoints[0].hunk_ids,
        ["seats"]
    );
    assert_eq!(
        workspace.review_history("other.md").unwrap().checkpoints[0].hunk_ids,
        ["marker"]
    );
}

#[test]
fn a_destination_created_after_preparation_cancels_only_the_move() {
    let fixture = Fixture::new();
    let workspace = fixture.open();
    let (_, checkpoint, _) = commit(&workspace, &proposal(&workspace), "seats");
    let (source, name) = workspace.parent(PATH).unwrap();
    let destination = workspace.directory(Path::new("")).unwrap();
    let occupied = fixture.0.join("workspace/late.md");
    let result = workspace.relocate(source, &name, destination, Path::new("late.md"), |step| {
        if step == MoveStep::Prepared {
            fs::write(&occupied, "External file\n")?;
        }
        Ok(())
    });
    assert_eq!(result.unwrap_err().kind(), io::ErrorKind::AlreadyExists);
    assert_eq!(
        workspace.review_history(PATH).unwrap().checkpoints[0].id,
        checkpoint
    );
    assert!(
        workspace
            .review_history("late.md")
            .unwrap()
            .checkpoints
            .is_empty()
    );
    assert_eq!(fs::read_to_string(occupied).unwrap(), "External file\n");
    assert_eq!(workspace.read(PATH).unwrap().text.as_deref(), Some(PARTIAL));
}
