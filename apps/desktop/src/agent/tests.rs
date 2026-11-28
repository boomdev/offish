use super::*;
use crate::workspace::review::Acceptance;
use crate::workspaces::SavedWorkspace;
use std::{
    cell::Cell,
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::{Path, PathBuf},
};

const PASSAGE: &str = "Keep the first release on the current schedule.";
const BRIEF: &str = "\u{feff}---\r\nstatus: draft\r\n---\r\n\r\nCafé 😀\r\nKeep the first release on the current schedule.\r\n";
const REF: &str = "The pilot date moves to 14 December. REF_PRIVATE_CONTENT.";
const CANARY: &str = "token=UNAPPROVED_CANARY";
const PROMPT: &str = "Revise the launch schedule.";

struct Project {
    temp: PathBuf,
    workspace: ActiveWorkspace,
    private: Vec<PathBuf>,
}

impl Project {
    fn new() -> Self {
        let serial = TURN_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temp =
            std::env::temp_dir().join(format!("offish-agent-{}-{serial}", std::process::id()));
        let root = temp.join("pilot");
        let private = root.join("private");
        fs::create_dir_all(root.join("refs")).unwrap();
        fs::create_dir(&private).unwrap();
        fs::create_dir(root.join(".git")).unwrap();
        fs::write(root.join("brief.md"), BRIEF).unwrap();
        fs::write(root.join("refs/assumptions.md"), REF).unwrap();
        fs::write(root.join("refs/large.txt"), "x".repeat(4097)).unwrap();
        fs::write(root.join("refs/blocked.txt"), CANARY).unwrap();
        fs::write(root.join("refs/local.ignore"), CANARY).unwrap();
        fs::write(root.join(".offishignore"), "refs/blocked.txt\n").unwrap();
        fs::write(root.join("refs/.gitignore"), "*.ignore\n").unwrap();
        fs::write(root.join(".git/config"), CANARY).unwrap();
        fs::write(private.join("token.txt"), CANARY).unwrap();
        fs::write(root.join(".env"), CANARY).unwrap();
        fs::write(temp.join("outside.txt"), CANARY).unwrap();
        symlink(temp.join("outside.txt"), root.join("refs/link.txt")).unwrap();
        fs::hard_link(temp.join("outside.txt"), root.join("refs/hardlink.txt")).unwrap();
        let workspace = ActiveWorkspace::open(
            SavedWorkspace {
                id: 1,
                roots: vec![root],
                custom_name: None,
            },
            &temp.join("recovery"),
        );
        Self {
            temp,
            workspace,
            private: vec![private],
        }
    }

    fn root(&self) -> &Path {
        &self.workspace.record.roots[0]
    }

    fn history(&self) -> HistoryStore {
        HistoryStore::new(&self.temp.join("chat"), &self.workspace).unwrap()
    }

    fn request(&self, mentions: &[(&str, bool)]) -> Request {
        let (_, root, relative) = self.workspace.route("root-0/brief.md").unwrap();
        let offset = BRIEF.find(PASSAGE).unwrap();
        let range = SourceMap::new(BRIEF)
            .unwrap()
            .editor_range(offset..offset + PASSAGE.len())
            .unwrap();
        Request {
            provider: Provider::Offline,
            prompt: PROMPT.into(),
            selection: Selection {
                path: "root-0/brief.md".into(),
                base: root.read(relative).unwrap().revision,
                from: range.start,
                to: range.end,
            },
            mentions: mentions
                .iter()
                .map(|(path, directory)| Mention {
                    path: format!("root-0/{path}"),
                    directory: *directory,
                })
                .collect(),
        }
    }

    fn prepare(&self, conversation: &mut Conversation, mentions: &[(&str, bool)]) -> Prepared {
        conversation
            .prepare(&self.workspace, &self.private, self.request(mentions))
            .unwrap()
    }

    fn poll(&self, conversation: &mut Conversation, id: &str) -> Update {
        conversation
            .poll(id, &self.workspace, &self.private, &self.history())
            .unwrap()
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.temp).unwrap();
    }
}

