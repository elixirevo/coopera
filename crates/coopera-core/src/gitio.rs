use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Thin wrapper over the system `git` binary. Decision 03: shell-out over
/// libraries — all needed operations are plumbing commands verified in spikes.
pub struct Git {
    pub root: PathBuf,
}

impl Git {
    /// Discover the repository containing `dir`. Errors if not inside a repo.
    pub fn discover(dir: &Path) -> Result<Git> {
        let out = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["rev-parse", "--show-toplevel"])
            .output()
            .context("failed to execute git — is git installed?")?;
        if !out.status.success() {
            bail!("not a git repository: {}", dir.display());
        }
        let root = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
        Ok(Git { root })
    }

    fn run(&self, args: &[&str]) -> Result<String> {
        let out = Command::new("git")
            .arg("-C")
            .arg(&self.root)
            .args(args)
            .output()
            .context("failed to execute git")?;
        if !out.status.success() {
            bail!(
                "git {:?} failed: {}",
                args,
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }

    /// Current branch name. Works before the first commit; falls back to "HEAD".
    pub fn current_branch(&self) -> String {
        self.run(&["symbolic-ref", "--short", "HEAD"])
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| "HEAD".to_string())
    }

    /// Changed files (staged, unstaged, and untracked) relative to repo root.
    pub fn changed_files(&self) -> Vec<String> {
        let Ok(out) = self.run(&["status", "--porcelain"]) else {
            return Vec::new();
        };
        out.lines()
            .filter_map(|l| {
                if l.len() < 4 {
                    return None;
                }
                // Format: "XY path" or "XY old -> new"
                let path = &l[3..];
                let path = path.split(" -> ").last().unwrap_or(path);
                Some(path.trim_matches('"').to_string())
            })
            .collect()
    }

    /// Stage the given repo-relative paths.
    pub fn add(&self, paths: &[String]) -> Result<()> {
        if paths.is_empty() {
            return Ok(());
        }
        let mut args: Vec<&str> = vec!["add", "--"];
        args.extend(paths.iter().map(|s| s.as_str()));
        self.run(&args)?;
        Ok(())
    }

    /// Short HEAD sha; None before the first commit.
    pub fn head_short(&self) -> Option<String> {
        self.run(&["rev-parse", "--short", "HEAD"])
            .ok()
            .map(|s| s.trim().to_string())
    }

    /// Configured git user name (digest attribution); "unknown" if unset.
    pub fn user_name(&self) -> String {
        self.run(&["config", "user.name"])
            .map(|s| s.trim().to_string())
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "unknown".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discover_fails_outside_repo() {
        let dir = tempfile::tempdir().unwrap();
        assert!(Git::discover(dir.path()).is_err());
    }

    #[test]
    fn discover_and_status_in_fresh_repo() {
        let dir = tempfile::tempdir().unwrap();
        Command::new("git")
            .arg("-C")
            .arg(dir.path())
            .arg("init")
            .arg("-q")
            .status()
            .unwrap();
        let git = Git::discover(dir.path()).unwrap();
        std::fs::write(dir.path().join("a.txt"), "x").unwrap();
        let changed = git.changed_files();
        assert!(
            changed.iter().any(|p| p == "a.txt"),
            "untracked file should be listed: {changed:?}"
        );
    }
}
