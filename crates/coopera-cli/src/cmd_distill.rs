//! F3 — distillation (SKELETON). The plumbing lands here; the LLM call and
//! digest parsing are the next implementation target after scaffolding.
//!
//! TODO(F3): acceptance criteria (03-prd F3):
//! - [ ] invoke the user's own agent headlessly (`claude -p`, later
//!       `codex exec -c model_reasoning_effort="low"`) with the transcript and
//!       the fixed Digest schema (coopera_core::digest::Digest)
//! - [ ] write the rendered digest (<=30 lines) to wiki/sessions/<ts>-<id>.md
//! - [ ] when the digest contains real decisions/learnings, produce a wiki
//!       page draft (prefer editing an existing similar page) and stage it on
//!       the working branch (code-PR ride-along)
//! - [ ] run redaction (coopera_core::redact) on everything persisted
//! - [ ] on any failure: leave an undistilled marker, never fail the caller

use coopera_core::gitio::Git;

pub fn run(transcript: Option<&str>, retro: bool) -> i32 {
    let Ok(cwd) = std::env::current_dir() else {
        return 0;
    };
    let Ok(git) = Git::discover(&cwd) else {
        eprintln!("coopera distill: not a git repository");
        return 1;
    };

    if retro {
        // TODO(F3/F9): scan for undistilled markers + transcripts newer than
        // their digests, then distill them (universal fallback for crashed
        // sessions and hook-less tools like Antigravity).
        eprintln!("coopera distill --retro: not implemented yet (M1 skeleton)");
        return 0;
    }

    let Some(transcript) = transcript else {
        eprintln!("coopera distill: --transcript <path> is required (or --retro)");
        return 1;
    };

    // M1 skeleton behaviour: record that a distillation is pending so nothing
    // is silently lost. This marker doubles as the --retro work queue.
    let marker_dir = git.root.join(".coopera/cache");
    let _ = std::fs::create_dir_all(&marker_dir);
    let line = format!("{}\t{}\n", jiff::Timestamp::now(), transcript);
    let marker = marker_dir.join("undistilled.log");
    let previous = std::fs::read_to_string(&marker).unwrap_or_default();
    if std::fs::write(&marker, previous + &line).is_ok() {
        eprintln!("coopera distill: queued (distiller not implemented yet — see TODO(F3))");
    }
    0
}
