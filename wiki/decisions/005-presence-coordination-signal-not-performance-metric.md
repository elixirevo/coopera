---
title: 'Presence: coordination signal, not performance metric'
type: decision
anchors:
- crates/coopera-core/src/presence.rs
triggers:
- presence
- surveillance
- privacy
- coordination
summary: Presence shows who/what-branch/intent for coordination only. No time tracking, metrics, or performance inference — opt-out guaranteed.
last_verified: c45d2bb
confidence: draft
source: wiki/sessions/20260729-010048-c551ba6c.md
---

Presence enables: "I see you're on auth branch — can I touch session-token code?"

Presence does NOT contain: token counts, model inference time, tool call frequency, code diff size, activity timeline, behavioral patterns.

Why: once presence leaks to metrics, developers hide data (don't update if idle, lie about intent, opt-out entirely) → system dies.

**Anti-surveillance guarantee**: opt-out built-in (`coopera status --hide`), presence file is plain YAML/JSON (auditable), never used for comparisons/dashboards/reporting. Presence is conversation-starting, not performance fuel.
