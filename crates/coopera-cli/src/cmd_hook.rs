//! Hook entrypoints (F2 injection, F3 distill wiring, F5 presence, F6 prompt
//! triggers). Contract: read hook JSON on stdin, write hook JSON on stdout,
//! and NEVER block the session — any failure degrades to an empty injection
//! with a one-line notice (fail-open, non-functional requirement).

use coopera_core::config::Config;
use coopera_core::gitio::Git;
use coopera_core::hookio::{HookInput, HookOutput};
use coopera_core::inject;
use coopera_core::presence::{self, PresenceEntry};
use coopera_core::redact::redact;
use coopera_core::staleness;
use coopera_core::wiki;
use std::io::Read;
use std::path::PathBuf;
use std::time::Duration;

/// Set on agent processes spawned by the distiller. Their sessions get no
/// injection and are never distilled — otherwise each distillation would
/// spawn another one (recursion guard).
const DISTILL_ENV: &str = "COOPERA_DISTILL";

/// Which tool is hosting this session (set by the hook command line that
/// `coopera init` writes, e.g. `COOPERA_TOOL=codex`).
const TOOL_ENV: &str = "COOPERA_TOOL";

/// Network budget for presence fetch/push at session boundaries. SessionStart
/// has a 3s overall budget (NFR); a slow remote degrades to local refs.
const NET_TIMEOUT: Duration = Duration::from_secs(2);

fn read_input() -> HookInput {
    let mut buf = String::new();
    let _ = std::io::stdin().read_to_string(&mut buf);
    HookInput::from_stdin_str(&buf)
}

fn working_dir(input: &HookInput) -> PathBuf {
    input
        .cwd
        .as_ref()
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
}

fn tool_name() -> String {
    std::env::var(TOOL_ENV).unwrap_or_else(|_| "claude-code".to_string())
}

fn emit(output: &HookOutput) -> i32 {
    match serde_json::to_string(output) {
        Ok(json) => {
            println!("{json}");
            0
        }
        Err(_) => 0, // even a serialization bug must not block the session
    }
}

/// F2+F5+F7 — SessionStart: fetch teammates' presence, publish our own entry,
/// then inject the activity map + wiki pack under the hard token budget.
pub fn session_start() -> i32 {
    if std::env::var_os(DISTILL_ENV).is_some() {
        return emit(&HookOutput::session_start(String::new(), None));
    }
    let input = read_input();
    let dir = working_dir(&input);

    let output = match Git::discover(&dir) {
        Ok(git) => {
            let config = Config::load(&git.root);
            let (pages, errors) = wiki::load_wiki(&git.root);
            let branch = git.current_branch();
            let changed = git.changed_files();
            let stale = staleness::find_stale(&git, &pages);

            // F5: bounded fetch, then announce this session (sync push — the
            // session boundary is the moment teammates should see us).
            let fetched = presence::fetch(&git, NET_TIMEOUT);
            let session = input
                .session_id
                .clone()
                .unwrap_or_else(|| format!("pid-{}", std::process::id()));
            let now = jiff::Timestamp::now();
            let entry = PresenceEntry {
                user: git.user_name(),
                session: session.clone(),
                tool: tool_name(),
                branch: branch.clone(),
                started: now.to_string(),
                last_seen: now.to_string(),
                intent: "session started".to_string(),
                status: "active".to_string(),
            };
            let _ = presence::publish(&git, &entry, true, NET_TIMEOUT);

            // F7: pushed branches are the primary work signal (push axis).
            let entries = presence::load_all(&git);
            let branches = git.recent_remote_branches(5);
            let plines = presence::format_lines(&entries, &branches, now, Some(&session));
            let _ = presence::materialize(&git.root, &plines);

            let pack =
                inject::build_pack(&pages, &branch, &changed, &config.budget, &stale, &plines);
            let mut msg = if pack.items == 0 && pack.stale == 0 && pack.presence_items == 0 {
                "coopera: no team context yet (wiki is empty)".to_string()
            } else {
                format!(
                    "coopera: injected {} items (~{} tokens)",
                    pack.items, pack.tokens
                )
            };
            if pack.presence_items > 0 {
                msg.push_str(&format!(", {} teammate signal(s)", pack.presence_items));
            }
            if pack.stale > 0 {
                msg.push_str(&format!(", {} stale excluded", pack.stale));
            }
            if !fetched && git.has_origin() {
                msg.push_str(" — presence fetch skipped (network)");
            }
            if !errors.is_empty() {
                msg.push_str(&format!(
                    " — {} unparseable wiki page(s), run `coopera wiki lint`",
                    errors.len()
                ));
            }
            HookOutput::session_start(pack.text, Some(msg))
        }
        // Fail-open: outside a repo (or git missing) we inject nothing.
        Err(e) => {
            HookOutput::session_start(String::new(), Some(format!("coopera: inactive ({e})")))
        }
    };
    emit(&output)
}

