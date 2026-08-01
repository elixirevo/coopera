---
title: Using coopera
type: playbook
anchors:
- coopera/INDEX.md
triggers:
- coopera
- wiki
summary: Add binary install instructions (Releases page, no Rust required), COOPERA_BIN override for local builds, read-only mode description for teammates without the binary, and rolling release cadence note.
confidence: draft
source: coopera/sessions/20260801-155417-6edfa020.md
---

coopera injects team context at session start and distills sessions into
digests when they end. You never need to run it manually. To add durable
team knowledge directly, create a page in the matching wiki/ directory and
open a PR. Run `coopera wiki lint` (or let CI do it) before merging.


### Session update
## Installing the binary (no Rust required)

Download from the rolling `latest` release — rebuilt on every push to main:

```bash
# macOS arm64
curl -fsSL "https://github.com/elixirevo/coopera/releases/download/latest/coopera-aarch64-apple-darwin.tar.gz" | tar -xz -C ~/.local/bin

# macOS x86_64
curl -fsSL "https://github.com/elixirevo/coopera/releases/download/latest/coopera-x86_64-apple-darwin.tar.gz" | tar -xz -C ~/.local/bin

# Linux x86_64
curl -fsSL "https://github.com/elixirevo/coopera/releases/download/latest/coopera-x86_64-unknown-linux-gnu.tar.gz" | tar -xz -C ~/.local/bin
```

Windows: download `coopera-x86_64-pc-windows-msvc.zip` from the Releases page and place `coopera.exe` on your PATH.

## Using a local build

Set `COOPERA_BIN` in your shell profile to override the PATH lookup without touching committed config:

```bash
export COOPERA_BIN="$HOME/dev/coopera/target/release/coopera"
```

## Read-only mode (no binary installed)

Teammates who clone but do not install the binary enter **read-only mode** automatically: hooks pass through silently (exit 0), wiki pages are readable, and their commits/pushes appear in the activity map. They do not get session injection, presence publishing, or auto-distillation until they install the binary and approve hooks once.
