---
title: Distillation by each agent, not centralized
type: decision
anchors:
- crates/coopera-cli/src/distill.rs
triggers:
- distill
- session end
- summarize
summary: Each tool distills its own sessions (claude -p, codex exec, Antigravity fallback) — no API key, cost on user's subscription.
last_verified: c45d2bb
confidence: draft
source: wiki/sessions/20260729-010048-c551ba6c.md
---

Centralized distillation requires shared API key (cost ambiguity) or hosted server (violates zero-servers). Instead:

- **Claude Code**: SessionEnd handler spawns distiller agent → JSON schema result
- **Codex**: SessionEnd command → `codex exec distill` subprocess → Bash CLI invocation
- **Antigravity**: No SessionEnd hook — SessionStart detects previous undistilled session, triggers backfill

Each user's model choice is local (Opus/Sonnet/Haiku). Quality validated via golden-set regression before deployment.
