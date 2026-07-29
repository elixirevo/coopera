//! F2/F3 hook entrypoints. Contract: read hook JSON on stdin, write hook JSON
//! on stdout, and NEVER block the session — any failure degrades to an empty
//! injection with a one-line notice (fail-open, non-functional requirement).

use coopera_core::config::Config;
use coopera_core::gitio::Git;
use coopera_core::hookio::{HookInput, HookOutput};
use coopera_core::inject;
use coopera_core::staleness;
use coopera_core::wiki;
use std::io::Read;
use std::path::PathBuf;

/// Set on agent processes spawned by the distiller. Their sessions get no
/// injection and are never distilled — otherwise each distillation would
/// spawn another one (recursion guard).
const DISTILL_ENV: &str = "COOPERA_DISTILL";

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

/// F2 — SessionStart: build the injection pack from the wiki (L0+L1) under the
/// hard token budget, with a one-line visible trace.
pub fn session_start() -> i32 {
    if std::env::var_os(DISTILL_ENV).is_some() {
        let quiet = HookOutput::session_start(String::new(), None);
        println!(
            "{}",
            serde_json::to_string(&quiet).unwrap_or_else(|_| "{}".to_string())
        );
        return 0;
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
            let pack = inject::build_pack(&pages, &branch, &changed, &config.budget, &stale);
            let mut msg = if pack.items == 0 && pack.stale == 0 {
                "coopera: no team context yet (wiki is empty)".to_string()
            } else {
                format!(
                    "coopera: injected {} items (~{} tokens)",
                    pack.items, pack.tokens
                )
            };
            if pack.stale > 0 {
                msg.push_str(&format!(", {} stale excluded", pack.stale));
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

    match serde_json::to_string(&output) {
        Ok(json) => {
            println!("{json}");
            0
        }
        Err(_) => 0, // even a serialization bug must not block the session
    }
}

/// F3 — SessionEnd: spawn background distillation (detached) and exit
/// immediately so session teardown is never delayed. The distiller itself
/// lives in cmd_distill.
pub fn session_end() -> i32 {
    if std::env::var_os(DISTILL_ENV).is_some() {
        return 0; // never distill the distiller's own session
    }
    let input = read_input();
    let dir = working_dir(&input);

    if let (Some(transcript), Ok(git)) = (&input.transcript_path, Git::discover(&dir)) {
        let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("coopera"));
        let mut cmd = std::process::Command::new(exe);
        cmd.arg("distill").arg("--transcript").arg(transcript);
        if let Some(session) = &input.session_id {
            cmd.arg("--session").arg(session);
        }
        // Detached spawn: we intentionally do not wait.
        let _ = cmd
            .current_dir(&git.root)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }
    0
}
