//! F1 — `coopera init`: install the harness into the current repository.
//! Idempotent: re-running must change nothing that is already installed.

use anyhow::{Context, Result};
use coopera_core::gitio::Git;
use std::path::Path;

const MARKER_BEGIN: &str = "<!-- coopera:begin -->";
const MARKER_END: &str = "<!-- coopera:end -->";

const AGENT_SECTION: &str = "\
## Team context (coopera)\n\
This repository uses coopera, a harness that shares team context between AI coding sessions.\n\
- Shared team knowledge lives in `wiki/` (concepts, modules, decisions, playbooks). Read `wiki/INDEX.md` first; do not bulk-read the whole wiki directory.\n\
- Before planning or making design decisions, consult the injected team context and relevant wiki pages. Avoid conflicting with or duplicating in-flight teammate work; align with recorded team decisions.\n\
- Session digests are written to `wiki/sessions/` automatically; wiki changes ride along with your code PR for human review.\n\
- If your tool does not run coopera hooks (e.g. Antigravity), start each task by reading `.coopera/cache/presence.md` (teammate activity map) and `wiki/INDEX.md`.\n";

const CONFIG_TEMPLATE: &str = "\
# coopera configuration (defaults shown; all values optional)\n\
[budget]\n\
# Hard token caps for the injection pack (00-harness §2.4)\n\
total = 1500\n\
presence = 300\n\
wiki = 800\n\
instructions = 200\n";

const INDEX_TEMPLATE: &str = "\
# Wiki index\n\n\
Shared team knowledge, maintained by coopera. Pages are promoted via PR review.\n\n\
| Type | Purpose |\n\
|---|---|\n\
| concepts/ | Team vocabulary (ubiquitous language) |\n\
| modules/ | Module guides: responsibilities, invariants, gotchas |\n\
| decisions/ | Lightweight ADRs: what was chosen and why |\n\
| playbooks/ | How-to procedures for recurring tasks |\n\n\
Session digests (raw feed, not injected): sessions/\n";

const SAMPLE_PAGE: &str = "\
---\n\
title: Using coopera\n\
type: playbook\n\
anchors: [\"wiki/\"]\n\
triggers: [coopera, wiki]\n\
summary: This repo shares team context via coopera — wiki pages are promoted by PR review; session digests are automatic.\n\
confidence: high\n\
---\n\n\
coopera injects team context at session start and distills sessions into\n\
digests when they end. You never need to run it manually. To add durable\n\
team knowledge directly, create a page in the matching wiki/ directory and\n\
open a PR. Run `coopera wiki lint` (or let CI do it) before merging.\n";

pub fn run() -> i32 {
    match install() {
        Ok(summary) => {
            println!("{summary}");
            0
        }
        Err(e) => {
            eprintln!("coopera init failed: {e:#}");
            1
        }
    }
}

fn install() -> Result<String> {
    let cwd = std::env::current_dir().context("cannot determine current directory")?;
    let git =
        Git::discover(&cwd).context("coopera requires a git repository — run 'git init' first")?;
    let root = git.root.clone();
    let mut actions: Vec<String> = Vec::new();

    // 1) wiki/ scaffold
    for sub in ["concepts", "modules", "decisions", "playbooks", "sessions"] {
        let dir = root.join("wiki").join(sub);
        if !dir.exists() {
            std::fs::create_dir_all(&dir)?;
            actions.push(format!("created wiki/{sub}/"));
        }
    }
    write_if_absent(&root.join("wiki/INDEX.md"), INDEX_TEMPLATE, &mut actions)?;
    write_if_absent(
        &root.join("wiki/playbooks/using-coopera.md"),
        SAMPLE_PAGE,
        &mut actions,
    )?;

    // 2) .coopera/ config + cache ignore
    std::fs::create_dir_all(root.join(".coopera/cache"))?;
    write_if_absent(
        &root.join(".coopera/config.toml"),
        CONFIG_TEMPLATE,
        &mut actions,
    )?;
    ensure_gitignore_line(&root, ".coopera/cache/", &mut actions)?;

    // 3) Hooks: Claude Code (.claude/settings.json) + Codex (.codex/config.toml)
    install_claude_hooks(&root, &mut actions)?;
    install_codex_hooks(&root, &mut actions)?;

    // 4) Agent instruction files: managed marker block in CLAUDE.md / AGENTS.md
    for file in ["CLAUDE.md", "AGENTS.md"] {
        upsert_marker_block(&root.join(file), &mut actions)?;
    }

    if actions.is_empty() {
        Ok("coopera: already installed (nothing to do)".to_string())
    } else {
        Ok(format!(
            "coopera: installed\n  - {}",
            actions.join("\n  - ")
        ))
    }
}

fn write_if_absent(path: &Path, content: &str, actions: &mut Vec<String>) -> Result<()> {
    if !path.exists() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, content)?;
        actions.push(format!(
            "created {}",
            path.file_name().unwrap_or_default().to_string_lossy()
        ));
    }
    Ok(())
}

fn ensure_gitignore_line(root: &Path, line: &str, actions: &mut Vec<String>) -> Result<()> {
    let path = root.join(".gitignore");
    let current = std::fs::read_to_string(&path).unwrap_or_default();
    if !current.lines().any(|l| l.trim() == line) {
        let mut next = current;
        if !next.is_empty() && !next.ends_with('\n') {
            next.push('\n');
        }
        next.push_str(line);
        next.push('\n');
        std::fs::write(&path, next)?;
        actions.push(format!("added {line} to .gitignore"));
    }
    Ok(())
}