/// F6 — UserPromptSubmit: refresh this session's intent on the presence
/// board (local now, remote in background — this hook must stay fast) and
/// inject trigger-matched page summaries.
pub fn user_prompt_submit() -> i32 {
    if std::env::var_os(DISTILL_ENV).is_some() {
        return emit(&HookOutput::for_event(
            "UserPromptSubmit",
            String::new(),
            None,
        ));
    }
    let input = read_input();
    let dir = working_dir(&input);

    let output = match Git::discover(&dir) {
        Ok(git) => {
            if let (Some(prompt), Some(session)) = (&input.prompt, &input.session_id) {
                let user = git.user_name();
                if let Some(mut entry) = presence::load(&git, &user, session) {
                    let intent: String = redact(prompt)
                        .replace('\n', " ")
                        .chars()
                        .take(120)
                        .collect();
                    if !intent.trim().is_empty() {
                        entry.intent = intent;
                    }
                    entry.last_seen = jiff::Timestamp::now().to_string();
                    entry.branch = git.current_branch();
                    // Background push: prompt latency budget is ~100ms local.
                    let _ = presence::publish(&git, &entry, false, NET_TIMEOUT);
                }
            }

            let (context, hits) = input
                .prompt
                .as_deref()
                .map(|p| trigger_context(&git, p))
                .unwrap_or_default();
            let msg = if hits > 0 {
                Some(format!("coopera: +{hits} page(s) matched this prompt"))
            } else {
                None
            };
            HookOutput::for_event("UserPromptSubmit", context, msg)
        }
        Err(_) => HookOutput::for_event("UserPromptSubmit", String::new(), None),
    };
    emit(&output)
}

/// Keyword triggers (page frontmatter `triggers:`) matched against the
/// prompt — "the moment you type 'idempotency', the team's definition
/// follows" (00-harness §2.2). Local files only; staleness check is skipped
/// here for latency, [unreviewed] markers still apply.
fn trigger_context(git: &Git, prompt: &str) -> (String, usize) {
    let lower = prompt.to_lowercase();
    let (pages, _) = wiki::load_wiki(&git.root);
    let mut lines: Vec<String> = Vec::new();
    for page in &pages {
        let hit = page.front.triggers.iter().any(|t| {
            let t = t.trim().to_lowercase();
            !t.is_empty() && lower.contains(&t)
        });
        if !hit {
            continue;
        }
        let rel = page.path.to_string_lossy().replace('\\', "/");
        let rel = rel
            .split("coopera/")
            .last()
            .map(|s| format!("coopera/{s}"))
            .unwrap_or_else(|| rel.to_string());
        let marker = if page.front.confidence == "draft" {
            " [unreviewed]"
        } else {
            ""
        };
        lines.push(format!(
            "- [{}]{} {} — {} ({})",
            page.front.page_type,
            marker,
            page.front.title,
            page.front.summary.replace('\n', " "),
            rel
        ));
        if lines.len() >= 3 {
            break;
        }
    }
    if lines.is_empty() {
        return (String::new(), 0);
    }
    let n = lines.len();
    (
        format!(
            "<team-context source=\"coopera\" trigger=\"keyword\">\nTeam knowledge relevant to this request:\n{}\n</team-context>",
            lines.join("\n")
        ),
        n,
    )
}

/// F3+F5 — SessionEnd: clean up this session's presence ref and spawn
/// background distillation. Exits immediately; teardown is never delayed.
pub fn session_end() -> i32 {
    if std::env::var_os(DISTILL_ENV).is_some() {
        return 0; // never distill the distiller's own session
    }
    let input = read_input();
    let dir = working_dir(&input);

    if let Ok(git) = Git::discover(&dir) {
        if let Some(session) = &input.session_id {
            presence::remove(&git, &git.user_name(), session, NET_TIMEOUT);
        }
        if let Some(transcript) = &input.transcript_path {
            let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("coopera"));
            let mut cmd = std::process::Command::new(exe);
            cmd.arg("distill").arg("--transcript").arg(transcript);
            if let Some(session) = &input.session_id {
                cmd.arg("--session").arg(session);
            }
            // Detached spawn (own process group): we intentionally do not
            // wait, and hook teardown must not be able to kill it mid-run.
            cmd.current_dir(&git.root)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());
            let _ = coopera_core::spawn::detach(&mut cmd).spawn();
        }
    }
    0
}
