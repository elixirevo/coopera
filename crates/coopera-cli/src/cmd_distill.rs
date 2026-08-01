//! F3 — distillation orchestration: transcript → user's own agent (headless)
//! → fixed-schema digest in coopera/sessions/ → wiki drafts staged on the
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

/// Each distillation is an agent call on the user's own subscription — a
/// single retro run stays cost-bounded; the rest drains on later runs.
const RETRO_MAX_PER_RUN: usize = 3;
/// A transcript written this recently belongs to a session that is likely
/// still live (Claude Code appends continuously) — never distill mid-flight.
const ACTIVE_MTIME_SECS: u64 = 10 * 60;
/// A presence entry seen this recently also marks its session as live even
/// when the transcript has been quiet (user thinking, long tool run).
const ACTIVE_PRESENCE_SECS: i64 = 30 * 60;

/// A lock this old belongs to a crashed run — break it. Kept above the worst
/// honest case (one distillation ≤ 600s; mtime is refreshed between items).
const LOCK_STALE_SECS: u64 = 30 * 60;

fn lock_path(root: &Path) -> PathBuf {
    root.join(".coopera/cache/retro.lock")
}

/// Cross-process guard: one retro run per repo. Session starts spawn retro
/// unconditionally; without this, parallel sessions would double-distill the
/// same backlog (and double-spend agent calls).
struct RetroLock {
    path: PathBuf,
}

impl RetroLock {
    fn acquire(root: &Path) -> Option<RetroLock> {
        let path = lock_path(root);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        for _ in 0..2 {
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(mut f) => {
                    let _ = writeln!(f, "pid {}", std::process::id());
                    return Some(RetroLock { path });
                }
                Err(_) => {
                    let stale = path
                        .metadata()
                        .and_then(|m| m.modified())
                        .ok()
                        .and_then(|m| std::time::SystemTime::now().duration_since(m).ok())
                        .map(|age| age.as_secs() > LOCK_STALE_SECS)
                        // Metadata unreadable = lock vanished mid-check: retry.
                        .unwrap_or(true);
                    if !stale {
                        return None;
                    }
                    let _ = std::fs::remove_file(&path);
                }
            }
        }
        None
    }

    /// Refresh mtime so a long (but honest) run is not mistaken for stale.
    fn touch(&self) {
        if let Ok(f) = std::fs::OpenOptions::new().write(true).open(&self.path) {
            let _ = f.set_modified(std::time::SystemTime::now());
        }
    }
}

impl Drop for RetroLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// F9 — retroactive distillation, the universal fallback for crashed
/// sessions and hook-less tools (Antigravity). Two sources: the failure
/// queue, and a scan of the Claude Code transcript store for sessions that
/// never produced a digest.
fn run_retro(git: &Git) -> i32 {
    let Some(lock) = RetroLock::acquire(&git.root) else {
        eprintln!("coopera distill --retro: another retro run is active — skipping");
        return 0;
    };
    let mut done = 0usize;
    let mut seen = 0usize;
    let mut attempts = 0usize;

    let pending = read_pending(&git.root);
    for transcript in &pending {
        let path = Path::new(transcript);
        if !path.exists() {
            clear_pending(&git.root, transcript);
            continue;
        }
        if attempts >= RETRO_MAX_PER_RUN {
            break; // leave the rest for the next run
        }
        attempts += 1;
        seen += 1;
        match distill_one(git, path, None) {
            Ok(summary) => {
                clear_pending(&git.root, transcript);
                eprintln!("{summary}");
                done += 1;
            }
            Err(e) => eprintln!("coopera distill --retro: {transcript}: {e:#}"),
        }
        lock.touch();
    }

    for path in scan_undistilled(git, RETRO_MAX_PER_RUN.saturating_sub(attempts)) {
        seen += 1;
        match distill_one(git, &path, None) {
            Ok(summary) => {
                eprintln!("{summary}");
                done += 1;
            }
            Err(e) => eprintln!("coopera distill --retro: {}: {e:#}", path.display()),
        }
        lock.touch();
    }

    if seen == 0 {
        eprintln!(
            "[{}] coopera distill --retro: nothing to do",
            jiff::Timestamp::now()
        );
    } else {
        eprintln!(
            "[{}] coopera distill --retro: {done}/{seen} processed",
            jiff::Timestamp::now()
        );
    }
    0
}

