---
title: 'Coopera: understanding integration, not task handoff'
type: concept
anchors:
- docs/project/01-idea-brief.md
- docs/project/00-harness.md
- CLAUDE.md
triggers:
- cooperation
- context
- handoff
- alignment
- understanding
summary: Coopera enables all team LLMs to develop under one shared codebase model (same concepts, decisions, gotchas), eliminating meaning collisions. Not a succession tool (A → B) but a synchronization tool (A and B know each other's world).
last_verified: 5b26760
confidence: draft
source: coopera/sessions/20260729-003529-6edfa020.md
---

## Not Handoff
- Not continuation (A's session state → B's session)
- Not task queues (step 1, then step 2, then step 3)
- Not work-in-progress notification

## What It Is
5 developers, 5 AI agents, 1 mental model of the codebase. When A decides "Redis for cache due to latency SLA", B's agent knows this and plans accordingly (not duplicating, not conflicting). When A learns "payloads >1MB fail", B sees it before writing similar code.

## How
1. **Shared wiki** = team's single source of truth (concepts, decisions, module guides), updated by PR review, not auto-published
2. **Presence map** = who is on which branch, what is their intent, updated at SessionStart
3. **Injection at SessionStart** = all this context goes into model's reasoning window
4. **No surveillance** = presence is for coordination, not performance tracking (opt-out guaranteed, no time accounting)

## Why It Matters
Without: 5-person team develops 5 slightly different models of the code. Each LLM rediscovers, redebates, creates overlapping/conflicting implementations. With: 1 codebase, 1 shared understanding, 5 agents whose outputs harmonize.

## Success Metrics
Not "faster handoffs" or "reduced communication", but:
- Fewer semantic collisions (teams planning at cross-purposes)
- Fewer duplicate implementations (same feature coded twice)
- Fewer rule drifts (code style, patterns, conventions staying consistent)
- Higher confidence in team PRs (reviewers see why choices were made, not just what)
