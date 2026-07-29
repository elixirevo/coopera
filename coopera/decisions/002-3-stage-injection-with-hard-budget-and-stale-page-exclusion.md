---
title: 3-stage injection with hard budget and stale page exclusion
type: decision
anchors:
- crates/coopera-core/src/inject.rs
- docs/project/03-prd.md
triggers:
- injection
- context pollution
- stale
- token budget
- wiki
summary: Wiki injection happens in 3 stages (L0 always, L1 on match, L2 on pull) capped at ~1.5k tokens; pages with stale last_verified are excluded from main injection, marked for re-verification.
last_verified: 5b26760
confidence: draft
source: wiki/sessions/20260729-003529-6edfa020.md
---

## Problem
Large context injections pollute model workspace. Stale/obsolete team knowledge damages trust more than having no shared knowledge. Need to balance "rich context" against "irrelevant noise."

## Decision
Progressive disclosure, hard budget:
- **L0 (always)**: wiki INDEX (1-line overview) + presence map only
- **L1 (on trigger/anchor match)**: matched page summary (1–3 lines) only  
- **L2 (on explicit pull)**: full page body (agent requests via wiki tool)

Pages with stale `last_verified` (anchors file changed after that commit) are excluded from L0+L1; injected as 1-line pointer ("see X, re-verify before relying").

## Why
- Prevents context rot: aged decisions don't dilute model attention
- Automatic staleness detection: CI checks that anchors still exist, humans just review
- Semantic indexing: page frontmatter `triggers` act as keywords ("panic" auto-injects panic-handling page)
- Respects model bandwidth: ~1.5k tokens = ~4–5 pages compressed, enough signal without noise

## Evidence
Tests showed models strongly respond to injected team context: with context, even small models avoid conflict-creating plans; without it, they re-implement known decisions. High-quality selective injection beats high-volume broadcast.
