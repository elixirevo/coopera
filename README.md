# coopera

A team-context harness for AI coding tools (Claude Code, Codex, Antigravity).

coopera captures each developer's session context automatically and integrates it —
an LLM wiki in git plus push-based work tracking — so every teammate's LLM works
from the same shared understanding of the project. No server: git is the only
data source. It is installed into your existing tools via hooks; you never run
it manually.

Status: pre-release, M1 in progress (internal dogfooding).

## Install (no Rust required)

Download the binary for your platform from GitHub Releases and put it on PATH:

```bash
gh release download -R elixirevo/coopera -p "coopera-$(uname -m | sed s/arm64/aarch64/)-apple-darwin.tar.gz"
tar -xzf coopera-*.tar.gz && mv coopera ~/.local/bin/  # or anywhere on PATH
```

Hooks committed in a coopera-enabled repo find `coopera` on PATH automatically;
without it you still get read-only mode (wiki guidance + push tracking).

## Build & test (from source)

```bash
cargo build
cargo test
```

Install into a repository (normally done once per repo):

```bash
cargo run -p coopera-cli -- init
```

Design docs live in `docs/project/` (Korean).