#[test]
fn scoped_disclosure_refuses_canaries_even_as_explicit_mentions() {
    let project = Project::new();
    let mut conversation = Conversation::default();
    for path in [
        "../outside.txt",
        "refs/link.txt",
        "refs/hardlink.txt",
        "refs/blocked.txt",
        "refs/local.ignore",
        ".git/config",
        ".env",
        "private/token.txt",
        "refs/large.txt",
        "brief.md",
    ] {
        let result = conversation.prepare(
            &project.workspace,
            &project.private,
            project.request(&[(path, false)]),
        );
        assert!(result.is_err(), "unexpected disclosure from {path}");
        assert!(!result.err().unwrap().to_string().contains(CANARY));
    }
    let prepared = project.prepare(
        &mut conversation,
        &[("refs", true), ("refs/assumptions.md", false)],
    );
    assert_eq!(prepared.selection.text, PASSAGE);
    assert!(prepared.limited);
    assert_eq!(prepared.files.len(), 1);
    assert_eq!(prepared.files[0].text, REF);
    assert!(!serde_json::to_string(&prepared).unwrap().contains(CANARY));
    assert!(matches!(
        project.poll(&mut conversation, &prepared.id),
        Update::Revoked
    ));
    assert!(project.history().load().unwrap().is_empty());
}

