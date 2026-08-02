---
title: Antigravity hook contract (agy 1.1.9)
type: module
anchors:
- crates/coopera-core/src/
- crates/coopera-cli/src/cmd_hook.rs
triggers:
- antigravity
- agy
- hooks.json
- PreInvocation
- Stop
summary: Antigravity hooks live in .agents/hooks.json as named entries. PreInvocation fires before every model call (coopera injects on invocationNum 1 only); Stop fires when each turn's execution loop ends (capture marker — distillation waits for quiescence). All payloads carry a stable conversationId and transcriptPath.
last_verified: 0b1bcf4
confidence: draft
source: coopera/sessions/20260802-000520-ed2016df.md
---

## Antigravity hook contract (agy 1.1.9)

### Config location

`.agents/hooks.json` in the repo root — named hook objects merge natively. coopera occupies the `"coopera"` key; other named hooks are preserved.

```json
{
  "coopera": {
    "PreInvocation": [
      { "type": "command", "command": "...", "timeout": 15 }
    ],
    "Stop": [
      { "type": "command", "command": "...", "timeout": 15 }
    ]
  }
}
```

`timeout` is in seconds (default 30). Handlers run via `sh -c` with cwd = the directory containing hooks.json.

### Events used by coopera

| Event | When it fires | coopera action |
|---|---|---|
| PreInvocation | Before every model call | Inject team context via `injectSteps[].ephemeralMessage` on `invocationNum` 1 only; refresh presence on every call |
| Stop | Each turn's execution loop ends (NOT conversation end — that moment does not exist) | Queue transcript for capture; retro distills after 10min quiescence |

### Payload fields (camelCase protojson)

- `conversationId` — stable across all hooks in one conversation; used as the presence key
- `transcriptPath` — absolute path to the rolling JSONL; the file is always named `transcript.jsonl`, so the queue records the conversationId alongside it
- `invocationNum` (PreInvocation) — 1-based model-call counter; inject only when <= 1
- `workspacePaths` — workspace roots (agy payloads carry no cwd)

### PreInvocation response

```json
{"injectSteps": [{"ephemeralMessage": "<team context text>"}]}
```

A bare `{}` response is a no-op (safe fail-open).

### Headless anti-recursion

`agy -p` (headless) loads only global hooks and fires none (measured on 1.1.9). Distiller agy calls are therefore anti-recursive even without the COOPERA_DISTILL guard.

### Transcript format

JSONL steps with `step_index`/`source`/`type`. `type: USER_INPUT` carries the developer's prompt wrapped in `<USER_REQUEST>...</USER_REQUEST>` (surrounding metadata blocks are chrome); `type: PLANNER_RESPONSE` carries model text plus a `tool_calls` array whose args hold double-quoted path values (e.g. `TargetFile`). Detected by `antigravity_detect()` (step_index/source shape).

### Trust gate

Workspace hooks only load for trusted folders (`trustedWorkspaces` in the agy settings). Trust is granted once interactively; headless runs never prompt.

### Known limits

- No SessionStart/SessionEnd equivalents — presence is emitted on first PreInvocation; cleanup relies on the 24h lazy GC.
- agy stores transcripts under `~/.gemini/<product>/brain/<id>/.system_generated/logs/transcript.jsonl`; workspace matching for a retroactive scan of past sessions is an open problem.
