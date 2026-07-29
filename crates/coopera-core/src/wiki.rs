use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The team wiki directory at the repo root. A visible, tool-owned name:
/// hidden dirs are skipped by default agent search (ripgrep — measured), and
/// a generic `wiki/` collides with existing repos / reads as project files.
pub const WIKI_DIR: &str = "coopera";

pub const PAGE_TYPES: [&str; 4] = ["concept", "module", "decision", "playbook"];
pub const MAX_BODY_LINES: usize = 100;

/// Wiki page frontmatter — the schema is the contract (03-prd F4).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Frontmatter {
    pub title: String,
    #[serde(rename = "type")]
    pub page_type: String,
    #[serde(default)]
    pub anchors: Vec<String>,
    #[serde(default)]
    pub triggers: Vec<String>,
    /// 1–3 line summary. This is what gets injected at L1 — required.
    pub summary: String,
    /// Commit the page was last verified against. None = never verified (draft).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_verified: Option<String>,
    /// "high" (human-reviewed) or "draft" (auto-distilled, unreviewed).
    #[serde(default = "default_confidence")]
    pub confidence: String,
    /// Source session digest path, if auto-distilled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

fn default_confidence() -> String {
    "draft".to_string()
}

#[derive(Debug, Clone)]
pub struct Page {
    pub path: PathBuf,
    pub front: Frontmatter,
    pub body: String,
}

#[derive(Debug)]
pub struct LintIssue {
    pub path: PathBuf,
    pub message: String,
}

/// Parse a page: `---` frontmatter block followed by markdown body.
pub fn parse_page(path: &Path, content: &str) -> Result<Page> {
    let content = content.trim_start_matches('\u{feff}');
    let Some(rest) = content.strip_prefix("---") else {
        bail!(
            "{}: missing frontmatter (must start with ---)",
            path.display()
        );
    };
    let Some(end) = rest.find("\n---") else {
        bail!("{}: unterminated frontmatter", path.display());
    };
    let yaml = &rest[..end];
    let body = rest[end + 4..].trim_start_matches('\n').to_string();
    let front: Frontmatter = serde_yaml::from_str(yaml)
        .with_context(|| format!("{}: invalid frontmatter", path.display()))?;
    Ok(Page {
        path: path.to_path_buf(),
        front,
        body,
    })
}

/// Load all pages under wiki/{concepts,modules,decisions,playbooks}.
/// INDEX.md and sessions/ are not pages. Unparseable files are returned as errors.
pub fn load_wiki(repo_root: &Path) -> (Vec<Page>, Vec<LintIssue>) {
    let mut pages = Vec::new();
    let mut errors = Vec::new();
    for sub in ["concepts", "modules", "decisions", "playbooks"] {
        let dir = repo_root.join(WIKI_DIR).join(sub);
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            match std::fs::read_to_string(&path) {
                Ok(content) => match parse_page(&path, &content) {
                    Ok(page) => pages.push(page),
                    Err(e) => errors.push(LintIssue {
                        path,
                        message: e.to_string(),
                    }),
                },
                Err(e) => errors.push(LintIssue {
                    path,
                    message: e.to_string(),
                }),
            }
        }
    }
    pages.sort_by(|a, b| a.path.cmp(&b.path));
    (pages, errors)
}

/// Anchor glob prefix ("src/payments/**" → "src/payments/"). Shared by
/// injection ranking and staleness matching.
pub fn anchor_prefix(anchor: &str) -> &str {
    anchor.trim_end_matches("**").trim_end_matches('*')
}

/// Serialize a page back to disk form (frontmatter + body).
pub fn render(front: &Frontmatter, body: &str) -> Result<String> {
    let yaml = serde_yaml::to_string(front).context("failed to serialize frontmatter")?;
    let mut body = body.trim_end().to_string();
    body.push('\n');
    Ok(format!("---\n{yaml}---\n\n{body}"))
}

/// F4 lint rules: summary/anchors required, body length cap, known type.
pub fn lint(page: &Page) -> Vec<LintIssue> {
    let mut issues = Vec::new();
    let mut push = |m: String| {
        issues.push(LintIssue {
            path: page.path.clone(),
            message: m,
        })
    };
    if page.front.title.trim().is_empty() {
        push("title is required".into());
    }
    if !PAGE_TYPES.contains(&page.front.page_type.as_str()) {
        push(format!(
            "type must be one of {PAGE_TYPES:?}, got '{}'",
            page.front.page_type
        ));
    }
    if page.front.summary.trim().is_empty() {
        push("summary is required (it is what gets injected)".into());
    }
    if page.front.summary.lines().count() > 3 {
        push("summary must be at most 3 lines".into());
    }
    if page.front.anchors.is_empty() {
        push("anchors are required (relevance ranking depends on them)".into());
    }
    if !["high", "draft"].contains(&page.front.confidence.as_str()) {
        push(format!(
            "confidence must be 'high' or 'draft', got '{}'",
            page.front.confidence
        ));
    }
    let body_lines = page.body.lines().count();
    if body_lines > MAX_BODY_LINES {
        push(format!(
            "body has {body_lines} lines, max {MAX_BODY_LINES} (one page, one topic)"
        ));
    }
    issues
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = "---\ntitle: Payment idempotency\ntype: concept\nanchors: [\"src/payments/\"]\ntriggers: [idempotency, retry]\nsummary: Use DB unique constraints for charge dedup.\nconfidence: high\n---\n\nBody text.\n";

    #[test]
    fn parses_valid_page() {
        let p = parse_page(Path::new("x.md"), GOOD).unwrap();
        assert_eq!(p.front.title, "Payment idempotency");
        assert_eq!(p.front.anchors, vec!["src/payments/"]);
        assert_eq!(p.body.trim(), "Body text.");
        assert!(lint(&p).is_empty());
    }

    #[test]
    fn lint_catches_missing_anchors_and_bad_type() {
        let src = "---\ntitle: T\ntype: nonsense\nsummary: s\n---\nbody";
        let p = parse_page(Path::new("x.md"), src).unwrap();
        let issues = lint(&p);
        assert!(issues.iter().any(|i| i.message.contains("anchors")));
        assert!(issues.iter().any(|i| i.message.contains("type")));
    }

    #[test]
    fn rejects_file_without_frontmatter() {
        assert!(parse_page(Path::new("x.md"), "just text").is_err());
    }

    #[test]
    fn render_round_trips() {
        let p = parse_page(Path::new("x.md"), GOOD).unwrap();
        let rendered = render(&p.front, &p.body).unwrap();
        let again = parse_page(Path::new("x.md"), &rendered).unwrap();
        assert_eq!(again.front.title, p.front.title);
        assert_eq!(again.front.anchors, p.front.anchors);
        assert_eq!(again.body.trim(), p.body.trim());
        assert!(
            !rendered.contains("last_verified"),
            "None fields must be omitted: {rendered}"
        );
    }
}
