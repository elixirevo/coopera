//! Session-digest retention — the hybrid storage tier. Recent digests stay
//! in the working tree (provenance reviewers can see); old ones move to a
//! git-native ref tree and their deletion is staged so it rides with the
//! next code commit like any wiki change (decision 004 symmetry). The
//! archive ref is a browsable snapshot index — the durable source of truth
//! is main history, where every archived digest was once committed. Retrieve
//! with `git cat-file -p refs/coopera/archive/sessions:<name>`.

use crate::gitio::Git;
use crate::wiki::Page;
use std::collections::HashSet;

pub const ARCHIVE_REF: &str = "refs/coopera/archive/sessions";
const SESSIONS_DIR: &str = "coopera/sessions";

#[derive(Debug, Default)]
pub struct SweepResult {
    /// File names moved into the archive ref this run.
    pub archived: Vec<String>,
}

/// Age in days of a digest, from the date its filename carries
/// (`YYYYMMDD-HHMMSS-<sid>.md`, UTC). None for foreign filenames — the
/// sweep never touches files it did not name itself.
fn age_days(file_name: &str, today: jiff::civil::Date) -> Option<i64> {
    let stamp = file_name.get(..8)?;
    let date = jiff::civil::Date::strptime("%Y%m%d", stamp).ok()?;
    let span = date.until(today).ok()?;
    Some(span.get_days() as i64)
}

