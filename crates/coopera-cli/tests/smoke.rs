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
    run_in_env(dir, args, stdin, &[])
}

fn run_in_env(
    dir: &Path,
    args: &[&str],
    stdin: Option<&str>,
    envs: &[(&str, &str)],
) -> (String, String, bool) {
    let mut cmd = Command::new(bin());
    cmd.args(args)
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in envs {
        cmd.env(k, v);
    }
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
    assert!(dir.join("coopera/INDEX.md").exists());
    assert!(dir.join(".coopera/config.toml").exists());
    let settings = std::fs::read_to_string(dir.join(".claude/settings.json")).unwrap();
    assert!(settings.contains("hook session-start"), "{settings}");
    assert!(settings.contains("hook user-prompt-submit"), "{settings}");
    let codex = std::fs::read_to_string(dir.join(".codex/config.toml")).unwrap();
    assert!(codex.contains("[[hooks.SessionStart]]"), "{codex}");
    assert!(codex.contains("COOPERA_TOOL=codex"), "{codex}");
    assert!(
        codex.contains("COOPERA_SESSION_FALLBACK=\"ppid-$PPID\""),
        "codex hooks need a stable session key (no session id in payloads): {codex}"
    );

    // Committed hook config must stay machine-independent: no absolute paths
    // (the installer's home dir would be wrong on every teammate's machine).
    let installer = bin();
    for (name, content) in [("settings.json", &settings), ("config.toml", &codex)] {
        assert!(
            !content.contains(installer),
            "{name} must not bake in the installer path: {content}"
        );
        assert!(
            !content.contains("/Users/") && !content.contains("/home/"),
            "{name} must not contain absolute home paths: {content}"
        );
        assert!(
            content.contains("${COOPERA_BIN:-coopera}"),
            "{name} must resolve the binary from PATH/COOPERA_BIN: {content}"
        );
    }
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
        dir.join("coopera/decisions/001-db-unique.md"),
        "---\ntitle: Charge dedup via DB unique constraints\ntype: decision\nanchors: [\"src/payments/\"]\ntriggers: [idempotency]\nsummary: Use DB unique constraints instead of Redis locks for charge dedup.\nconfidence: high\n---\n\nContext and rationale here.\n",
    )
    .unwrap();

    // F2: SessionStart injection includes the decision, respects protocol.
    // (COOPERA_RETRO_SYNC keeps the auto-retro sweep synchronous so no
    // background process races the test's later queue-file writes.)
    let (stdout, stderr, ok) = run_in_env(
        dir,
        &["hook", "session-start"],
        Some("{}"),
        &[("COOPERA_RETRO_SYNC", "1")],
    );
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

    // P2: injection is instrumented locally.
    let metrics = std::fs::read_to_string(dir.join(".coopera/cache/metrics.jsonl")).unwrap();
    assert!(
        metrics.lines().any(|l| l.contains("\"event\":\"inject\"")),
        "inject event must be recorded: {metrics}"
    );

    // P0: an undistilled backlog is surfaced in the visibility line (the
    // count is read before the auto-retro sweep clears dead entries).
    std::fs::write(
        dir.join(".coopera/cache/undistilled.log"),
        "/nonexistent/transcript.jsonl\n",
    )
    .unwrap();
    let (stdout, _, ok) = run_in_env(
        dir,
        &["hook", "session-start"],
        Some("{}"),
        &[("COOPERA_RETRO_SYNC", "1")],
    );
    assert!(ok);
    let json: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert!(
        json["systemMessage"]
            .as_str()
            .unwrap()
            .contains("1 undistilled session(s) pending"),
        "backlog must be visible: {json}"
    );
    std::fs::write(dir.join(".coopera/cache/undistilled.log"), "").unwrap();

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
        dir.join("coopera/concepts/bad.md"),
        "---\ntitle: Bad\ntype: concept\nsummary: \"\"\n---\nbody\n",
    )
    .unwrap();
    let (_, stderr, ok) = run_in(dir, &["wiki", "lint"], None);
    assert!(!ok, "lint must fail on schema violations");
    assert!(stderr.contains("anchors"), "{stderr}");
}

