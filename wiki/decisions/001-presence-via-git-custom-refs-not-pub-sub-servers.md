---
title: Presence via git custom refs, not pub/sub servers
type: decision
anchors:
- crates/coopera-core/src/presence.rs
- crates/coopera-core/src/inject.rs
- .coopera/config.toml
triggers:
- presence
- real-time
- session awareness
- synchronization
summary: Developer presence (who works on what, in which session, with what intent) lives in per-session git custom refs (refs/coopera/presence/<user>/<session>), fetched once at SessionStart. Avoids servers, merge conflicts, and real-time subscription issues.
last_verified: 5b26760
confidence: draft
source: wiki/sessions/20260729-003529-6edfa020.md
---

## Problem
Team members need to know what siblings are working on (prevent duplication, enable hand-off, synchronize context). True real-time (MCP subscriptions) doesn't reach models in practice; clients don't expose subscriptions to the LLM.

## Decision
Store presence in git custom refs by user and session:
```
refs/coopera/presence/<user>/<session_id> -> presence.json
```

Each developer force-pushes only their own refs (no merge conflict possible via namespace isolation). Fetch at SessionStart, cost ~1–2 seconds, ~137 tokens injected.

## Why This
- No external servers: git already trusted infrastructure, developers already trust push access
- Merge-conflict-free: per-user+per-session namespacing makes force-push semantically safe
- Offline-first: developer has full presence history locally
- Upgrade path: same schema, same API if later deployed to lightweight hub

## Tradeoff
Stale between sessions (can't alert mid-work to pivot). Acceptable: presence serves awareness, not live orchestration. Async teams pivot on branch/commit, not mid-session.
