---
title: Injection visibility is adoption prerequisite
type: decision
anchors:
- crates/coopera-cli/src/inject.rs
triggers:
- inject
- visibility
- transparency
- trust
summary: 'SessionStart always displays ''1 line: team context injected'' — invisible injection fails (Cursor auto-memory proof).'
last_verified: c45d2bb
confidence: draft
source: coopera/sessions/20260729-010048-c551ba6c.md
---

Cursor auto-Memories killed: invisible injection → users didn't know it happened → felt magical and untrustworthy → reverted to explicit AGENTS.md.

Solution: always show what was injected.

SessionStart output: `Team context: 2 active sessions · 1 new decision (payments: stripe) · 3 stale excluded`

In systemMessage: `Active: Alice (auth branch), Bob (payments). Recent: [payments/001] Stripe chosen. Concepts: try 'idempotency' or 'sourcing'.`

Benefit: users see injection happened (builds trust), can trace decisions back to injected premises (debuggability), can spot stale knowledge (feedback loop).