/// F3 end-to-end with a stub agent standing in for `claude -p`:
/// transcript → digest in coopera/sessions/ → decision draft → staged → queue cleared.
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
        dir.join("coopera/decisions/001-db-unique.md"),
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
        stderr.contains("coopera/sessions/"),
        "summary line expected: {stderr}"
    );

    // Digest written, redacted, with mechanical touched-files extraction.
    let sessions: Vec<_> = std::fs::read_dir(dir.join("coopera/sessions"))
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
    let page_path = dir.join("coopera/decisions/002-retry-backoff-via-db-constraints.md");
    let page = std::fs::read_to_string(&page_path).expect("draft decision page must exist");
    assert!(page.contains("confidence: draft"), "{page}");
    assert!(page.contains("source: coopera/sessions/"), "{page}");
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
        staged.contains("coopera/sessions/"),
        "digest must be staged: {staged}"
    );
    assert!(
        staged.contains("coopera/decisions/002-"),
        "draft must be staged: {staged}"
    );

    // Success clears the retro queue.
    let queue =
        std::fs::read_to_string(dir.join(".coopera/cache/undistilled.log")).unwrap_or_default();
    assert!(
        !queue.contains("abcd1234"),
        "queue must be cleared on success: {queue}"
    );

    // P2: distillation is instrumented locally.
    let metrics = std::fs::read_to_string(dir.join(".coopera/cache/metrics.jsonl")).unwrap();
    assert!(
        metrics.lines().any(|l| l.contains("\"event\":\"distill\"")),
        "distill event must be recorded: {metrics}"
    );
}

fn pad_line() -> String {
    format!(r#"{{"type":"pad","x":"{}"}}"#, "x".repeat(20_000))
}

/// A plausible finished-session transcript: 4 substantive messages plus a
/// pad line (skipped by the extractor) that pushes it over the 16KB scan
/// threshold.
fn backlog_transcript() -> String {
    [
        r#"{"type":"user","message":{"role":"user","content":"Make payment retries idempotent"}}"#
            .to_string(),
        r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"I will refactor the retry logic."}]}}"#.to_string(),
        r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"Also add tests"}]}}"#.to_string(),
        r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Done. We chose DB unique constraints."}]}}"#.to_string(),
        pad_line(),
    ]
    .join("\n")
}

