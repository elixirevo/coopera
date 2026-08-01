---
title: 'Codex rollout schema: response_item is canonical, event_msg is skipped'
type: decision
anchors:
- crates/coopera-core/src/distill.rs
triggers:
- codex
- rollout
- event_msg
- response_item
- apply_patch
summary: In Codex JSONL rollouts, response_item lines carry the durable conversation; event_msg lines are streaming duplicates and must be skipped. System-injected role:user packets (environment_context, turn_aborted, AGENTS.md dumps) are filtered by prefix. Edits appear as custom_tool_call apply_patch envelopes with paths relative to session cwd.
last_verified: a59914d
confidence: draft
source: coopera/sessions/20260801-221820-85014d16.md
---

## Codex rollout JSONL schema

### First line: session_meta
```json
{"timestamp": "...", "type": "session_meta", "payload": {"id": "<uuid>", "cwd": "/abs/path/to/repo", ...}}
```
- `payload.id` is the session identifier (used for digest dedup, 8-char prefix).
- `payload.cwd` identifies the repo — the store is not repo-keyed, so this is the only matching signal.
- The first line runs 12–44 KB because it embeds base instructions; head sniff for marker detection must use 128 KB (not 16 KB).

### Subsequent lines
| `type` | Role | Action |
|---|---|---|
| `response_item` | user / assistant / developer / reasoning | **Canonical** — use these |
| `event_msg` | same | Streaming duplicate — **skip** |
| `custom_tool_call` | — | Tool invocations, including `apply_patch` |

### System-injected role:user packets (filter by prefix)
- `<environment_context>` — env snapshot
- `<turn_aborted>` — abort marker
- `# AGENTS.md instructions for …` — plain-text dump, no XML wrapper (~250 occurrences per real corpus)

### Touched files
Derive from `custom_tool_call` lines where `payload.name == "apply_patch"`. Paths in the patch envelope may be absolute or relative; relative ones resolve against `payload.cwd` (session cwd), which may be a repo subdirectory — not against the repo root.

## Stability
Verified across 318 real rollouts spanning Codex CLI 0.104 to 0.146 (2026-08-02). Schema is stable for this extractor.
