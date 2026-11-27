// Integration check through the installed command's entry point.
#[test]
fn demo_keeps_the_callers_store_and_settings_isolated() {
    let dir = std::env::temp_dir().join(format!("instinct-demo-test-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let store = dir.join("existing.jsonl");
    let sentinel = b"original bytes\n";
    std::fs::write(&store, sentinel).unwrap();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_instinct"))
        .args(["demo"])
        .env("INSTINCT_STORE", &store)
        .env("INSTINCT_PROJECT", "unrelated")
        .output()
        .unwrap();
    assert_eq!(std::fs::read(&store).unwrap(), sentinel);
    std::fs::remove_dir_all(dir).unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("You: [task] / Me: [task]"));
    assert!(text.contains("Tess's usual client updates"));
    assert!(text.contains("No applicable experience"));
}

#[test]
fn imported_bank_preserves_rows_and_feedback_retries_count_once() {
    use std::{
        fs,
        process::{Command, Stdio},
    };

    let dir = std::env::temp_dir().join(format!("instinct-journal-test-{}", std::process::id()));
    fs::create_dir(&dir).unwrap();
    let store = dir.join("bank.jsonl");
    let first = r#"{"kind":"discovery","id":"legacy","title":"Desk","when":"pickup sleeve","found":"accepted","how":"look up seat","evidence":"desk sheet"}"#;
    fs::write(&store, first).unwrap();
    let run = |args: &[&str]| {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_instinct"));
        cmd.args(args)
            .env("INSTINCT_STORE", &store)
            .env_remove("INSTINCT_OLLAMA")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        cmd
    };
    let writers: Vec<_> = (0..12)
        .map(|_| {
            run(&["hit", "legacy", "--outcome-id", "event-1"])
                .spawn()
                .unwrap()
        })
        .collect();
    for writer in writers {
        assert!(writer.wait_with_output().unwrap().status.success());
    }
    let saved = fs::read_to_string(&store).unwrap();
    assert_eq!(saved.lines().count(), 2);
    assert_eq!(saved.lines().next(), Some(first));
    assert!(!run(&["miss", "legacy", "--outcome-id", "event-1"])
        .output()
        .unwrap()
        .status
        .success());
    assert_eq!(fs::read_to_string(&store).unwrap(), saved);
    let stats = run(&["stats"]).output().unwrap();
    assert!(String::from_utf8_lossy(&stats.stdout).contains("1 hits / 0 misses"));
    assert!(run(&["retract", "hit", "legacy"])
        .output()
        .unwrap()
        .status
        .success());
    assert!(run(&["hit", "legacy", "--outcome-id", "event-1"])
        .output()
        .unwrap()
        .status
        .success());
    let stats = run(&["stats"]).output().unwrap();
    assert!(String::from_utf8_lossy(&stats.stdout).contains("0 hits / 0 misses"));
    let torn = format!("{saved}{{\"kind\":");
    fs::write(&store, &torn).unwrap();
    let attempt = run(&["retract", "hit", "legacy"]).output().unwrap();
    assert!(!attempt.status.success());
    assert_eq!(fs::read_to_string(&store).unwrap(), torn);
    fs::remove_dir_all(dir).unwrap();
}
