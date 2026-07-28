//! Smoke test — proves the M1 vertical slice end-to-end:
//! init installs the harness, a wiki page + changed file produce a
//! budget-respecting SessionStart injection, and lint gates the schema.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_coopera")
}

fn git(dir: &Path, args: &[&str]) {
    let ok = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .unwrap()
        .status
        .success();
    assert!(ok, "git {args:?} failed");
}

fn run_in(dir: &Path, args: &[&str], stdin: Option<&str>) -> (String, String, bool) {
    let mut cmd = Command::new(bin());
    cmd.args(args)
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().unwrap();
    if let Some(input) = stdin {
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
    }
    drop(child.stdin.take());
    let out = child.wait_with_output().unwrap();
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        out.status.success(),
    )
}

#[test]
fn m1_vertical_slice() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    git(dir, &["init", "-q"]);

    // F1: init installs the harness
    let (stdout, stderr, ok) = run_in(dir, &["init"], None);
    assert!(ok, "init failed: {stderr}");
    assert!(stdout.contains("coopera: installed"), "{stdout}");
    assert!(dir.join("wiki/INDEX.md").exists());
    assert!(dir.join(".coopera/config.toml").exists());
    let settings = std::fs::read_to_string(dir.join(".claude/settings.json")).unwrap();
    assert!(settings.contains("hook session-start"), "{settings}");
    assert!(std::fs::read_to_string(dir.join("AGENTS.md"))
        .unwrap()
        .contains("coopera"));

    // F1: idempotent re-run
    let (stdout, _, ok) = run_in(dir, &["init"], None);
    assert!(ok);
    assert!(
        stdout.contains("nothing to do"),
        "re-run must be a no-op: {stdout}"
    );

    // Add a decision page + a matching changed file
    std::fs::create_dir_all(dir.join("src/payments")).unwrap();
    std::fs::write(dir.join("src/payments/retry.rs"), "// wip").unwrap();
    std::fs::write(
        dir.join("wiki/decisions/001-db-unique.md"),
        "---\ntitle: Charge dedup via DB unique constraints\ntype: decision\nanchors: [\"src/payments/\"]\ntriggers: [idempotency]\nsummary: Use DB unique constraints instead of Redis locks for charge dedup.\nconfidence: high\n---\n\nContext and rationale here.\n",
    )
    .unwrap();

    // F2: SessionStart injection includes the decision, respects protocol
    let (stdout, stderr, ok) = run_in(dir, &["hook", "session-start"], Some("{}"));
    assert!(ok, "hook failed: {stderr}");
    let json: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("hook output must be JSON");
    let ctx = json["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert!(
        ctx.contains("DB unique constraints"),
        "decision summary must be injected: {ctx}"
    );
    assert!(ctx.contains("Guidance"), "guidance line must be present");
    let msg = json["systemMessage"].as_str().unwrap();
    assert!(
        msg.contains("coopera: injected"),
        "visibility line required: {msg}"
    );

    // F2: fail-open outside a repo
    let outside = tempfile::tempdir().unwrap();
    let (stdout, _, ok) = run_in(outside.path(), &["hook", "session-start"], Some("{}"));
    assert!(ok, "hook must exit 0 outside a repo (fail-open)");
    let json: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert!(json["systemMessage"].as_str().unwrap().contains("inactive"));

    // F4: lint passes on the valid wiki, fails on a broken page
    let (_, _, ok) = run_in(dir, &["wiki", "lint"], None);
    assert!(ok, "lint should pass on valid pages");
    std::fs::write(
        dir.join("wiki/concepts/bad.md"),
        "---\ntitle: Bad\ntype: concept\nsummary: \"\"\n---\nbody\n",
    )
    .unwrap();
    let (_, stderr, ok) = run_in(dir, &["wiki", "lint"], None);
    assert!(!ok, "lint must fail on schema violations");
    assert!(stderr.contains("anchors"), "{stderr}");
}