/// P0 — retro scan: drains the undigested backlog while skipping live
/// sessions (fresh mtime), the distiller's own transcripts (prompt marker),
/// tiny sessions, and dead queue entries.
#[cfg(unix)]
#[test]
fn retro_scan_skips_noise_and_drains_backlog() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    git(dir, &["init", "-q"]);
    git(dir, &["config", "user.email", "tester@example.com"]);
    git(dir, &["config", "user.name", "Tester"]);
    let (_, stderr, ok) = run_in(dir, &["init"], None);
    assert!(ok, "init failed: {stderr}");
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-qm", "seed"]);

    let stub = dir.join("stub-agent.sh");
    std::fs::write(
        &stub,
        "#!/bin/sh\ncat > /dev/null\ncat <<'JSON'\n{\"intent\":\"Backlog session\",\"decisions\":[\"One durable decision\"],\"learnings\":[],\"wiki_pages\":[]}\nJSON\n",
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

    // Fake transcript store under a fake HOME, keyed by the repo-root slug
    // exactly as the scanner computes it.
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .unwrap();
    let root = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let slug: String = root
        .chars()
        .map(|c| {
            if c == '/' || c == '\\' || c == ':' {
                '-'
            } else {
                c
            }
        })
        .collect();
    let home = tempfile::tempdir().unwrap();
    let store = home.path().join(".claude/projects").join(&slug);
    std::fs::create_dir_all(&store).unwrap();

    let hour_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
    let age = |p: &Path| {
        std::fs::File::options()
            .write(true)
            .open(p)
            .unwrap()
            .set_modified(hour_ago)
            .unwrap();
    };

    let real = store.join("realsess-0000-4444.jsonl");
    std::fs::write(&real, backlog_transcript()).unwrap();
    age(&real);
    let distiller = store.join("distsess-1111.jsonl");
    std::fs::write(
        &distiller,
        format!(
            "{}\n{}",
            r#"{"type":"user","message":{"role":"user","content":"You are the distiller for coopera, a team-context harness. Read the coding-session conversation below."}}"#,
            pad_line()
        ),
    )
    .unwrap();
    age(&distiller);
    let tiny = store.join("tinysess-2222.jsonl");
    std::fs::write(
        &tiny,
        r#"{"type":"user","message":{"role":"user","content":"hi"}}"#,
    )
    .unwrap();
    age(&tiny);
    // Fresh mtime → still live → untouched.
    std::fs::write(store.join("livesess-3333.jsonl"), backlog_transcript()).unwrap();

    // Dead queue entry → cleared without an attempt.
    std::fs::write(
        dir.join(".coopera/cache/undistilled.log"),
        "/gone/away.jsonl\n",
    )
    .unwrap();

    // A live lock blocks the whole run (no distills, no queue mutation).
    let lock = dir.join(".coopera/cache/retro.lock");
    std::fs::write(&lock, "pid 0\n").unwrap();
    let (_, stderr, ok) = run_in_env(
        dir,
        &["distill", "--retro"],
        None,
        &[("HOME", home.path().to_str().unwrap())],
    );
    assert!(ok);
    assert!(
        stderr.contains("another retro run is active"),
        "live lock must skip the run: {stderr}"
    );
    assert_eq!(
        std::fs::read_dir(dir.join("coopera/sessions"))
            .unwrap()
            .count(),
        0,
        "no distillation may happen under a live lock"
    );

    // A stale lock (crashed run) is broken and the sweep proceeds.
    age(&lock);
    let (_, stderr, ok) = run_in_env(
        dir,
        &["distill", "--retro"],
        None,
        &[("HOME", home.path().to_str().unwrap())],
    );
    assert!(ok, "retro failed: {stderr}");

    let digests: Vec<String> = std::fs::read_dir(dir.join("coopera/sessions"))
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        digests.len(),
        1,
        "only the backlog session distills: {digests:?} — {stderr}"
    );
    assert!(digests[0].ends_with("realsess.md"), "{digests:?}");
    let queue =
        std::fs::read_to_string(dir.join(".coopera/cache/undistilled.log")).unwrap_or_default();
    assert!(
        queue.trim().is_empty(),
        "dead queue entry must be cleared: {queue}"
    );
    assert!(!lock.exists(), "lock must be released after the run");

    // P0 self-heal: the next session start drains new backlog automatically
    // (COOPERA_RETRO_SYNC=1 makes the spawn synchronous for determinism).
    let real2 = store.join("nextsess-5555.jsonl");
    std::fs::write(&real2, backlog_transcript()).unwrap();
    age(&real2);
    let (_, stderr, ok) = run_in_env(
        dir,
        &["hook", "session-start"],
        Some("{}"),
        &[
            ("HOME", home.path().to_str().unwrap()),
            ("COOPERA_RETRO_SYNC", "1"),
        ],
    );
    assert!(ok, "session-start failed: {stderr}");
    let digests: Vec<String> = std::fs::read_dir(dir.join("coopera/sessions"))
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        digests.len(),
        2,
        "session start must drain the new backlog: {digests:?}"
    );
    assert!(
        digests.iter().any(|d| d.ends_with("nextsess.md")),
        "{digests:?}"
    );
}

/// P1 — Codex-style sessions (no session id in the hook payload) keep a
/// stable presence key via COOPERA_SESSION_FALLBACK: the same ref is
/// created at start, updated on prompt, and removed at end.
#[test]
fn fallback_session_key_covers_full_lifecycle() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    git(dir, &["init", "-q"]);
    git(dir, &["config", "user.email", "coder@example.com"]);
    git(dir, &["config", "user.name", "Coder"]);
    let (_, stderr, ok) = run_in(dir, &["init"], None);
    assert!(ok, "init failed: {stderr}");

    let envs: &[(&str, &str)] = &[
        ("COOPERA_SESSION_FALLBACK", "ppid-4242"),
        ("COOPERA_RETRO_SYNC", "1"),
    ];
    let refname = "refs/coopera/presence/Coder/ppid-4242";

    let (_, stderr, ok) = run_in_env(dir, &["hook", "session-start"], Some("{}"), envs);
    assert!(ok, "session-start failed: {stderr}");
    let show = |args: &[&str]| {
        let out = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .unwrap();
        (
            String::from_utf8_lossy(&out.stdout).into_owned(),
            out.status.success(),
        )
    };
    let (_, exists) = show(&["rev-parse", "--verify", "-q", refname]);
    assert!(exists, "fallback key must create a presence ref");

    let (_, _, ok) = run_in_env(
        dir,
        &["hook", "user-prompt-submit"],
        Some(r#"{"prompt":"Wire the codex adapter"}"#),
        envs,
    );
    assert!(ok);
    let (yaml, _) = show(&["cat-file", "-p", &format!("{refname}:presence.yaml")]);
    assert!(
        yaml.contains("Wire the codex adapter"),
        "intent must update via the fallback key: {yaml}"
    );

    let (_, _, ok) = run_in_env(dir, &["hook", "session-end"], Some("{}"), envs);
    assert!(ok);
    let (_, exists) = show(&["rev-parse", "--verify", "-q", refname]);
    assert!(!exists, "session-end must clean up the fallback-key ref");
}

