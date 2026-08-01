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
            record_distill_error(&git.root, &e);
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

/// Below this size a transcript is assumed to hold no distillable substance.
const MIN_TRANSCRIPT_BYTES: u64 = 16 * 1024;
/// Transcripts older than this are history, not backlog.
const MAX_TRANSCRIPT_AGE: Duration = Duration::from_secs(14 * 24 * 3600);
/// Head bytes sniffed from a Codex rollout. The session_meta first line
/// alone runs 12–44KB in real rollouts (it embeds the agent's base
/// instructions), and the distiller-marker check needs to see past it into
/// the first user message.
const CODEX_HEAD_BYTES: usize = 128 * 1024;

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
/// queue, and a scan of the Claude Code and Codex transcript stores for
/// sessions that never produced a digest.
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
            Err(e) => {
                record_distill_error(&git.root, &e);
                eprintln!("coopera distill --retro: {transcript}: {e:#}");
            }
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
            Err(e) => {
                record_distill_error(&git.root, &e);
                eprintln!("coopera distill --retro: {}: {e:#}", path.display());
            }
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

/// Recent transcripts for this repo that have no matching digest in
/// coopera/sessions/ — from the Claude Code store (repo-keyed directory) and
/// the Codex store (date-keyed, matched via the session_meta cwd). Digest
/// filenames end in the 8-char session prefix, so matching is mechanical.
/// Skipped: tiny transcripts (below substance), live sessions (fresh mtime
/// or recent presence), and the distiller's own sessions (recognized by the
/// fixed prompt marker — recursion guard for the offline path). Newest
/// first, so the freshest backlog drains first.
fn scan_undistilled(git: &Git, max: usize) -> Vec<PathBuf> {
    if max == 0 {
        return Vec::new();
    }
    let Some(home) = std::env::var_os("HOME") else {
        return Vec::new();
    };
    let home = PathBuf::from(home);
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
    let store = home.join(".claude/projects").join(slug);

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
    let mut candidates: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&store) {
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
            let Some(modified) = eligible_mtime(&path, now) else {
                continue;
            };
            if head_snippet(&path, 64 * 1024).contains(coopera_core::distill::PROMPT_MARKER) {
                continue; // a distiller's own session
            }
            candidates.push((modified, path));
        }
    }
    scan_codex_store(git, &home, &have, &active, now, &mut candidates);
    candidates.sort_by(|a, b| b.0.cmp(&a.0));
    candidates.into_iter().take(max).map(|(_, p)| p).collect()
}

/// Size and age gates shared by both stores, from metadata alone (no bytes
/// read). Some(mtime) when the file is big enough, old enough to not be
/// mid-write, and young enough to still be backlog.
fn eligible_mtime(path: &Path, now: std::time::SystemTime) -> Option<std::time::SystemTime> {
    let meta = path.metadata().ok()?;
    if meta.len() < MIN_TRANSCRIPT_BYTES {
        return None;
    }
    let modified = meta.modified().ok()?;
    // A clock-skewed (future) mtime reads as "just written" — skip.
    let age = now.duration_since(modified).ok()?;
    if age > MAX_TRANSCRIPT_AGE || age < Duration::from_secs(ACTIVE_MTIME_SECS) {
        return None;
    }
    Some(modified)
}

/// Scan the Codex rollout store (`~/.codex/sessions/YYYY/MM/DD/`). Repo
/// membership comes from the session_meta first line, so every size/age
/// eligible file gets a head sniff; guards mirror the Claude scan. Dedup
/// uses the same 8-char session-id prefix digest filenames end with —
/// Codex ids are UUIDv7, so two sessions started within the same ~65s
/// window share a prefix and the later one is skipped; accepted, such twins
/// are parallel spawns of one task.
fn scan_codex_store(
    git: &Git,
    home: &Path,
    have: &std::collections::HashSet<String>,
    active: &std::collections::HashSet<String>,
    now: std::time::SystemTime,
    candidates: &mut Vec<(std::time::SystemTime, PathBuf)>,
) {
    let root_canon = git.root.canonicalize().unwrap_or_else(|_| git.root.clone());
    let mut dirs: Vec<(PathBuf, u8)> = vec![(home.join(".codex/sessions"), 0)];
    while let Some((dir, depth)) = dirs.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let path = e.path();
            if path.is_dir() {
                if depth < 3 {
                    dirs.push((path, depth + 1));
                }
                continue;
            }
            let name = e.file_name().to_string_lossy().into_owned();
            if !name.starts_with("rollout-") || !name.ends_with(".jsonl") {
                continue;
            }
            let Some(modified) = eligible_mtime(&path, now) else {
                continue;
            };
            let head = head_snippet(&path, CODEX_HEAD_BYTES);
            let Some(meta) = coopera_core::distill::codex_meta(&head) else {
                continue;
            };
            if meta.id.is_empty() || !cwd_in_repo(&meta.cwd, &git.root, &root_canon) {
                continue;
            }
            let sid8: String = meta.id.chars().take(8).collect();
            if have.contains(&sid8) {
                continue;
            }
            if active.contains(&coopera_core::presence::sanitize(&meta.id)) {
                continue;
            }
            if head.contains(coopera_core::distill::PROMPT_MARKER) {
                continue; // a distiller's own session (codex exec)
            }
            candidates.push((modified, path));
        }
    }
}

/// Does the session cwd live inside this repo? Component-wise prefix match
/// on both the raw and canonicalized spellings (macOS /var ↔ /private/var);
/// subdirectory sessions belong to the repo too.
fn cwd_in_repo(cwd: &str, root: &Path, root_canon: &Path) -> bool {
    if cwd.is_empty() {
        return false;
    }
    let p = Path::new(cwd);
    let canon = p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    [root, root_canon]
        .iter()
        .any(|r| p.starts_with(r) || canon.starts_with(r))
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

/// Truncated error record — the metrics file is a counter, not a log.
fn record_distill_error(root: &Path, e: &anyhow::Error) {
    let msg: String = format!("{e:#}").chars().take(200).collect();
    coopera_core::metrics::record(root, "distill_error", &[("error", msg.into())]);
}

fn distill_one(git: &Git, transcript_path: &Path, session: Option<&str>) -> Result<String> {
    let t0 = std::time::Instant::now();
    let config = Config::load(&git.root);
    let raw = std::fs::read_to_string(transcript_path)
        .with_context(|| format!("cannot read transcript {}", transcript_path.display()))?;
    // A session_meta first line marks a Codex rollout; anything else is
    // treated as a Claude Code transcript.
    let codex = distill::codex_meta(&raw);
    let excerpt = if codex.is_some() {
        distill::extract_codex_rollout(&raw, &git.root)
    } else {
        distill::extract_transcript(&raw, &git.root)
    };
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

    // Digest — always written once past the substance threshold. Codex
    // rollout filenames don't start with the session id, so the id comes
    // from the session_meta line there.
    let session_id = session
        .map(str::to_string)
        .or_else(|| {
            codex
                .as_ref()
                .map(|m| m.id.clone())
                .filter(|id| !id.is_empty())
        })
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
        tool: if codex.is_some() {
            "codex"
        } else {
            "claude-code"
        }
        .to_string(),
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
    coopera_core::metrics::record(
        &git.root,
        "distill",
        &[
            ("decisions", (resp.decisions.len() as u64).into()),
            ("learnings", (resp.learnings.len() as u64).into()),
            ("wiki_diff", ((staged.len() - 1) as u64).into()),
            ("duration_ms", (t0.elapsed().as_millis() as u64).into()),
        ],
    );
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