#[test]
fn approved_snapshot_is_immutable_and_stages_an_exact_review_edit() {
    let project = Project::new();
    let mut conversation = Conversation::default();
    let prepared = project.prepare(&mut conversation, &[("refs/assumptions.md", false)]);
    fs::write(project.root().join("refs/assumptions.md"), CANARY).unwrap();
    let history = project.history();
    assert!(
        conversation
            .approve(
                "different id",
                &project.workspace,
                &project.private,
                &history
            )
            .is_err()
    );
    conversation
        .approve(&prepared.id, &project.workspace, &project.private, &history)
        .unwrap();
    let mut answer = String::new();
    for _ in 0..3 {
        match project.poll(&mut conversation, &prepared.id) {
            Update::Delta { text } => answer.push_str(&text),
            _ => panic!("expected an incremental response"),
        }
    }
    let Update::Done {
        message,
        staged: Some(staged),
    } = project.poll(&mut conversation, &prepared.id)
    else {
        panic!("expected a validated proposal");
    };
    assert_eq!(message.answer, answer);
    assert_eq!(
        staged.proposal.hunks[0].edit.replacement,
        "Plan for the pilot on 14 December."
    );
    assert_eq!(
        fs::read_to_string(project.root().join("brief.md")).unwrap(),
        BRIEF
    );
    let (_, root, path) = project.workspace.route(&staged.path).unwrap();
    let result = root
        .accept_proposal(&staged.proposal, &["selection".into()])
        .unwrap();
    let Acceptance::Accepted {
        revision,
        checkpoint_id,
        ..
    } = result
    else {
        panic!("acceptance conflicted")
    };
    assert_eq!(
        fs::read_to_string(project.root().join("brief.md")).unwrap(),
        BRIEF.replace(PASSAGE, "Plan for the pilot on 14 December."),
    );
    root.restore_checkpoint(path, &revision, checkpoint_id)
        .unwrap();
    assert_eq!(
        fs::read_to_string(project.root().join("brief.md")).unwrap(),
        BRIEF
    );

    let stored = fs::read_to_string(project.temp.join("chat/1.json")).unwrap();
    assert!(stored.contains(PROMPT));
    for private in [PASSAGE, REF, CANARY, "assumptions.md"] {
        assert!(!stored.contains(private));
    }
    assert_eq!(
        fs::metadata(project.temp.join("chat/1.json"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
}

struct Instrumented<'a> {
    polled: &'a Cell<usize>,
    malicious: bool,
}

impl Adapter for Instrumented<'_> {
    fn start(&self, context: ProviderContext<'_>) -> Box<dyn Iterator<Item = Event> + Send> {
        assert_eq!(context.prompt, PROMPT);
        assert_eq!(context.selection.text, PASSAGE);
        assert_eq!(context.files.len(), 1);
        assert_eq!(context.files[0].text, REF);
        self.polled.set(self.polled.get() + 1);
        let from = if self.malicious {
            0
        } else {
            context.selection.from
        };
        let events = [
            Event::Text("A first delta.".into()),
            Event::Candidate(SourceEdit {
                from,
                to: context.selection.to,
                expected: context.selection.text.clone(),
                replacement: "Replace this.".into(),
            }),
        ];
        Box::new(events.into_iter())
    }
}

#[test]
fn revocation_and_stale_or_out_of_selection_candidates_produce_no_edits() {
    let project = Project::new();
    let history = project.history();
    let mut conversation = Conversation::default();
    let declines = project.prepare(&mut conversation, &[("refs/assumptions.md", false)]);
    conversation.cancel(&declines.id, &history).unwrap();
    let runs = Cell::new(0);
    let adapter = Instrumented {
        polled: &runs,
        malicious: true,
    };
    assert!(
        conversation
            .approve_with(
                &declines.id,
                &project.workspace,
                &project.private,
                &history,
                &adapter
            )
            .is_err()
    );
    assert_eq!(runs.get(), 0);

    for cancel in [true, false] {
        let prepared = project.prepare(&mut conversation, &[("refs/assumptions.md", false)]);
        conversation
            .approve_with(
                &prepared.id,
                &project.workspace,
                &project.private,
                &history,
                &adapter,
            )
            .unwrap();
        assert!(matches!(
            project.poll(&mut conversation, &prepared.id),
            Update::Delta { .. }
        ));
        if cancel {
            conversation.cancel(&prepared.id, &history).unwrap();
            assert!(matches!(
                project.poll(&mut conversation, &prepared.id),
                Update::Revoked
            ));
            assert!(matches!(
                history.load().unwrap().last().unwrap().outcome,
                Outcome::Cancelled
            ));
        } else {
            assert!(matches!(
                project.poll(&mut conversation, &prepared.id),
                Update::Done { staged: None, .. }
            ));
        }
    }

    let prepared = project.prepare(&mut conversation, &[]);
    conversation
        .approve(&prepared.id, &project.workspace, &project.private, &history)
        .unwrap();
    let external = format!("{BRIEF}\r\nExternal update.\r\n");
    fs::write(project.root().join("brief.md"), &external).unwrap();
    for _ in 0..3 {
        assert!(matches!(
            project.poll(&mut conversation, &prepared.id),
            Update::Delta { .. }
        ));
    }
    assert!(matches!(
        project.poll(&mut conversation, &prepared.id),
        Update::Done { staged: None, .. }
    ));
    assert_eq!(
        fs::read_to_string(project.root().join("brief.md")).unwrap(),
        external
    );
}

#[test]
fn history_caps_turns_and_deletion_cannot_be_undone_by_late_completion() {
    let project = Project::new();
    let history = project.history();
    let mut conversation = Conversation::default();
    for _ in 0..21 {
        let prepared = project.prepare(&mut conversation, &[]);
        conversation
            .approve(&prepared.id, &project.workspace, &project.private, &history)
            .unwrap();
        conversation.revoke();
    }
    let restored = project.history().load().unwrap();
    assert_eq!(restored.len(), 20);
    assert!(
        restored
            .iter()
            .all(|message| matches!(message.outcome, Outcome::Interrupted))
    );
    assert!(matches!(
        project.poll(&mut Conversation::default(), &restored[0].id),
        Update::Revoked
    ));
    assert!(HistoryStore::new(&project.root().join("private/chat"), &project.workspace).is_err());

    let prepared = project.prepare(&mut conversation, &[]);
    conversation
        .approve(&prepared.id, &project.workspace, &project.private, &history)
        .unwrap();
    history.clear().unwrap();
    conversation.cancel(&prepared.id, &history).unwrap();
    assert!(history.load().unwrap().is_empty());
    assert!(!project.temp.join("chat/1.json").exists());
}
