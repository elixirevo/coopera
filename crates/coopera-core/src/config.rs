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

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub budget: Budget,
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
    }
}