/// Recent Claude Code transcripts for this repo that have no matching digest
/// in coopera/sessions/. Digest filenames end in the 8-char session prefix, so
/// matching is mechanical. Skipped: tiny transcripts (below substance), live
/// sessions (fresh mtime or recent presence), and the distiller's own
/// sessions (recognized by the fixed prompt marker — recursion guard for the
/// offline path). Newest first, so the freshest backlog drains first.
fn scan_undistilled(git: &Git, max: usize) -> Vec<PathBuf> {
    if max == 0 {
        return Vec::new();
    }
    let Some(home) = std::env::var_os("HOME") else {
        return Vec::new();
    };
    let slug: String = git
        .root
        .to_string_lossy()
        .chars()
        .map(|c| {
            if c == '/' || c == '\\' || c == ':' {
                '-'
            } else {
                c
            }
        })
        .collect();
    let store = PathBuf::from(home).join(".claude/projects").join(slug);

    let mut have: std::collections::HashSet<String> = std::collections::HashSet::new();
    if let Ok(entries) = std::fs::read_dir(git.root.join("coopera/sessions")) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if let Some(sid) = name.strip_suffix(".md").and_then(|n| n.rsplit('-').next()) {
                have.insert(sid.to_string());
            }
        }
    }
    let active = coopera_core::presence::recent_session_ids(
        git,
        jiff::Timestamp::now(),
        ACTIVE_PRESENCE_SECS,
    );

    let now = std::time::SystemTime::now();
    let Ok(entries) = std::fs::read_dir(&store) else {
        return Vec::new();
    };
    let mut candidates: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();
    for path in entries.flatten().map(|e| e.path()) {
        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }
        let Some(stem) = path.file_stem().map(|s| s.to_string_lossy().into_owned()) else {
            continue;
        };
        let sid8: String = stem.chars().take(8).collect();
        if have.contains(&sid8) {
            continue;
        }
        if active.contains(&coopera_core::presence::sanitize(&stem)) {
            continue; // session still live per presence board
        }
        let Ok(meta) = path.metadata() else { continue };
        if meta.len() < 16 * 1024 {
            continue;
        }
        let Ok(modified) = meta.modified() else {
            continue;
        };
        // A clock-skewed (future) mtime reads as "just written" — skip.
        let Ok(age) = now.duration_since(modified) else {
            continue;
        };
        if age > Duration::from_secs(14 * 24 * 3600) {
            continue;
        }
        if age < Duration::from_secs(ACTIVE_MTIME_SECS) {
            continue; // still being written
        }
        if head_snippet(&path, 64 * 1024).contains(coopera_core::distill::PROMPT_MARKER) {
            continue; // a distiller's own session
        }
        candidates.push((modified, path));
    }
    candidates.sort_by(|a, b| b.0.cmp(&a.0));
    candidates.into_iter().take(max).map(|(_, p)| p).collect()
}

/// First `bytes` of a file, lossily decoded — enough to fingerprint distiller
/// transcripts without reading multi-MB session files whole.
fn head_snippet(path: &Path, bytes: usize) -> String {
    use std::io::Read as _;
    let Ok(f) = std::fs::File::open(path) else {
        return String::new();
    };
    let mut buf = Vec::with_capacity(bytes.min(64 * 1024));
    let _ = f.take(bytes as u64).read_to_end(&mut buf);
    String::from_utf8_lossy(&buf).into_owned()
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
    let digest_rel = format!("coopera/sessions/{stamp}-{short}.md");
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

/// Where hook-spawned distiller output lands. Background distillation used to
/// discard stderr entirely — failures were invisible, which read as "coopera
/// does nothing" (the visibility principle of decision 006 applies to the
/// capture axis too).
pub(crate) fn log_path(root: &Path) -> PathBuf {
    root.join(".coopera/cache/distill.log")
}

/// Append one line to the distill log, fail-open. Capped: past 512KB the log
/// is truncated first (it is a local debug aid, not a durable record).
pub(crate) fn append_log(root: &Path, line: &str) {
    let path = log_path(root);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(meta) = path.metadata() {
        if meta.len() > 512 * 1024 {
            let _ = std::fs::write(&path, "");
        }
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = writeln!(f, "{line}");
    }
}

/// Sessions still waiting for distillation (failed or never attempted).
pub(crate) fn pending_count(root: &Path) -> usize {
    read_pending(root).len()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_add_read_clear_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        assert_eq!(pending_count(root), 0);
        add_pending(root, "/a.jsonl");
        add_pending(root, "/b.jsonl");
        add_pending(root, "/a.jsonl"); // dedup
        assert_eq!(read_pending(root), vec!["/a.jsonl", "/b.jsonl"]);
        assert_eq!(pending_count(root), 2);
        clear_pending(root, "/a.jsonl");
        assert_eq!(read_pending(root), vec!["/b.jsonl"]);
    }

    #[test]
    fn append_log_writes_and_truncates_past_cap() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        append_log(root, "first line");
        let content = std::fs::read_to_string(log_path(root)).unwrap();
        assert!(content.contains("first line"));

        std::fs::write(log_path(root), "x".repeat(600 * 1024)).unwrap();
        append_log(root, "after rotation");
        let content = std::fs::read_to_string(log_path(root)).unwrap();
        assert!(content.contains("after rotation"));
        assert!(content.len() < 1024, "oversized log must be truncated");
    }
}
