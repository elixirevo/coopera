---
title: Distillation by each agent, not centralized
type: decision
anchors:
- crates/coopera-cli/src/distill.rs
triggers:
- distill
- session end
- summarize
summary: Each tool distills its own sessions via its own CLI — claude-code → `claude -p` (stdin), codex → `codex exec --ephemeral -s read-only … -o COOPERA_OUT -` (response file), antigravity → `agy --effort low -p COOPERA_PROMPT` (argv). Fallback chain tries installed alternatives; no CLI at all leaves the item queued with a clear error.
last_verified: c45d2bb
confidence: draft
source: coopera/sessions/20260802-000520-ed2016df.md
---

Centralized distillation requires shared API key (cost ambiguity) or hosted server (violates zero-servers). Instead:

- **Claude Code**: SessionEnd handler spawns distiller agent → JSON schema result
- **Codex**: SessionEnd command → `codex exec distill` subprocess → Bash CLI invocation
- **Antigravity**: No SessionEnd hook — SessionStart detects previous undistilled session, triggers backfill

Each user's model choice is local (Opus/Sonnet/Haiku). Quality validated via golden-set regression before deployment.


### Session update
## Implementation detail (completed 2026-08-02)

Two placeholders handle the three calling conventions:

- `COOPERA_OUT` — replaced with a temp file path; response is read from that file instead of stdout. Needed for `codex exec` whose stdout is a streaming event log that would corrupt JSON parsing.
- `COOPERA_PROMPT` — replaced with the full prompt text as a single argv token (120 KB guard to stay under Linux's per-arg limit). Needed for `agy -p` which ignores stdin entirely.

Built-in command strings:

| Tool | Command |
|---|---|
| claude-code | `claude -p` (prompt via stdin) |
| codex | `codex exec --ephemeral -s read-only -c model_reasoning_effort="low" -o COOPERA_OUT -` |
| antigravity | `agy --effort low --print-timeout 600s -p COOPERA_PROMPT` |

Fallback order when the hosting tool's CLI is absent: try other installed CLIs (claude → codex → agy) so single-tool users still get distillation. If none found, item stays queued and the distill.log records a clear error.

Config overrides: `[distill.agents.<tool>]` per-tool, `[distill] command` global (backward-compatible).

### Anti-recursion

`agy -p` (headless) does not fire Antigravity hooks — distiller agy calls are anti-recursive for free. `codex exec --ephemeral` does not create a rollout session. `claude -p` is guarded by the existing `COOPERA_DISTILL` env var.
