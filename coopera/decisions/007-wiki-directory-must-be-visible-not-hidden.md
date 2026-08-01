---
title: Wiki directory must be visible, not hidden
type: decision
anchors:
- crates/coopera-core/src/inject.rs
- crates/coopera-cli/src/cmd_init.rs
triggers:
- wiki
- directory
- hidden
- dotdir
- discoverability
- ripgrep
summary: The shared wiki lives in coopera/ (visible, committed) not in .wiki or .coopera/wiki. ripgrep skips dotdirs by default, making a hidden wiki invisible to agent search — which breaks the product's core value for uninstalled teammates and hookless tools.
last_verified: '9164319'
confidence: draft
source: coopera/sessions/20260801-155417-6edfa020.md
---

## Decision

The shared wiki lives in `coopera/` — a visible, top-level committed directory. Hidden alternatives (`.wiki`, `.coopera/wiki`) were considered and rejected.

## Rationale

ripgrep (the engine behind Claude Code Grep, Codex file search, and the `rg` CLI) skips hidden directories by default. A dotdir wiki is therefore invisible to:

- Teammates in read-only mode (no binary installed) who rely on organic search
- Hookless tools like Antigravity that explore the codebase without injection
- Any agent that runs `rg` or the equivalent without `--hidden`

This directly undermines decision 006 (injection visibility is adoption prerequisite): knowledge that cannot be discovered fails its purpose.

## Live verification

`rg --files .wiki/` returns results only with `--hidden`; standard `rg` search finds nothing. Same behavior observed in Claude Code Grep during spike.

## The configurable escape hatch

Teams that find `coopera/` noisy can set `[wiki] dir = "docs/wiki"` in `.coopera/config.toml` to relocate it. The default must remain visible so the zero-config case works.

## Principle

The wiki is a human- and agent-readable output meant to be read alongside the code, not a tool-internal artifact. Hiding it trades discoverability for cosmetic tidiness — wrong tradeoff.
