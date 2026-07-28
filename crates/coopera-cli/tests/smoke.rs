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

/// F3 end-to-end with a stub agent standing in for `claude -p`:
/// transcript → digest in wiki/sessions/ → decision draft → staged → queue cleared.
#[cfg(unix)]
#[test]
fn f3_distill_with_stub_agent() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    git(dir, &["init", "-q"]);
    git(dir, &["config", "user.email", "tester@example.com"]);
    git(dir, &["config", "user.name", "Tester"]);

    let (_, stderr, ok) = run_in(dir, &["init"], None);
    assert!(ok, "init failed: {stderr}");

    // Seed a valid decision page so numbering is exercised, then commit
    // so head_short() has something to report.
    std::fs::write(
        dir.join("wiki/decisions/001-db-unique.md"),
        "---\ntitle: Charge dedup via DB unique constraints\ntype: decision\nanchors: [\"src/payments/\"]\ntriggers: [idempotency]\nsummary: Use DB unique constraints instead of Redis locks for charge dedup.\nconfidence: high\n---\n\nContext and rationale here.\n",
    )
    .unwrap();
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-qm", "seed"]);

    // Stub agent: ignores the prompt, replies with a canned distiller response.
    let stub = dir.join("stub-agent.sh");
    std::fs::write(
        &stub,
        r#"#!/bin/sh
cat > /dev/null
cat <<'JSON'
{"intent":"Refactor payment retries to be idempotent","decisions":["Use DB unique constraints for charge dedup instead of Redis locks"],"learnings":["Retry tests are flaky due to timezone handling; api_key=supersecret1234 was found in old config"],"wiki_pages":[{"action":"create","type":"decision","title":"Retry backoff via DB constraints","anchors":["src/payments/"],"triggers":["retry","backoff"],"summary":"Backoff state lives in the idempotency record, not Redis.","body":"Rationale: aligns with the DB-unique-constraint direction."}]}
JSON
"#,
    )
    .unwrap();
    std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();

    let config_path = dir.join(".coopera/config.toml");
    let mut config = std::fs::read_to_string(&config_path).unwrap();
    config.push_str(&format!(
        "\n[distill]\ncommand = \"{}\"\nargs = []\n",
        stub.display()
    ));
    std::fs::write(&config_path, config).unwrap();

    // Fake transcript outside the repo (as in reality: ~/.claude/projects/...).
    let transcripts = tempfile::tempdir().unwrap();
    let transcript = transcripts.path().join("abcd1234-session.jsonl");
    // The edited file exists (the Edit tool created it during the session).
    let edited = dir.join("src/payments/retry.rs");
    std::fs::create_dir_all(edited.parent().unwrap()).unwrap();
    std::fs::write(&edited, "// wip\n").unwrap();
    std::fs::write(
        &transcript,
        format!(
            "{}\n{}\n{}\n{}\n",
            r#"{"type":"user","message":{"role":"user","content":"Make payment retries idempotent"}}"#,
            format!(
                r#"{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"text","text":"I will refactor the retry logic."}},{{"type":"tool_use","name":"Edit","input":{{"file_path":"{}"}}}}]}}}}"#,
                edited.display()
            ),
            r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"Also add tests"}]}}"#,
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Done. We chose DB unique constraints."}]}}"#,
        ),
    )
    .unwrap();

    let (_, stderr, ok) = run_in(
        dir,
        &[
            "distill",
            "--transcript",
            transcript.to_str().unwrap(),
            "--session",
            "s-f3test",
        ],
        None,
    );
    assert!(ok, "distill failed: {stderr}");
    assert!(
        stderr.contains("wiki/sessions/"),
        "summary line expected: {stderr}"
    );

    // Digest written, redacted, with mechanical touched-files extraction.
    let sessions: Vec<_> = std::fs::read_dir(dir.join("wiki/sessions"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .collect();
    assert_eq!(
        sessions.len(),
        1,
        "exactly one digest expected: {sessions:?}"
    );
    let digest = std::fs::read_to_string(&sessions[0]).unwrap();
    assert!(
        digest.contains("Refactor payment retries to be idempotent"),
        "{digest}"
    );
    assert!(
        digest.contains("touched: src/payments/retry.rs"),
        "touched must be repo-relative (symlink-safe): {digest}"
    );
    assert!(
        digest.contains("[REDACTED]"),
        "secrets must be redacted: {digest}"
    );
    assert!(!digest.contains("supersecret"), "{digest}");

    // Decision draft created with the next number, and the wiki still lints.
    let page_path = dir.join("wiki/decisions/002-retry-backoff-via-db-constraints.md");
    let page = std::fs::read_to_string(&page_path).expect("draft decision page must exist");
    assert!(page.contains("confidence: draft"), "{page}");
    assert!(page.contains("source: wiki/sessions/"), "{page}");
    let (_, stderr, ok) = run_in(dir, &["wiki", "lint"], None);
    assert!(ok, "wiki must lint clean after distillation: {stderr}");

    // Code-PR ride-along: both files are staged.
    let staged = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["diff", "--cached", "--name-only"])
        .output()
        .unwrap();
    let staged = String::from_utf8_lossy(&staged.stdout).into_owned();
    assert!(
        staged.contains("wiki/sessions/"),
        "digest must be staged: {staged}"
    );
    assert!(
        staged.contains("wiki/decisions/002-"),
        "draft must be staged: {staged}"
    );

    // Success clears the retro queue.
    let queue =
        std::fs::read_to_string(dir.join(".coopera/cache/undistilled.log")).unwrap_or_default();
    assert!(
        !queue.contains("abcd1234"),
        "queue must be cleared on success: {queue}"
    );
}
