---
title: Hook commands use PATH lookup, never absolute paths
type: decision
anchors:
- crates/coopera-cli/src/cmd_init.rs
triggers:
- hook
- COOPERA_BIN
- install
- binary
- portability
- machine-specific
summary: coopera init writes hook commands as ${COOPERA_BIN:-coopera} (PATH lookup with env override), never as absolute paths. This keeps committed config machine-independent, lets non-installers pass through silently, and lets local-build users override via shell profile.
last_verified: '9164319'
confidence: draft
source: coopera/sessions/20260801-155417-6edfa020.md
---

## Decision

`coopera init` writes hook commands in the form:

```sh
c="${COOPERA_BIN:-coopera}"; command -v "$c" >/dev/null 2>&1 && COOPERA_TOOL=claude-code "$c" hook session-start || echo {}
```

Absolute paths (e.g. `/Users/alice/.local/bin/coopera`) are **never** written to committed files.

## Rationale

- **Team portability**: hook config is committed and shared. An absolute path from the installer's machine causes hook errors for every other teammate on first session open.
- **Read-only mode**: the `command -v` guard means teammates without the binary receive an empty `{}` response; the session continues with zero friction.
- **Local build override**: developers building from source add `export COOPERA_BIN=/path/to/target/release/coopera` to their shell profile. The committed config is never touched.

## Regression guard

The e2e smoke test asserts that `coopera init` output contains no `/Users/`, `/home/`, or drive-letter absolute path patterns. This catches regressions immediately.

## Convention (mirrored in CLAUDE.md)

Committed settings must never contain machine-specific paths. Hook commands go via PATH. To use a local build, `export COOPERA_BIN=...` in your shell profile.
