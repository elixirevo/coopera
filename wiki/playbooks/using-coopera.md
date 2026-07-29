---
title: Using coopera
type: playbook
anchors: ["wiki/"]
triggers: [coopera, wiki]
summary: This repo shares team context via coopera — wiki pages are promoted by PR review; session digests are automatic.
confidence: high
---

coopera injects team context at session start and distills sessions into
digests when they end. You never need to run it manually. To add durable
team knowledge directly, create a page in the matching wiki/ directory and
open a PR. Run `coopera wiki lint` (or let CI do it) before merging.
