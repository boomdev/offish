use super::*;

fn fixture(frames: &[Value]) -> Command {
    let mut command = Command::new("/bin/sh");
    command
        .arg("-c")
        .arg("printf '%s\\n' \"$@\"")
        .arg("codex-fixture");
    command.args(frames.iter().map(Value::to_string));
    command
}

#[test]
fn only_completed_matching_turn_can_return_a_replacement() {
    let reply =
        json!({"summary":"Schedule revised.", "replacement":"Plan for December."}).to_string();
    let mut command = fixture(&[
        json!({"id":3,"result":{"turn":{"id":"turn"}}}),
        json!({"method":"item/agentMessage/delta","params":{"threadId":"other","turnId":"turn","delta":"ignored"}}),
        json!({"method":"item/agentMessage/delta","params":{"threadId":"thread","turnId":"turn","delta":"approved progress"}}),
        json!({"method":"item/completed","params":{"threadId":"thread","turnId":"turn",
            "item":{"type":"agentMessage","phase":"finalAnswer","text":reply}}}),
        json!({"method":"turn/completed","params":{"threadId":"other","turn":{"id":"turn","status":"failed"}}}),
        json!({"method":"turn/completed","params":{"threadId":"thread","turn":{"id":"turn","status":"completed"}}}),
    ]);
    let stop = AtomicBool::new(false);
    let mut server = Transport::spawn(&mut command, &stop, Duration::from_secs(2)).unwrap();
    let mut received = false;
    let output = finish_turn(&mut server, "thread", || received = true).unwrap();
    assert!(received);
    assert_eq!(output.replacement, "Plan for December.");
}

#[test]
fn unsolicited_server_requests_and_late_frames_are_refused() {
    let mut command =
        fixture(&[json!({"id":7,"method":"item/tool/call","params":{"message":"SECRET_CANARY"}})]);
    let stop = AtomicBool::new(false);
    let mut server = Transport::spawn(&mut command, &stop, Duration::from_secs(2)).unwrap();
    let error = server.frame().unwrap_err().to_string();
    assert!(error.contains("outside this conversation"));
    assert!(!error.contains("SECRET_CANARY"));
    stop.store(true, Ordering::Release);
    assert_eq!(
        server.frame().unwrap_err().to_string(),
        "Codex turn cancelled."
    );
}

#[test]
#[ignore = "requires the pinned static Codex binary and a Linux bubblewrap installation"]
fn pinned_binary_initializes_and_starts_an_ephemeral_thread_offline() {
    let binary = binary().unwrap();
    let stop = AtomicBool::new(false);
    verify_version(&binary, &stop).unwrap();
    let mut command = sandbox(&binary, false).unwrap();
    let mut server =
        Transport::spawn(command.args(CONFIG), &stop, Duration::from_secs(15)).unwrap();
    let result = open_thread(&mut server).unwrap();
    assert!(
        result
            .pointer("/thread/id")
            .and_then(Value::as_str)
            .is_some()
    );
}

#[test]
#[ignore = "requires a static C compiler and a Linux bubblewrap installation"]
fn sandbox_hides_the_workspace_and_refuses_direct_writes() {
    let scratch = env::temp_dir().join(format!("offish-sandbox-check-{}", std::process::id()));
    std::fs::create_dir(&scratch).unwrap();
    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(scratch.clone());

    let canary = scratch.join("private.md");
    std::fs::write(&canary, "host-private fixture").unwrap();
    let source = scratch.join("probe.c");
    let executable = scratch.join("probe");
    std::fs::write(
        &source,
        r#"
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

int main(int argc, char **argv) {
    if (argc != 2 || getenv("OFFISH_TEST_PRIVATE") || getenv("DBUS_SESSION_BUS_ADDRESS")) return 1;
    if (open(argv[1], O_RDONLY) != -1) return 2;
    if (open(argv[1], O_WRONLY) != -1) return 3;
    if (open("/app/codex", O_WRONLY) != -1) return 4;
    if (open("/app/new", O_WRONLY | O_CREAT, 0600) != -1) return 5;
    int scratch = open("/tmp/codex/check", O_WRONLY | O_CREAT, 0600);
    if (scratch == -1) return 6;
    close(scratch);
    puts("isolated");
    return 0;
}
"#,
    )
    .unwrap();
    assert!(
        Command::new("cc")
            .arg("-static")
            .arg(&source)
            .arg("-o")
            .arg(&executable)
            .status()
            .unwrap()
            .success()
    );
    let mut command = sandbox(&executable, true).unwrap();
    let stop = AtomicBool::new(false);
    let mut server = Transport::spawn(command.arg(&canary), &stop, Duration::from_secs(3)).unwrap();
    assert_eq!(server.line().unwrap(), b"isolated\n");
    assert_eq!(
        std::fs::read_to_string(canary).unwrap(),
        "host-private fixture"
    );
}
