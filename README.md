# coopera

A team-context harness for AI coding tools (Claude Code, Codex, Antigravity).

coopera captures each developer's session context automatically and integrates it —
an LLM wiki in git plus push-based work tracking — so every teammate's LLM works
from the same shared understanding of the project. No server: git is the only
data source. It is installed into your existing tools via hooks; you never run
it manually.

Status: pre-release, M1 in progress (internal dogfooding).

## Install (no Rust required)

Prebuilt binaries: `coopera-aarch64-apple-darwin` (Apple Silicon),
`coopera-x86_64-apple-darwin` (Intel Mac), `coopera-x86_64-unknown-linux-gnu`,
`coopera-x86_64-pc-windows-msvc.zip` (Windows).

Rolling build (updated on every push to main):

```bash
curl -fsSL -o coopera.tar.gz "https://github.com/elixirevo/coopera/releases/download/latest/coopera-$(uname -m | sed s/arm64/aarch64/)-apple-darwin.tar.gz"
tar -xzf coopera.tar.gz && mv coopera ~/.local/bin/  # or anywhere on PATH
```

Windows (PowerShell; hooks assume Git Bash, which Claude Code on Windows uses):

```powershell
iwr -useb "https://github.com/elixirevo/coopera/releases/download/latest/coopera-x86_64-pc-windows-msvc.zip" -OutFile coopera.zip
Expand-Archive coopera.zip -DestinationPath "$env:USERPROFILE\bin"   # add to PATH
```

Windows status: compiled and unit/e2e-tested in CI on every push; live hook
integration on a real Windows machine is still to be verified.

Pinned stable build: replace `download/latest` with `download/v0.1.0`
(or use `releases/latest/download/...`, which resolves to the newest version tag).

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