/// M2 — presence across two clones through a bare origin: publish at
/// SessionStart, cross-visibility, intent refresh + trigger injection at
/// UserPromptSubmit, cleanup at SessionEnd (F5/F6/F7 e2e).
#[test]
fn m2_presence_cross_clone() {
    let origin = tempfile::tempdir().unwrap();
    git(origin.path(), &["init", "-q", "--bare"]);
    let work = tempfile::tempdir().unwrap();
    let a = work.path().join("a");
    let b = work.path().join("b");
    let origin_s = origin.path().to_str().unwrap();
    git(work.path(), &["clone", "-q", origin_s, a.to_str().unwrap()]);
    git(work.path(), &["clone", "-q", origin_s, b.to_str().unwrap()]);
    for (dir, name) in [(&a, "Alice"), (&b, "Bob")] {
        git(
            dir,
            &["config", "user.email", &format!("{name}@example.com")],
        );
        git(dir, &["config", "user.name", name]);
    }

    // Alice installs and starts a session -> presence ref reaches origin.
    let (_, stderr, ok) = run_in(&a, &["init"], None);
    assert!(ok, "init A failed: {stderr}");
    let (_, stderr, ok) = run_in(
        &a,
        &["hook", "session-start"],
        Some(r#"{"session_id":"sess-aaaa-1111"}"#),
    );
    assert!(ok, "session-start A failed: {stderr}");
    assert!(a.join(".coopera/cache/presence.md").exists());
    let out = Command::new("git")
        .args(["ls-remote", origin_s, "refs/coopera/presence/*"])
        .output()
        .unwrap();
    let refs = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(
        refs.contains("refs/coopera/presence/Alice/sess-aaaa-1111"),
        "Alice's presence must reach origin: {refs}"
    );

    // Bob starts a session and sees Alice in the injected activity map.
    let (_, stderr, ok) = run_in(&b, &["init"], None);
    assert!(ok, "init B failed: {stderr}");
    let (stdout, stderr, ok) = run_in(
        &b,
        &["hook", "session-start"],
        Some(r#"{"session_id":"sess-bbbb-2222"}"#),
    );
    assert!(ok, "session-start B failed: {stderr}");
    let json: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    let ctx = json["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert!(ctx.contains("Active teammate work"), "{ctx}");
    assert!(ctx.contains("Alice"), "Alice must be visible to Bob: {ctx}");
    assert!(
        json["systemMessage"]
            .as_str()
            .unwrap()
            .contains("teammate signal"),
        "{json}"
    );

    // Bob's prompt refreshes his intent (local ref) and triggers a page.
    std::fs::write(
        b.join("coopera/concepts/idem.md"),
        "---\ntitle: Idempotency\ntype: concept\nanchors: [\"src/\"]\ntriggers: [idempotency]\nsummary: Dedup via DB unique constraints.\nconfidence: high\n---\n\nBody.\n",
    )
    .unwrap();
    let (stdout, stderr, ok) = run_in(
        &b,
        &["hook", "user-prompt-submit"],
        Some(r#"{"session_id":"sess-bbbb-2222","prompt":"Work on payment idempotency now"}"#),
    );
    assert!(ok, "user-prompt-submit failed: {stderr}");
    let json: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    let ctx = json["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert!(
        ctx.contains("Dedup via DB unique constraints."),
        "trigger-matched summary must be injected: {ctx}"
    );
    let out = Command::new("git")
        .arg("-C")
        .arg(&b)
        .args([
            "cat-file",
            "-p",
            "refs/coopera/presence/Bob/sess-bbbb-2222:presence.yaml",
        ])
        .output()
        .unwrap();
    let yaml = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(
        yaml.contains("payment idempotency"),
        "intent refresh: {yaml}"
    );

    // Alice's session ends -> her ref is removed from origin.
    let (_, _, ok) = run_in(
        &a,
        &["hook", "session-end"],
        Some(r#"{"session_id":"sess-aaaa-1111"}"#),
    );
    assert!(ok);
    let out = Command::new("git")
        .args(["ls-remote", origin_s, "refs/coopera/presence/*"])
        .output()
        .unwrap();
    let refs = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(
        !refs.contains("Alice"),
        "Alice's presence must be cleaned up: {refs}"
    );
}
