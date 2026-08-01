use serde::Deserialize;
use std::path::Path;

/// Token budget for the injection pack. Hard caps, not targets.
/// Defaults follow 00-harness §2.4 (total ~1.5k tokens).
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Budget {
    pub total: usize,
    pub presence: usize,
    pub wiki: usize,
    pub instructions: usize,
}

impl Default for Budget {
    fn default() -> Self {
        Self {
            total: 1500,
            presence: 300,
            wiki: 800,
            instructions: 200,
        }
    }
}

/// One headless agent invocation (command + args, prompt on stdin).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct AgentCmd {
    pub command: String,
    pub args: Vec<String>,
}

/// Distillation settings (F3). The command is the user's own agent run
/// headlessly — coopera never carries its own API key (implementation
/// decision, 01-idea-brief). Decision 003: each tool distills its own
/// sessions, so resolution is per hosting tool with availability fallback.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Distill {
    /// Global agent override for every tool; receives the prompt on stdin.
    /// Unset (the default) means "use the hosting tool's own agent".
    pub command: Option<String>,
    /// Arguments for the global override (e.g. ["-p"] for `claude -p`).
    pub args: Option<Vec<String>>,
    pub timeout_secs: u64,
    /// Sessions with fewer conversation messages than this are skipped
    /// (substance threshold — empty sessions produce no digest).
    pub min_messages: usize,
    /// Per-tool overrides, keyed by COOPERA_TOOL name (`claude-code`,
    /// `codex`, ...). Beats the global override for that tool.
    pub agents: std::collections::BTreeMap<String, AgentCmd>,
}

impl Default for Distill {
    fn default() -> Self {
        Self {
            command: None,
            args: None,
            // Real distillations of large sessions measured ~102s on
            // 2026-08-02; 180s left too little headroom for model/API
            // variance and killed runs that would have succeeded.
            timeout_secs: 600,
            min_messages: 3,
            agents: std::collections::BTreeMap::new(),
        }
    }
}

/// In agent args, this placeholder is replaced with a fresh temp-file path
/// and the agent's reply is read from that file instead of stdout — for CLIs
/// whose stdout is a noisy event stream (codex exec).
pub const REPLY_FILE_PLACEHOLDER: &str = "COOPERA_OUT";

/// In agent args, this placeholder is replaced with the full prompt text and
/// stdin stays closed — for CLIs whose print mode only accepts the prompt as
/// an argument (agy takes `-p <prompt>` and ignores piped stdin).
pub const PROMPT_PLACEHOLDER: &str = "COOPERA_PROMPT";

