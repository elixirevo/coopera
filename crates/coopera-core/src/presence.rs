//! F5 — presence board on git custom refs (design: 00-harness §3, decision
//! page wiki/decisions/001): one ref per session under
//! `refs/coopera/presence/<user>/<session>`, force-pushed only by its owner,
//! fetched once at SessionStart. No servers; conflict-free by namespacing.

use crate::gitio::Git;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;

pub const REF_PREFIX: &str = "refs/coopera/presence";
pub const FILE_NAME: &str = "presence.yaml";
/// Entries whose last_seen is older than this are hidden from the map
/// (crashed sessions age out of display; refs are cleaned lazily).
pub const MAX_AGE_SECS: i64 = 24 * 3600;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresenceEntry {
    pub user: String,
    pub session: String,
    pub tool: String,
    pub branch: String,
    pub started: String,
    pub last_seen: String,
    pub intent: String,
    pub status: String, // active | done
}

/// Ref path components must stay flat and shell-safe.
pub fn sanitize(component: &str) -> String {
    let s: String = component
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let s = s.trim_matches('-').to_string();
    if s.is_empty() {
        "unknown".to_string()
    } else {
        s.chars().take(48).collect()
    }
}

pub fn ref_name(user: &str, session: &str) -> String {
    format!("{REF_PREFIX}/{}/{}", sanitize(user), sanitize(session))
}

/// Session ids are UUIDs; 8 chars is unambiguous within a team's active set.
pub fn short_session(session: &str) -> String {
    session.chars().take(8).collect()
}

/// Write the entry to its local ref; push synchronously (bounded by
/// `timeout`) when `sync` is set, otherwise fire-and-forget. Fail-open:
/// a missing remote or slow network never fails the caller.
pub fn publish(git: &Git, entry: &PresenceEntry, sync: bool, timeout: Duration) -> Result<()> {
    let refname = ref_name(&entry.user, &entry.session);
    let yaml = serde_yaml::to_string(entry)?;
    git.update_ref_with_file(&refname, FILE_NAME, &yaml, "presence")?;
    if git.has_origin() {
        let refspec = format!("+{refname}:{refname}");
        if sync {
            let _ = git.run_network(&["push", "origin", &refspec], timeout);
        } else {
            git.spawn_network(&["push", "origin", &refspec]);
        }
    }
    Ok(())
}

/// Read one entry back from its local ref.
pub fn load(git: &Git, user: &str, session: &str) -> Option<PresenceEntry> {
    let yaml = git
        .read_ref_file(&ref_name(user, session), FILE_NAME)
        .ok()?;
    serde_yaml::from_str(&yaml).ok()
}

/// All locally known entries (call `fetch` first for freshness).
pub fn load_all(git: &Git) -> Vec<PresenceEntry> {
    git.list_refs(REF_PREFIX)
        .iter()
        .filter_map(|r| {
            let yaml = git.read_ref_file(r, FILE_NAME).ok()?;
            serde_yaml::from_str(&yaml).ok()
        })
        .collect()
}

/// Fetch teammates' presence refs and branch heads in one bounded call.
/// `--prune` on the presence refspec makes remote deletions take effect.
pub fn fetch(git: &Git, timeout: Duration) -> bool {
    if !git.has_origin() {
        return false;
    }
    git.run_network(
        &[
            "fetch",
            "--prune",
            "origin",
            &format!("+{REF_PREFIX}/*:{REF_PREFIX}/*"),
            "+refs/heads/*:refs/remotes/origin/*",
        ],
        timeout,
    )
    .is_ok()
}

/// Remove a session's ref locally and remotely (session cleanup).
pub fn remove(git: &Git, user: &str, session: &str, timeout: Duration) {
    let refname = ref_name(user, session);
    git.delete_ref(&refname);
    if git.has_origin() {
        let _ = git.run_network(&["push", "origin", &format!(":{refname}")], timeout);
    }
}

fn age_secs(now: &jiff::Timestamp, iso: &str) -> Option<i64> {
    let ts: jiff::Timestamp = iso.parse().ok()?;
    Some(now.duration_since(ts).as_secs())
}

/// Sanitized session ids with a presence entry seen within `max_age_secs`.
/// Used to keep retroactive distillation away from sessions that are still
/// live (their transcript is mid-flight, not a finished session).
pub fn recent_session_ids(
    git: &Git,
    now: jiff::Timestamp,
    max_age_secs: i64,
) -> std::collections::HashSet<String> {
    load_all(git)
        .iter()
        .filter(|e| {
            age_secs(&now, &e.last_seen)
                .map(|a| a <= max_age_secs)
                .unwrap_or(false)
        })
        .map(|e| sanitize(&e.session))
        .collect()
}