/// Hook command with graceful degradation: try the installer's absolute
/// binary path, fall back to `coopera` on PATH, and if neither exists emit a
/// valid empty hook reply. Teammates who cloned but did not install coopera
/// get read-only mode (AGENTS.md-compiled guidance) with zero error noise —
/// the partial-adoption contract (01-idea-brief).
fn guarded_command(exe: &str, tool: &str, sub: &str) -> String {
    format!(
        "c=\"{exe}\"; [ -x \"$c\" ] || c=\"$(command -v coopera)\"; [ -x \"$c\" ] && COOPERA_TOOL={tool} \"$c\" hook {sub} || echo {{}}"
    )
}

/// Merge coopera hooks into .claude/settings.json, preserving everything
/// else. Our entries are rebuilt each run (upgrades replace old formats);
/// the file is only written when the parsed value actually changes.
fn install_claude_hooks(root: &Path, actions: &mut Vec<String>) -> Result<()> {
    let exe = std::env::current_exe().context("cannot resolve coopera binary path")?;
    let exe = exe.to_string_lossy();
    let path = root.join(".claude/settings.json");
    let original: serde_json::Value = match std::fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).context(".claude/settings.json is not valid JSON")?,
        Err(_) => serde_json::json!({}),
    };
    let mut settings = original.clone();

    for (event, sub) in [
        ("SessionStart", "session-start"),
        ("UserPromptSubmit", "user-prompt-submit"),
        ("SessionEnd", "session-end"),
    ] {
        let command = guarded_command(&exe, "claude-code", sub);
        let hooks = settings
            .as_object_mut()
            .context("settings.json root must be an object")?
            .entry("hooks")
            .or_insert_with(|| serde_json::json!({}));
        let entries = hooks
            .as_object_mut()
            .context("settings.json 'hooks' must be an object")?
            .entry(event)
            .or_insert_with(|| serde_json::json!([]));
        let list = entries
            .as_array_mut()
            .context("hook event entry must be an array")?;
        list.retain(|e| !e.to_string().contains("coopera"));
        list.push(serde_json::json!({
            "hooks": [{ "type": "command", "command": command }]
        }));
    }
    if settings != original {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, serde_json::to_string_pretty(&settings)?)?;
        actions.push("registered Claude Code hooks (.claude/settings.json)".to_string());
    }
    Ok(())
}

const CODEX_MARKER: &str = "# --- coopera hooks (managed) ---";

/// F8 — register Codex hooks (verified schema, spike ③: project
/// `.codex/config.toml` with `[[hooks.EventName]]` tables; PascalCase events,
/// command handlers only). The managed block (marker to EOF) is regenerated
/// each run so upgrades propagate. TOML literal strings carry the guarded
/// command. Note: the project must be trusted in Codex and the hook
/// definitions approved once via `/hooks`.
fn install_codex_hooks(root: &Path, actions: &mut Vec<String>) -> Result<()> {
    let exe = std::env::current_exe().context("cannot resolve coopera binary path")?;
    let exe = exe.to_string_lossy();
    let path = root.join(".codex/config.toml");
    let current = std::fs::read_to_string(&path).unwrap_or_default();
    let prefix = match current.find(CODEX_MARKER) {
        Some(i) => current[..i].trim_end().to_string(),
        None => current.trim_end().to_string(),
    };

    let mut block =
        format!("{CODEX_MARKER}\n# Keep this block last in the file; init regenerates it.\n");
    for (event, sub) in [
        ("SessionStart", "session-start"),
        ("UserPromptSubmit", "user-prompt-submit"),
        ("SessionEnd", "session-end"),
    ] {
        let command = guarded_command(&exe, "codex", sub);
        block.push_str(&format!(
            "[[hooks.{event}]]\n[[hooks.{event}.hooks]]\ntype = \"command\"\ncommand = '{command}'\n\n"
        ));
    }
    let next = if prefix.is_empty() {
        block.trim_end().to_string() + "\n"
    } else {
        format!("{prefix}\n\n{}\n", block.trim_end())
    };
    if next != current {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, next)?;
        actions.push("registered Codex hooks (.codex/config.toml)".to_string());
    }
    Ok(())
}

/// Insert or refresh the coopera-managed block in an agent instruction file.
fn upsert_marker_block(path: &Path, actions: &mut Vec<String>) -> Result<()> {
    let block = format!("{MARKER_BEGIN}\n{AGENT_SECTION}{MARKER_END}\n");
    let current = std::fs::read_to_string(path).unwrap_or_default();
    let next =
        if let (Some(start), Some(end)) = (current.find(MARKER_BEGIN), current.find(MARKER_END)) {
            let mut s = String::new();
            s.push_str(&current[..start]);
            s.push_str(&block);
            s.push_str(current[end + MARKER_END.len()..].trim_start_matches('\n'));
            s
        } else {
            let mut s = current.clone();
            if !s.is_empty() && !s.ends_with('\n') {
                s.push('\n');
            }
            if !s.is_empty() {
                s.push('\n');
            }
            s.push_str(&block);
            s
        };
    if next != current {
        std::fs::write(path, next)?;
        actions.push(format!(
            "updated coopera section in {}",
            path.file_name().unwrap_or_default().to_string_lossy()
        ));
    }
    Ok(())
}

pub fn status() -> i32 {
    let cwd = match std::env::current_dir() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("coopera: {e}");
            return 1;
        }
    };
    match Git::discover(&cwd) {
        Ok(git) => {
            let (pages, errors) = coopera_core::wiki::load_wiki(&git.root);
            let installed = git.root.join(".coopera/config.toml").exists();
            println!(
                "coopera status: installed={installed} wiki_pages={} parse_errors={} branch={}",
                pages.len(),
                errors.len(),
                git.current_branch()
            );
            0
        }
        Err(_) => {
            println!("coopera status: not a git repository");
            1
        }
    }
}
