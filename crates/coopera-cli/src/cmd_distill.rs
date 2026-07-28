//! F3 — distillation orchestration: transcript → user's own agent (headless)
//! → fixed-schema digest in wiki/sessions/ → wiki drafts staged on the
//! working branch. Fail-open: failures leave the transcript in the retro
//! queue (`.coopera/cache/undistilled.log`) and never fail the caller.

use anyhow::{bail, Context, Result};
use coopera_core::config::Config;
use coopera_core::digest::Digest;
use coopera_core::distill;
use coopera_core::gitio::Git;
use coopera_core::redact::redact;
use coopera_core::wiki;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

pub fn run(transcript: Option<&str>, session: Option<&str>, retro: bool) -> i32 {
    let Ok(cwd) = std::env::current_dir() else {
        return 0;
    };
    let Ok(git) = Git::discover(&cwd) else {
        eprintln!("coopera distill: not a git repository");
        return 1;
    };

    if retro {
        return run_retro(&git);
    }
    let Some(transcript) = transcript else {
        eprintln!("coopera distill: --transcript <path> is required (or --retro)");
        return 1;
    };

    // Crash safety: queue first, clear only after success (F9 retro reuses this).
    add_pending(&git.root, transcript);
    match distill_one(&git, Path::new(transcript), session) {
        Ok(summary) => {
            clear_pending(&git.root, transcript);
            eprintln!("{summary}");
            0
        }
        Err(e) => {
            eprintln!("coopera distill: failed ({e:#}) — kept in retro queue");
            0
        }
    }
}

/// F9 groundwork: process everything left in the retro queue (crashed
/// sessions, hook-less tools). Each entry clears itself on success.
fn run_retro(git: &Git) -> i32 {
    let pending = read_pending(&git.root);
    if pending.is_empty() {
        eprintln!("coopera distill --retro: queue is empty");
        return 0;
    }
    let mut done = 0usize;
    for transcript in &pending {
        let path = Path::new(transcript);
        if !path.exists() {
            clear_pending(&git.root, transcript);
            continue;
        }
        match distill_one(git, path, None) {
            Ok(summary) => {
                clear_pending(&git.root, transcript);
                eprintln!("{summary}");
                done += 1;
            }
            Err(e) => eprintln!("coopera distill --retro: {transcript}: {e:#}"),
        }
    }
    eprintln!(
        "coopera distill --retro: {done}/{} processed",
        pending.len()
    );
    0
}

fn distill_one(git: &Git, transcript_path: &Path, session: Option<&str>) -> Result<String> {
    let config = Config::load(&git.root);
    let raw = std::fs::read_to_string(transcript_path)
        .with_context(|| format!("cannot read transcript {}", transcript_path.display()))?;
    let excerpt = distill::extract_transcript(&raw, &git.root);
    if excerpt.messages < config.distill.min_messages {
        return Ok(format!(
            "coopera distill: skipped — {} message(s) below substance threshold ({})",
            excerpt.messages, config.distill.min_messages
        ));
    }

    let (pages, _) = wiki::load_wiki(&git.root);
    let prompt = distill::build_prompt(&excerpt, &pages, &git.root);
    let reply = run_agent(&config, &prompt, &git.root)?;
    let resp = distill::parse_response(&reply)?;

    // Digest — always written once past the substance threshold.
    let session_id = session
        .map(str::to_string)
        .or_else(|| {
            transcript_path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "unknown".to_string());
    let short: String = session_id.chars().take(8).collect();
    let now = jiff::Timestamp::now();
    let stamp = now.strftime("%Y%m%d-%H%M%S").to_string();
    let digest = Digest {
        session: session_id,
        author: git.user_name(),
        tool: "claude-code".to_string(),
        timestamp: now.to_string(),
        intent: resp.intent.clone(),
        decisions: resp.decisions.clone(),
        learnings: resp.learnings.clone(),
        touched: excerpt.touched.clone(),
    };
    let digest_rel = format!("wiki/sessions/{stamp}-{short}.md");
    let digest_abs = git.root.join(&digest_rel);
    if let Some(parent) = digest_abs.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&digest_abs, redact(&digest.render()) + "\n")?;

    // Wiki drafts — only when the session had substance (PRD F3).
    let mut staged = vec![digest_rel.clone()];
    let mut drafts_note = String::new();
    if !resp.decisions.is_empty() || !resp.learnings.is_empty() {
        let head = git.head_short();
        let m = distill::materialize_pages(
            &git.root,
            &resp.wiki_pages,
            &excerpt.touched,
            &digest_rel,
            head.as_deref(),
        );
        for s in &m.skipped {
            eprintln!("coopera distill: draft skipped — {s}");
        }
        if !m.written.is_empty() {
            drafts_note = format!(", wiki drafts: {}", m.written.join(", "));
            staged.extend(m.written);
        }
    }

    // Code-PR ride-along: stage so the next commit carries the wiki change.
    git.add(&staged)?;
    Ok(format!(
        "coopera distill: {digest_rel} ({} decision(s), {} learning(s)){drafts_note}",
        resp.decisions.len(),
        resp.learnings.len()
    ))
}

/// Run the user's own agent headlessly, prompt on stdin. COOPERA_DISTILL=1
/// marks the child session so our own hooks neither inject into it nor try
/// to distill it (recursion guard).
///
/// Pipe-buffer note: replies are small JSON (well under the 64KB pipe buffer),
/// so polling try_wait before draining stdout cannot deadlock here.
fn run_agent(config: &Config, prompt: &str, cwd: &Path) -> Result<String> {
    let d = &config.distill;
    let mut child = std::process::Command::new(&d.command)
        .args(&d.args)
        .current_dir(cwd)
        .env("COOPERA_DISTILL", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("failed to start distiller agent '{}'", d.command))?;

    child
        .stdin
        .take()
        .context("no stdin handle")?
        .write_all(prompt.as_bytes())
        .context("failed to send prompt to distiller agent")?;

    let deadline = Instant::now() + Duration::from_secs(d.timeout_secs);
    loop {
        if child.try_wait()?.is_some() {
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            bail!("distiller agent timed out after {}s", d.timeout_secs);
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    let out = child.wait_with_output()?;
    if !out.status.success() {
        let err: String = String::from_utf8_lossy(&out.stderr)
            .chars()
            .take(400)
            .collect();
        bail!("distiller agent exited with {}: {err}", out.status);
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn queue_path(root: &Path) -> PathBuf {
    root.join(".coopera/cache/undistilled.log")
}

fn read_pending(root: &Path) -> Vec<String> {
    std::fs::read_to_string(queue_path(root))
        .unwrap_or_default()
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

fn add_pending(root: &Path, transcript: &str) {
    let mut lines = read_pending(root);
    if !lines.iter().any(|l| l == transcript) {
        lines.push(transcript.to_string());
    }
    let path = queue_path(root);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&path, lines.join("\n") + "\n");
}

fn clear_pending(root: &Path, transcript: &str) {
    let lines: Vec<String> = read_pending(root)
        .into_iter()
        .filter(|l| l != transcript)
        .collect();
    let _ = std::fs::write(queue_path(root), lines.join("\n") + "\n");
}
