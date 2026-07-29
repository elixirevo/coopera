//! Stale-page detection: a page is stale when files matching its anchors
//! changed in commits made after the page was last verified.
//!
//! Baseline choice (M1): the page's own last commit takes precedence over the
//! frontmatter `last_verified` sha. Wiki diffs ride along with code PRs, so
//! merging/editing a page IS its re-verification — and this avoids the trap
//! where a freshly distilled page (verified at the previous HEAD, shipped in
//! the same commit as the code it describes) would count as instantly stale.
//! `last_verified` remains as human-readable provenance and as the fallback
//! for pages that are not yet committed.

use crate::gitio::Git;
use crate::wiki::{anchor_prefix, Page};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

/// Paths of pages whose anchored code has changed since their baseline.
/// Fail-open: any git error (unknown sha, shallow clone) counts as fresh —
/// wrongly excluding knowledge is worse than occasionally injecting old news
/// with its [unreviewed]/pointer markers.
pub fn find_stale(git: &Git, pages: &[Page]) -> HashSet<PathBuf> {
    let mut changed_cache: HashMap<String, Option<Vec<String>>> = HashMap::new();
    let mut stale = HashSet::new();

    for page in pages {
        let rel = page
            .path
            .strip_prefix(&git.root)
            .map(|r| r.to_string_lossy().into_owned())
            .unwrap_or_else(|_| page.path.to_string_lossy().into_owned());

        let base = git
            .last_commit_of(&rel)
            .or_else(|| page.front.last_verified.clone());
        let Some(base) = base else {
            continue; // brand-new uncommitted draft: nothing to compare against
        };

        let changed = changed_cache
            .entry(base.clone())
            .or_insert_with(|| git.changed_between(&base).ok());
        let Some(changed) = changed else {
            continue; // fail-open on unknown base
        };
        if changed.is_empty() {
            continue;
        }

        let hit = page.front.anchors.iter().any(|anchor| {
            let prefix = anchor_prefix(anchor);
            !prefix.is_empty() && changed.iter().any(|f| f.starts_with(prefix))
        });
        if hit {
            stale.insert(page.path.clone());
        }
    }
    stale
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wiki;
    use std::path::Path;
    use std::process::Command;

    fn sh_git(dir: &Path, args: &[&str]) {
        let ok = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .unwrap()
            .status
            .success();
        assert!(ok, "git {args:?} failed");
    }

    const PAGE: &str = "---\ntitle: Payments dedup\ntype: decision\nanchors: [\"src/pay/\"]\nsummary: DB unique constraints for dedup.\nconfidence: high\n---\n\nBody.\n";

    #[test]
    fn ride_along_page_is_fresh_until_anchored_code_changes_again() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        sh_git(dir, &["init", "-q"]);
        sh_git(dir, &["config", "user.email", "t@t"]);
        sh_git(dir, &["config", "user.name", "T"]);

        // Commit 1: code and the page describing it ride together.
        std::fs::create_dir_all(dir.join("src/pay")).unwrap();
        std::fs::create_dir_all(dir.join("wiki/decisions")).unwrap();
        std::fs::write(dir.join("src/pay/a.rs"), "v1").unwrap();
        std::fs::write(dir.join("wiki/decisions/dedup.md"), PAGE).unwrap();
        sh_git(dir, &["add", "-A"]);
        sh_git(dir, &["commit", "-qm", "c1"]);

        let git = Git::discover(dir).unwrap();
        let (pages, _) = wiki::load_wiki(&git.root);
        assert_eq!(pages.len(), 1);
        assert!(
            find_stale(&git, &pages).is_empty(),
            "page committed with its code must be fresh"
        );

        // Uncommitted edits are in-flight work, not staleness.
        std::fs::write(dir.join("src/pay/a.rs"), "v2-wip").unwrap();
        assert!(
            find_stale(&git, &pages).is_empty(),
            "uncommitted change must not stale"
        );

        // Commit 2 touches the anchored code without touching the page → stale.
        sh_git(dir, &["add", "-A"]);
        sh_git(dir, &["commit", "-qm", "c2"]);
        let stale = find_stale(&git, &pages);
        assert_eq!(
            stale.len(),
            1,
            "anchored code changed after page's last commit"
        );
        assert!(stale.contains(&pages[0].path));

        // Commit 3 edits the page itself → re-verified → fresh again.
        std::fs::write(
            dir.join("wiki/decisions/dedup.md"),
            PAGE.replace("Body.", "Body updated after review."),
        )
        .unwrap();
        sh_git(dir, &["add", "-A"]);
        sh_git(dir, &["commit", "-qm", "c3"]);
        let (pages, _) = wiki::load_wiki(&git.root);
        assert!(
            find_stale(&git, &pages).is_empty(),
            "editing the page refreshes its baseline"
        );
    }

    #[test]
    fn unrelated_changes_and_unknown_bases_stay_fresh() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        sh_git(dir, &["init", "-q"]);
        sh_git(dir, &["config", "user.email", "t@t"]);
        sh_git(dir, &["config", "user.name", "T"]);
        std::fs::create_dir_all(dir.join("wiki/decisions")).unwrap();
        std::fs::write(dir.join("wiki/decisions/dedup.md"), PAGE).unwrap();
        std::fs::write(dir.join("readme.md"), "x").unwrap();
        sh_git(dir, &["add", "-A"]);
        sh_git(dir, &["commit", "-qm", "c1"]);
        std::fs::write(dir.join("readme.md"), "y").unwrap();
        sh_git(dir, &["add", "-A"]);
        sh_git(dir, &["commit", "-qm", "c2"]);

        let git = Git::discover(dir).unwrap();
        let (pages, _) = wiki::load_wiki(&git.root);
        assert!(
            find_stale(&git, &pages).is_empty(),
            "changes outside anchors must not stale the page"
        );

        // Uncommitted draft with an unknown last_verified sha: fail-open.
        std::fs::write(
            dir.join("wiki/decisions/new.md"),
            PAGE.replace("last_verified", "x")
                .replace("---\ntitle", "---\nlast_verified: deadbeef\ntitle"),
        )
        .unwrap();
        let (pages, _) = wiki::load_wiki(&git.root);
        assert_eq!(pages.len(), 2);
        assert!(
            find_stale(&git, &pages).is_empty(),
            "unknown base must fail open"
        );
    }
}