/// Built-in per-tool agent defaults, all verified against real CLIs on
/// 2026-08-02: codex-cli 0.146.0 (stdin prompt via `-`, final message via
/// `-o`, `--ephemeral` keeps distiller runs out of the session store —
/// offline recursion guard; read-only sandbox, low effort per PRD F8) and
/// agy (Antigravity; prompt must be the `-p` argument — no stdin — and the
/// fixed-schema prompt holds even through user-configured personas).
fn builtin_agent(tool: &str) -> Option<AgentCmd> {
    match tool {
        "claude-code" => Some(AgentCmd {
            command: "claude".to_string(),
            args: vec!["-p".to_string()],
        }),
        "codex" => Some(AgentCmd {
            command: "codex".to_string(),
            args: [
                "exec",
                "--ephemeral",
                "-s",
                "read-only",
                "-c",
                "model_reasoning_effort=\"low\"",
                "-o",
                REPLY_FILE_PLACEHOLDER,
                "-",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        }),
        "antigravity" => Some(AgentCmd {
            command: "agy".to_string(),
            args: [
                "--effort",
                "low",
                "--print-timeout",
                "600s",
                "-p",
                PROMPT_PLACEHOLDER,
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        }),
        _ => None,
    }
}

/// Fallback order when a tool's own CLI is missing: any installed agent
/// beats an undistilled session.
const KNOWN_TOOLS: [&str; 3] = ["claude-code", "codex", "antigravity"];

impl Config {
    /// Pick the distiller agent for a session hosted by `tool`.
    /// Precedence: per-tool config override → global config override →
    /// the hosting tool's own CLI (if on PATH) → any other known agent CLI
    /// on PATH (a codex-only teammate still distills claude-tagged backlog,
    /// and vice versa) → None (caller queues the transcript).
    /// Explicit config values are trusted verbatim — only built-in defaults
    /// are availability-checked via `on_path`.
    pub fn resolve_agent(
        &self,
        tool: &str,
        on_path: &dyn Fn(&str) -> bool,
    ) -> Option<(String, Vec<String>)> {
        let d = &self.distill;
        if let Some(a) = d.agents.get(tool) {
            if !a.command.trim().is_empty() {
                return Some((a.command.clone(), a.args.clone()));
            }
        }
        if let Some(cmd) = &d.command {
            return Some((cmd.clone(), d.args.clone().unwrap_or_default()));
        }
        if let Some(a) = builtin_agent(tool) {
            if on_path(&a.command) {
                return Some((a.command, a.args));
            }
        }
        for other in KNOWN_TOOLS {
            if other == tool {
                continue;
            }
            if let Some(a) = builtin_agent(other) {
                if on_path(&a.command) {
                    return Some((a.command, a.args));
                }
            }
        }
        None
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub budget: Budget,
    pub distill: Distill,
}

impl Config {
    /// Load `.coopera/config.toml` from the repo root. Missing or invalid
    /// config falls back to defaults — configuration must never block a session.
    pub fn load(repo_root: &Path) -> Config {
        let path = repo_root.join(".coopera").join("config.toml");
        match std::fs::read_to_string(&path) {
            Ok(s) => toml::from_str(&s).unwrap_or_default(),
            Err(_) => Config::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_budget_matches_design() {
        let b = Budget::default();
        assert_eq!(b.total, 1500);
        assert_eq!(b.presence + b.wiki + b.instructions, 1300); // 200 slack
    }

    #[test]
    fn missing_config_falls_back_to_default() {
        let dir = tempfile::tempdir().unwrap();
        let c = Config::load(dir.path());
        assert_eq!(c.budget.total, 1500);
        assert_eq!(c.distill.timeout_secs, 600);
        assert!(c.distill.command.is_none(), "no global override by default");
    }

    #[test]
    fn resolve_prefers_the_hosting_tools_own_agent() {
        let c = Config::default();
        let all = |_: &str| true;
        let (cmd, args) = c.resolve_agent("claude-code", &all).unwrap();
        assert_eq!(cmd, "claude");
        assert_eq!(args, vec!["-p"]);
        let (cmd, args) = c.resolve_agent("codex", &all).unwrap();
        assert_eq!(cmd, "codex");
        assert_eq!(args[0], "exec");
        assert!(args.contains(&REPLY_FILE_PLACEHOLDER.to_string()));
        assert!(args.contains(&"--ephemeral".to_string()));
        let (cmd, args) = c.resolve_agent("antigravity", &all).unwrap();
        assert_eq!(cmd, "agy");
        assert!(
            args.contains(&PROMPT_PLACEHOLDER.to_string()),
            "agy takes the prompt as an argument: {args:?}"
        );
    }

    #[test]
    fn resolve_falls_back_to_whatever_agent_is_installed() {
        let c = Config::default();
        // Codex-only machine: claude-code sessions still distill via codex.
        let codex_only = |name: &str| name == "codex";
        let (cmd, _) = c.resolve_agent("claude-code", &codex_only).unwrap();
        assert_eq!(cmd, "codex");
        // Hook-less tool with no session-native CLI: first available wins.
        let (cmd, _) = c.resolve_agent("antigravity", &codex_only).unwrap();
        assert_eq!(cmd, "codex");
        // Nothing installed, nothing configured: caller must queue.
        assert!(c.resolve_agent("claude-code", &|_| false).is_none());
    }

    #[test]
    fn resolve_respects_config_overrides_verbatim() {
        let toml = r#"
[distill]
command = "/tmp/global-stub"
args = []
[distill.agents.codex]
command = "/tmp/codex-stub"
args = ["--x"]
"#;
        let c: Config = toml::from_str(toml).unwrap();
        let none = |_: &str| false; // overrides must NOT be availability-checked
        let (cmd, args) = c.resolve_agent("codex", &none).unwrap();
        assert_eq!(cmd, "/tmp/codex-stub");
        assert_eq!(args, vec!["--x"]);
        let (cmd, args) = c.resolve_agent("claude-code", &none).unwrap();
        assert_eq!(cmd, "/tmp/global-stub");
        assert!(args.is_empty());
    }
}