/// Archive digests older than `retention_days`. Held back: digests still
/// referenced as `source:` by an unreviewed (draft) page — the reviewer may
/// want the evidence — and digests never committed (deleting those would
/// destroy, not archive). Ordering is crash-safe: the archive ref holds the
/// content before anything is removed from the tree. Fail-open: any git
/// error abandons the rest of the sweep for a later run.
pub fn sweep(git: &Git, pages: &[Page], retention_days: u32) -> SweepResult {
    let mut result = SweepResult::default();
    if retention_days == 0 {
        return result; // disabled
    }
    let dir = git.root.join(SESSIONS_DIR);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return result;
    };

    let held: HashSet<String> = pages
        .iter()
        .filter(|p| p.front.confidence == "draft")
        .filter_map(|p| p.front.source.clone())
        .collect();
    let today = jiff::Zoned::now().date();

    let mut candidates: Vec<(String, String)> = Vec::new(); // (name, content)
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.ends_with(".md") {
            continue;
        }
        match age_days(&name, today) {
            Some(age) if age >= retention_days as i64 => {}
            _ => continue,
        }
        let rel = format!("{SESSIONS_DIR}/{name}");
        if held.contains(&rel) {
            continue; // evidence for a pending review
        }
        if git.last_commit_of(&rel).is_none() {
            continue; // not in history yet — deletion would lose it
        }
        let Ok(content) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        candidates.push((name, content));
    }
    if candidates.is_empty() {
        return result;
    }
    candidates.sort();

    // Build the new archive tree on top of the existing one.
    let mut tree_entries = git.ls_tree(ARCHIVE_REF);
    for (name, content) in &candidates {
        let Ok(sha) = git.hash_object(content) else {
            return result;
        };
        tree_entries.retain(|(n, _)| n != name);
        tree_entries.push((name.clone(), sha));
    }
    tree_entries.sort();
    let parent = git.ref_sha(ARCHIVE_REF);
    let ok = git.mktree(&tree_entries).and_then(|tree| {
        git.commit_tree_to_ref(
            ARCHIVE_REF,
            &tree,
            parent.as_deref(),
            &format!("archive {} session digest(s)", candidates.len()),
        )
    });
    if ok.is_err() {
        return result;
    }

    // Content is durably in the ref (and in main history) — now retire the
    // tree copies and stage the deletions for the next code commit.
    for (name, _) in &candidates {
        let rel = format!("{SESSIONS_DIR}/{name}");
        if std::fs::remove_file(git.root.join(&rel)).is_ok() && git.add(&[rel]).is_ok() {
            result.archived.push(name.clone());
        }
    }

    // Best-effort share (non-force: a teammate's concurrent archive wins and
    // ours stays local — harmless, main history has everything either way).
    if !result.archived.is_empty() && git.has_origin() {
        git.spawn_network(&["push", "origin", &format!("{ARCHIVE_REF}:{ARCHIVE_REF}")]);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wiki::parse_page;
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

    fn digest(dir: &Path, name: &str, body: &str) {
        let p = dir.join(SESSIONS_DIR);
        std::fs::create_dir_all(&p).unwrap();
        std::fs::write(p.join(name), body).unwrap();
    }

    #[test]
    fn sweep_archives_old_committed_unheld_digests() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        sh_git(dir, &["init", "-q"]);
        sh_git(dir, &["config", "user.email", "t@example.com"]);
        sh_git(dir, &["config", "user.name", "Tester"]);
        let git = Git::discover(dir).unwrap();

        let today = jiff::Zoned::now().date();
        let fresh_name = format!("{}-120000-freshsss.md", today.strftime("%Y%m%d"));
        digest(dir, "20250101-000000-oldsess1.md", "old digest one");
        digest(dir, "20250102-000000-heldses1.md", "held digest");
        digest(dir, "20250103-000000-uncommit.md", "never committed");
        digest(dir, &fresh_name, "fresh digest");
        // Commit everything except the uncommitted one.
        sh_git(
            dir,
            &[
                "add",
                "coopera/sessions/20250101-000000-oldsess1.md",
                "coopera/sessions/20250102-000000-heldses1.md",
                &format!("coopera/sessions/{fresh_name}"),
            ],
        );
        sh_git(dir, &["commit", "-qm", "seed digests"]);

        // A draft page still cites the held digest as its source.
        let page = parse_page(
            Path::new("coopera/decisions/001-x.md"),
            "---\ntitle: X\ntype: decision\nanchors: [\"src/\"]\nsummary: S.\nconfidence: draft\nsource: coopera/sessions/20250102-000000-heldses1.md\n---\nbody",
        )
        .unwrap();

        let swept = sweep(&git, &[page], 90);
        assert_eq!(swept.archived, vec!["20250101-000000-oldsess1.md"]);
        assert!(!dir
            .join("coopera/sessions/20250101-000000-oldsess1.md")
            .exists());
        assert!(dir
            .join("coopera/sessions/20250102-000000-heldses1.md")
            .exists());
        assert!(dir
            .join("coopera/sessions/20250103-000000-uncommit.md")
            .exists());
        assert!(dir.join(format!("coopera/sessions/{fresh_name}")).exists());

        // Content retrievable from the archive ref; deletion staged.
        let archived = git
            .read_ref_file(ARCHIVE_REF, "20250101-000000-oldsess1.md")
            .unwrap();
        assert_eq!(archived, "old digest one");
        let staged = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["diff", "--cached", "--name-status"])
            .output()
            .unwrap();
        let staged = String::from_utf8_lossy(&staged.stdout).into_owned();
        assert!(
            staged.contains("D\tcoopera/sessions/20250101-000000-oldsess1.md"),
            "deletion must ride with the next commit: {staged}"
        );

        // Second sweep is incremental: a newly-old digest joins the same
        // tree, parented on the first archive commit. The draft page still
        // holds its evidence back.
        digest(dir, "20250104-000000-oldsess2.md", "old digest two");
        sh_git(
            dir,
            &["add", "coopera/sessions/20250104-000000-oldsess2.md"],
        );
        sh_git(dir, &["commit", "-qm", "another"]);
        let first_commit = git.ref_sha(ARCHIVE_REF).unwrap();
        let page = parse_page(
            Path::new("coopera/decisions/001-x.md"),
            "---\ntitle: X\ntype: decision\nanchors: [\"src/\"]\nsummary: S.\nconfidence: draft\nsource: coopera/sessions/20250102-000000-heldses1.md\n---\nbody",
        )
        .unwrap();
        let swept = sweep(&git, &[page], 90);
        assert_eq!(swept.archived, vec!["20250104-000000-oldsess2.md"]);
        assert_eq!(
            git.read_ref_file(ARCHIVE_REF, "20250101-000000-oldsess1.md")
                .unwrap(),
            "old digest one",
            "earlier archives survive later sweeps"
        );
        let log = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["log", "--format=%P", "-1", ARCHIVE_REF])
            .output()
            .unwrap();
        let parents = String::from_utf8_lossy(&log.stdout).into_owned();
        assert!(
            parents.trim().contains(&first_commit),
            "archive commits chain: {parents}"
        );
    }

    #[test]
    fn sweep_disabled_and_foreign_files_untouched() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        sh_git(dir, &["init", "-q"]);
        sh_git(dir, &["config", "user.email", "t@example.com"]);
        sh_git(dir, &["config", "user.name", "Tester"]);
        let git = Git::discover(dir).unwrap();
        digest(dir, "20250101-000000-oldsess1.md", "old");
        digest(dir, "notes-not-a-digest.md", "foreign");
        sh_git(dir, &["add", "-A"]);
        sh_git(dir, &["commit", "-qm", "seed"]);

        assert!(sweep(&git, &[], 0).archived.is_empty(), "0 disables");
        let swept = sweep(&git, &[], 90);
        assert_eq!(swept.archived, vec!["20250101-000000-oldsess1.md"]);
        assert!(
            dir.join("coopera/sessions/notes-not-a-digest.md").exists(),
            "files the sweep did not name are never touched"
        );
    }
}
