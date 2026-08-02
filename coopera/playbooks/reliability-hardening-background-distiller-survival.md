---
title: 'Reliability hardening: background distiller survival'
type: playbook
anchors:
- crates/coopera-cli/src/cmd_hook.rs
- crates/coopera-core/src/
triggers:
- distill
- retro
- session-start
- background
- setsid
- detach
summary: Background distillers must be detached into their own process group or the hook's cleanup will kill them. Session-start auto-drains the retro queue as a self-healing fallback. distill.log provides the only visibility into background distillation outcomes.
last_verified: 0b1bcf4
confidence: draft
source: coopera/sessions/20260802-000520-ed2016df.md
---

## Background distiller survival

### Root cause of silent distillation death

Hook processes run inside the host app's process group. When the app terminates a session it sends SIGTERM/SIGHUP to the whole group. A distiller spawned without `setsid` (or equivalent detach) is in that group and dies mid-run — even though the session closed cleanly.

Fix: `spawn::detach()` before execing the distiller child. This moves it to a new process group that survives the parent's shutdown.

### Self-healing retro loop

If SessionEnd fires but the distiller is killed anyway, the transcript lands in `.coopera/cache/undistilled.log`. Session-start reads this queue and drains up to 3 items per start:

- Lockfile (`.coopera/cache/retro.lock`) prevents concurrent drains
- Stale lock released after 30 min (handles crash-without-cleanup)
- Active sessions excluded: mtime < 10 min OR presence ref < 30 min old
- Distiller transcripts excluded via PROMPT_MARKER sniff (first 128 KB)

This means a killed distiller is retried automatically on the next session open, with no user intervention.

### distill.log

All distiller stdout/stderr is captured to `.coopera/cache/distill.log` (512 KB cap, rotated). This is the only post-mortem signal when the distiller runs in the background. Check it when `metrics.jsonl` shows `distill_error` events.

### Distiller transcript exclusion

`claude -p` (and other headless calls) leave a JSONL in `~/.claude/projects/`. Without exclusion, retro scans these as pending team sessions and wastes distiller calls on distiller output. The PROMPT_MARKER (a known string injected at the top of every distill prompt) reliably identifies these files for skipping.