fn age_label(secs: i64) -> String {
    if secs < 90 {
        "just now".to_string()
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else {
        format!("{}h ago", secs / 3600)
    }
}

/// Render the activity map: live teammate sessions first, then recently
/// active remote branches (push is the primary signal — 00-harness §3.2).
/// `self_session` is excluded; entries older than MAX_AGE_SECS are hidden.
pub fn format_lines(
    entries: &[PresenceEntry],
    branches: &[(String, i64, String)],
    now: jiff::Timestamp,
    self_session: Option<&str>,
) -> Vec<String> {
    let mut lines = Vec::new();
    for e in entries {
        if e.status == "done" {
            continue;
        }
        if let Some(own) = self_session {
            if sanitize(&e.session) == sanitize(own) {
                continue;
            }
        }
        let Some(age) = age_secs(&now, &e.last_seen) else {
            continue;
        };
        if age > MAX_AGE_SECS {
            continue;
        }
        lines.push(format!(
            "- {} · {} · branch {} — {} ({}, {})",
            e.user,
            e.tool,
            e.branch,
            e.intent,
            e.status,
            age_label(age)
        ));
    }
    let now_unix = now.as_second();
    for (name, date, subject) in branches {
        let age = now_unix - date;
        if age > 14 * 24 * 3600 {
            continue;
        }
        lines.push(format!(
            "- (branch) {name} — last commit: \"{subject}\" ({})",
            age_label(age)
        ));
    }
    lines
}

/// Materialize the map for hook-less consumers (Antigravity reads this file;
/// UserPromptSubmit reads it instead of touching the network).
pub fn materialize(repo_root: &Path, lines: &[String]) -> Result<()> {
    let dir = repo_root.join(".coopera/cache");
    std::fs::create_dir_all(&dir)?;
    let body = if lines.is_empty() {
        "# Teammate activity\n\n(none known)\n".to_string()
    } else {
        format!("# Teammate activity\n\n{}\n", lines.join("\n"))
    };
    std::fs::write(dir.join("presence.md"), body)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
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

    fn entry(user: &str, session: &str, intent: &str, last_seen: &str) -> PresenceEntry {
        PresenceEntry {
            user: user.into(),
            session: session.into(),
            tool: "claude-code".into(),
            branch: "main".into(),
            started: last_seen.into(),
            last_seen: last_seen.into(),
            intent: intent.into(),
            status: "active".into(),
        }
    }

    #[test]
    fn sanitize_keeps_refs_safe() {
        assert_eq!(sanitize("Elixir Kim"), "Elixir-Kim");
        assert_eq!(sanitize("../evil"), "evil");
        assert_eq!(sanitize(""), "unknown");
    }

    #[test]
    fn publish_fetch_roundtrip_via_bare_origin() {
        let origin = tempfile::tempdir().unwrap();
        sh_git(origin.path(), &["init", "-q", "--bare"]);

        let a = tempfile::tempdir().unwrap();
        sh_git(a.path(), &["init", "-q"]);
        sh_git(
            a.path(),
            &["remote", "add", "origin", origin.path().to_str().unwrap()],
        );
        let git_a = Git::discover(a.path()).unwrap();

        let now = jiff::Timestamp::now().to_string();
        publish(
            &git_a,
            &entry("alice", "sess-aaaa", "refactor payments", &now),
            true,
            Duration::from_secs(5),
        )
        .unwrap();

        // Developer B fetches and sees Alice's session.
        let b = tempfile::tempdir().unwrap();
        sh_git(b.path(), &["init", "-q"]);
        sh_git(
            b.path(),
            &["remote", "add", "origin", origin.path().to_str().unwrap()],
        );
        let git_b = Git::discover(b.path()).unwrap();
        assert!(fetch(&git_b, Duration::from_secs(5)));
        let entries = load_all(&git_b);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].user, "alice");
        assert_eq!(entries[0].intent, "refactor payments");

        // Cleanup propagates on the next fetch (prune).
        remove(&git_a, "alice", "sess-aaaa", Duration::from_secs(5));
        assert!(fetch(&git_b, Duration::from_secs(5)));
        assert!(load_all(&git_b).is_empty(), "pruned after remote delete");
    }

    #[test]
    fn recent_session_ids_reflect_last_seen_age() {
        let dir = tempfile::tempdir().unwrap();
        sh_git(dir.path(), &["init", "-q"]);
        let git = Git::discover(dir.path()).unwrap();
        let now = jiff::Timestamp::now();
        let fresh = now.to_string();
        let old = now
            .checked_sub(jiff::Span::new().hours(2))
            .unwrap()
            .to_string();
        publish(
            &git,
            &entry("me", "sess-live", "working", &fresh),
            false,
            Duration::from_secs(1),
        )
        .unwrap();
        publish(
            &git,
            &entry("me", "sess-dead", "crashed", &old),
            false,
            Duration::from_secs(1),
        )
        .unwrap();
        let ids = recent_session_ids(&git, now, 1800);
        assert!(ids.contains("sess-live"), "{ids:?}");
        assert!(!ids.contains("sess-dead"), "{ids:?}");
    }

    #[test]
    fn format_hides_self_done_and_aged_entries() {
        let now = jiff::Timestamp::now();
        let fresh = now.to_string();
        let old = now
            .checked_sub(jiff::Span::new().hours(30))
            .unwrap_or(now)
            .to_string();
        let mut done = entry("bob", "sess-done", "x", &fresh);
        done.status = "done".into();
        let entries = vec![
            entry("alice", "sess-aaaa", "refactor payments", &fresh),
            entry("me", "sess-self", "current work", &fresh),
            entry("carol", "sess-old", "ancient", &old),
            done,
        ];
        let branches = vec![(
            "origin/pay-idem".to_string(),
            now.as_second() - 7200,
            "wip retry".to_string(),
        )];
        let lines = format_lines(&entries, &branches, now, Some("sess-self"));
        let joined = lines.join("\n");
        assert!(joined.contains("alice"), "{joined}");
        assert!(!joined.contains("current work"), "self excluded: {joined}");
        assert!(!joined.contains("ancient"), "aged out: {joined}");
        assert!(!joined.contains("sess-done"), "done hidden: {joined}");
        assert!(joined.contains("origin/pay-idem"), "{joined}");
        assert!(joined.contains("2h ago"), "{joined}");
    }
}
