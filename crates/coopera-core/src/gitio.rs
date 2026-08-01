use anyhow::{bail, Context, Result};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

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

    /// Sha of the last commit that touched `rel_path`; None if never committed.
    pub fn last_commit_of(&self, rel_path: &str) -> Option<String> {
        self.run(&["log", "-1", "--format=%H", "--", rel_path])
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    /// Files changed in commits after `base` up to HEAD (committed changes
    /// only — uncommitted work is in-flight context, not staleness).
    pub fn changed_between(&self, base: &str) -> Result<Vec<String>> {
        Ok(self
            .run(&["diff", "--name-only", base, "HEAD"])?
            .lines()
            .map(str::to_string)
            .filter(|l| !l.is_empty())
            .collect())
    }

    fn run_with_stdin(&self, args: &[&str], input: &str) -> Result<String> {
        let mut child = Command::new("git")
            .arg("-C")
            .arg(&self.root)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("failed to execute git")?;
        child
            .stdin
            .take()
            .context("no stdin")?
            .write_all(input.as_bytes())?;
        let out = child.wait_with_output()?;
        if !out.status.success() {
            bail!(
                "git {:?} failed: {}",
                args,
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }

    /// Run a network-touching git command with a hard timeout — SessionStart
    /// has a latency budget, and a hanging remote must never block a session.
    pub fn run_network(&self, args: &[&str], timeout: Duration) -> Result<()> {
        let mut child = Command::new("git")
            .arg("-C")
            .arg(&self.root)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("failed to execute git")?;
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(status) = child.try_wait()? {
                if status.success() {
                    return Ok(());
                }
                bail!("git {args:?} failed");
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                bail!("git {args:?} timed out after {timeout:?}");
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    /// Fire-and-forget network command (background push) — never blocks, and
    /// detaches so hook-teardown group kills cannot reap the push mid-flight.
    pub fn spawn_network(&self, args: &[&str]) {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.root)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let _ = crate::spawn::detach(&mut cmd).spawn();
    }

    /// True if the repo has a remote named origin.
    pub fn has_origin(&self) -> bool {
        self.run(&["remote", "get-url", "origin"]).is_ok()
    }

    /// Point `refname` at a single-file tree containing `content` (plumbing
    /// path verified in spike ①: hash-object → mktree → commit-tree).
    pub fn update_ref_with_file(
        &self,
        refname: &str,
        file_name: &str,
        content: &str,
        message: &str,
    ) -> Result<()> {
        let blob = self
            .run_with_stdin(&["hash-object", "-w", "--stdin"], content)?
            .trim()
            .to_string();
        let tree = self
            .run_with_stdin(&["mktree"], &format!("100644 blob {blob}\t{file_name}\n"))?
            .trim()
            .to_string();
        // Plumbing commits carry no meaningful authorship (the file content
        // names the user) — pin a fixed identity so publishing works on
        // machines/CI without git identity configured.
        let commit = self
            .run(&[
                "-c",
                "user.name=coopera",
                "-c",
                "user.email=presence@coopera.local",
                "commit-tree",
                &tree,
                "-m",
                message,
            ])?
            .trim()
            .to_string();
        self.run(&["update-ref", refname, &commit])?;
        Ok(())
    }

    /// Read `file_name` from the tree a ref points at.
    pub fn read_ref_file(&self, refname: &str, file_name: &str) -> Result<String> {
        self.run(&["cat-file", "-p", &format!("{refname}:{file_name}")])
    }

    /// List refs matching `pattern` (full ref names).
    pub fn list_refs(&self, pattern: &str) -> Vec<String> {
        self.run(&["for-each-ref", "--format=%(refname)", pattern])
            .map(|out| out.lines().map(str::to_string).collect())
            .unwrap_or_default()
    }

    /// Delete a local ref (ignores absence).
    pub fn delete_ref(&self, refname: &str) {
        let _ = self.run(&["update-ref", "-d", refname]);
    }

    /// The default branch's remote-tracking name (e.g. "origin/main").
    pub fn default_remote_branch(&self) -> String {
        self.run(&["symbolic-ref", "--short", "refs/remotes/origin/HEAD"])
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| "origin/main".to_string())
    }

    /// Recently active remote branches: (short name, unix committer date,
    /// last commit subject), newest first, default branch and HEAD excluded.
    pub fn recent_remote_branches(&self, max: usize) -> Vec<(String, i64, String)> {
        let default = self.default_remote_branch();
        let Ok(out) = self.run(&[
            "for-each-ref",
            "--sort=-committerdate",
            "--format=%(refname:short)\t%(committerdate:unix)\t%(contents:subject)",
            "refs/remotes/origin",
        ]) else {
            return Vec::new();
        };
        out.lines()
            .filter_map(|l| {
                let mut parts = l.splitn(3, '\t');
                let name = parts.next()?.to_string();
                let date: i64 = parts.next()?.parse().ok()?;
                let subject = parts.next().unwrap_or("").to_string();
                if name == default || name.ends_with("/HEAD") || name == "origin" {
                    return None;
                }
                Some((name, date, subject))
            })
            .take(max)
            .collect()
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
