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

/// Distillation settings (F3). The command is the user's own agent run
/// headlessly — coopera never carries its own API key (implementation
/// decision, 01-idea-brief).
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Distill {
    /// Agent command; receives the prompt on stdin.
    pub command: String,
    /// Arguments passed to the command (e.g. ["-p"] for `claude -p`).
    pub args: Vec<String>,
    pub timeout_secs: u64,
    /// Sessions with fewer conversation messages than this are skipped
    /// (substance threshold — empty sessions produce no digest).
    pub min_messages: usize,
}

impl Default for Distill {
    fn default() -> Self {
        Self {
            command: "claude".to_string(),
            args: vec!["-p".to_string()],
            // Real distillations of large sessions measured ~102s on
            // 2026-08-02; 180s left too little headroom for model/API
            // variance and killed runs that would have succeeded.
            timeout_secs: 600,
            min_messages: 3,
        }
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
    }
}
