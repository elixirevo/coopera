---
title: Wiki changes stage with code PR, not separate
type: decision
anchors:
- crates/coopera-cli/src/wiki.rs
triggers:
- wiki
- PR
- distill
- knowledge
summary: Distilled wiki diffs stage in the working branch, ship with code PR — single review, unified visibility, knowledge promotion is atomic.
last_verified: c45d2bb
confidence: draft
source: coopera/sessions/20260729-010048-c551ba6c.md
---

Problem: separate wiki PRs per session accumulate → review fatigue → wiki abandoned.

Solution:
1. SessionEnd distiller generates wiki diffs (new/updated `.md` files)
2. Diffs stage in working branch: `git add coopera/decisions/003-*.md`
3. Developer commits code + wiki together
4. Push: code + wiki in single PR
5. Review: code reviewer sees both, knowledge lives on same commit as code
6. Merge: last_verified updates, knowledge promotion atomic with code

Rare case (no code changes): minimal init commit provides staging ground for wiki diff.
