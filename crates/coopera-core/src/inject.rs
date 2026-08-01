use crate::config::Budget;
use crate::wiki::{anchor_prefix, Page};
use std::collections::HashSet;
use std::path::PathBuf;

/// Guidance line — the exact wording proven in spike ② (00-spikes.md):
/// activity map + instruction produced full compliance even on the weakest model.
pub const GUIDANCE: &str = "Before planning or making design decisions, review the team context above. \
Avoid conflicting with or duplicating in-flight teammate work, and align with recorded team decisions. \
For details read the referenced wiki pages (start from coopera/INDEX.md; do not bulk-read the whole wiki).";

#[derive(Debug)]
pub struct InjectionPack {
    pub text: String,
    pub items: usize,
    pub tokens: usize,
    /// Pages whose anchored code changed since verification — injected with a
    /// [STALE — re-verify] marker while budget allows, pointers otherwise.
    pub stale: usize,
    /// Teammate-activity lines included (presence + branch signals).
    pub presence_items: usize,
}

/// Rough token estimate (chars/4). Good enough for hard-cap budgeting.
pub fn estimate_tokens(s: &str) -> usize {
    s.chars().count() / 4 + 1
}

/// Relevance score. Anchor path match against changed files dominates;
/// page type breaks ties (decisions matter most); drafts rank below reviewed.
fn score(page: &Page, branch: &str, changed: &[String]) -> i64 {
    let mut s: i64 = match page.front.page_type.as_str() {
        "decision" => 3,
        "module" => 2,
        _ => 1,
    };
    for anchor in &page.front.anchors {
        let prefix = anchor_prefix(anchor);
        if prefix.is_empty() {
            continue;
        }
        if changed.iter().any(|f| f.starts_with(prefix)) {
            s += 10;
        }
        if branch.contains(
            prefix
                .trim_end_matches('/')
                .split('/')
                .next_back()
                .unwrap_or(""),
        ) {
            s += 2;
        }
    }
    if page.front.confidence == "draft" {
        s -= 1;
    }
    s
}

fn rel_path(page: &Page) -> String {
    // Normalize separators so the split works on Windows paths too.
    let rel = page.path.to_string_lossy().replace('\\', "/");
    rel.split("coopera/")
        .last()
        .map(|s| format!("coopera/{s}"))
        .unwrap_or_else(|| rel.to_string())
}

/// Build the SessionStart injection pack (L0 index + L1 matched summaries)
/// under the hard token budget. Stale pages (anchored code changed since
/// verification) rank after fresh ones and carry an explicit
/// [STALE — re-verify] marker — flagged old knowledge beats a bare pointer,
/// but silently injecting it as fresh would be worse than nothing
/// (00-harness §2.4). Whatever exceeds the budget degrades to pointers.
pub fn build_pack(
    pages: &[Page],
    branch: &str,
    changed: &[String],
    budget: &Budget,
    stale: &HashSet<PathBuf>,
    presence: &[String],
) -> InjectionPack {
    let mut ranked: Vec<(&Page, i64)> = pages
        .iter()
        .filter(|p| !stale.contains(&p.path))
        .map(|p| (p, score(p, branch, changed)))
        .collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.path.cmp(&b.0.path)));

    let mut lines: Vec<String> = Vec::new();
    let mut wiki_tokens = 0usize;
    for (page, sc) in &ranked {
        let marker = if page.front.confidence == "draft" {
            " [unreviewed]"
        } else {
            ""
        };
        let line = format!(
            "- [{}]{} {} — {} ({})",
            page.front.page_type,
            marker,
            page.front.title,
            page.front.summary.replace('\n', " "),
            rel_path(page)
        );
        let t = estimate_tokens(&line);
        if wiki_tokens + t > budget.wiki {
            break;
        }
        // Pages with zero anchor relevance still get listed while budget allows —
        // that is the L0 index role — but stop early if nothing matched and we
        // already listed a handful.
        if *sc <= 3 && lines.len() >= 5 {
            break;
        }
        wiki_tokens += t;
        lines.push(line);
    }

    // Stale summaries after fresh ones, same relevance order; overflow
    // degrades to a pointer list so existence is never hidden.
    let mut stale_ranked: Vec<(&Page, i64)> = pages
        .iter()
        .filter(|p| stale.contains(&p.path))
        .map(|p| (p, score(p, branch, changed)))
        .collect();
    stale_ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.path.cmp(&b.0.path)));
    let stale_count = stale_ranked.len();

    let mut stale_lines: Vec<String> = Vec::new();
    let mut stale_leftover: Vec<String> = Vec::new();
    for (page, _) in &stale_ranked {
        let line = format!(
            "- [{}][STALE — re-verify] {} — {} ({})",
            page.front.page_type,
            page.front.title,
            page.front.summary.replace('\n', " "),
            rel_path(page)
        );
        let t = estimate_tokens(&line);
        if wiki_tokens + t > budget.wiki {
            stale_leftover.push(rel_path(page));
            continue;
        }
        wiki_tokens += t;
        stale_lines.push(line);
    }
    let stale_note = if stale_leftover.is_empty() {
        String::new()
    } else {
        let shown: Vec<&str> = stale_leftover.iter().take(5).map(String::as_str).collect();
        let more = if stale_leftover.len() > shown.len() {
            format!(" (+{} more)", stale_leftover.len() - shown.len())
        } else {
            String::new()
        };
        format!(
            "Stale — code changed since these were verified; re-verify before relying on them: {}{more}",
            shown.join(", ")
        )
    };

    // Presence section (F5/F7) under its own budget slice, teammate work first.
    let mut presence_lines: Vec<String> = Vec::new();
    let mut presence_tokens = 0usize;
    for line in presence {
        let t = estimate_tokens(line);
        if presence_tokens + t > budget.presence {
            break;
        }
        presence_tokens += t;
        presence_lines.push(line.clone());
    }

    if lines.is_empty()
        && stale_lines.is_empty()
        && stale_note.is_empty()
        && presence_lines.is_empty()
    {
        return InjectionPack {
            text: String::new(),
            items: 0,
            tokens: 0,
            stale: 0,
            presence_items: 0,
        };
    }

    let compose = |fresh: &[String], stale_shown: &[String]| {
        let mut text = String::from("<team-context source=\"coopera\">\n");
        if !presence_lines.is_empty() {
            text.push_str("## Active teammate work\n");
            text.push_str(&presence_lines.join("\n"));
            text.push_str("\n\n");
        }
        if !fresh.is_empty() || !stale_shown.is_empty() || !stale_note.is_empty() {
            text.push_str("## Team knowledge (coopera/)\n");
            if fresh.is_empty() && stale_shown.is_empty() {
                text.push_str("(no fresh pages)");
            } else {
                let mut all: Vec<&str> = fresh.iter().map(String::as_str).collect();
                all.extend(stale_shown.iter().map(String::as_str));
                text.push_str(&all.join("\n"));
            }
            if !stale_note.is_empty() {
                text.push_str("\n\n");
                text.push_str(&stale_note);
            }
            text.push_str("\n\n");
        }
        text.push_str("## Guidance\n");
        text.push_str(GUIDANCE);
        text.push_str("\n</team-context>");
        text
    };

    // Enforce the total hard cap defensively — stale summaries are the first
    // to go, fresh pages only after that.
    let mut text = compose(&lines, &stale_lines);
    let mut tokens = estimate_tokens(&text);
    while tokens > budget.total && (!stale_lines.is_empty() || lines.len() > 1) {
        if !stale_lines.is_empty() {
            stale_lines.pop();
        } else {
            lines.pop();
        }
        text = compose(&lines, &stale_lines);
        tokens = estimate_tokens(&text);
    }

    InjectionPack {
        items: lines.len() + stale_lines.len(),
        text,
        tokens,
        stale: stale_count,
        presence_items: presence_lines.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wiki::parse_page;
    use std::path::Path;

    fn page(name: &str, ptype: &str, anchors: &str, summary: &str) -> Page {
        let src = format!(
            "---\ntitle: {name}\ntype: {ptype}\nanchors: [{anchors}]\nsummary: {summary}\nconfidence: high\n---\nbody"
        );
        parse_page(Path::new(&format!("coopera/{ptype}s/{name}.md")), &src).unwrap()
    }

    fn no_stale() -> HashSet<PathBuf> {
        HashSet::new()
    }

    #[test]
    fn anchor_match_outranks_type_weight() {
        let pages = vec![
            page(
                "unrelated-decision",
                "decision",
                "\"src/auth/\"",
                "Auth decision.",
            ),
            page(
                "payments-note",
                "concept",
                "\"src/payments/\"",
                "Payments concept.",
            ),
        ];
        let pack = build_pack(
            &pages,
            "main",
            &["src/payments/retry.rs".into()],
            &Budget::default(),
            &no_stale(),
            &[],
        );
        let first = pack.text.lines().nth(2).unwrap();
        assert!(
            first.contains("payments-note"),
            "anchor-matched page should rank first: {first}"
        );
        assert_eq!(pack.items, 2);
        assert!(pack.text.contains("Guidance"));
    }

    #[test]
    fn budget_caps_item_count() {
        let pages: Vec<Page> = (0..100)
            .map(|i| {
                page(
                    &format!("p{i:03}"),
                    "concept",
                    "\"src/\"",
                    "A summary line for budget testing purposes.",
                )
            })
            .collect();
        let budget = Budget {
            total: 400,
            presence: 0,
            wiki: 200,
            instructions: 100,
        };
        let pack = build_pack(
            &pages,
            "main",
            &["src/x.rs".into()],
            &budget,
            &no_stale(),
            &[],
        );
        assert!(pack.tokens <= 400, "total cap violated: {}", pack.tokens);
        assert!(pack.items < 100);
        assert!(pack.items >= 1);
    }

    #[test]
    fn empty_wiki_injects_nothing() {
        let pack = build_pack(&[], "main", &[], &Budget::default(), &no_stale(), &[]);
        assert_eq!(pack.items, 0);
        assert!(pack.text.is_empty());
    }

    #[test]
    fn stale_pages_inject_flagged_summaries_after_fresh() {
        let fresh = page(
            "payments-note",
            "concept",
            "\"src/payments/\"",
            "Payments concept.",
        );
        let old = page(
            "old-decision",
            "decision",
            "\"src/auth/\"",
            "Auth uses session tokens.",
        );
        let stale: HashSet<PathBuf> = [old.path.clone()].into();
        let pack = build_pack(&[fresh, old], "main", &[], &Budget::default(), &stale, &[]);
        assert_eq!(pack.items, 2, "fresh + flagged stale are both injected");
        assert_eq!(pack.stale, 1);
        assert!(
            pack.text
                .contains("[STALE — re-verify] Auth uses session tokens")
                || pack
                    .text
                    .contains("[STALE — re-verify] old-decision — Auth uses session tokens."),
            "stale summary must be injected with an explicit warning: {}",
            pack.text
        );
        assert!(pack.text.contains("coopera/decisions/old-decision.md"));
        let fresh_idx = pack.text.find("payments-note").unwrap();
        let stale_idx = pack.text.find("old-decision").unwrap();
        assert!(
            fresh_idx < stale_idx,
            "fresh pages rank first: {}",
            pack.text
        );
    }

    #[test]
    fn stale_overflow_degrades_to_pointers() {
        let old = page(
            "only",
            "concept",
            "\"src/\"",
            "A fairly long stale summary that cannot possibly fit a tiny budget.",
        );
        let stale: HashSet<PathBuf> = [old.path.clone()].into();
        let tiny = Budget {
            total: 400,
            presence: 0,
            wiki: 5,
            instructions: 100,
        };
        let pack = build_pack(&[old], "main", &[], &tiny, &stale, &[]);
        assert_eq!(pack.items, 0);
        assert_eq!(pack.stale, 1);
        assert!(pack.text.contains("(no fresh pages)"));
        assert!(
            !pack.text.contains("cannot possibly fit"),
            "over-budget stale summary must not be injected: {}",
            pack.text
        );
        assert!(
            pack.text.contains("Stale") && pack.text.contains("coopera/concepts/only.md"),
            "over-budget stale page keeps its pointer: {}",
            pack.text
        );
    }

    #[test]
    fn presence_section_leads_and_respects_budget() {
        let presence = vec![
            "- alice · claude-code · branch pay — refactor retries (active, just now)".to_string(),
            "- (branch) origin/pay-idem — last commit: \"wip\" (2h ago)".to_string(),
        ];
        let pages = vec![page("note", "concept", "\"src/\"", "Summary.")];
        let pack = build_pack(
            &pages,
            "main",
            &[],
            &Budget::default(),
            &no_stale(),
            &presence,
        );
        assert_eq!(pack.presence_items, 2);
        let p_idx = pack.text.find("Active teammate work").unwrap();
        let k_idx = pack.text.find("Team knowledge").unwrap();
        assert!(p_idx < k_idx, "presence must lead: {}", pack.text);

        let tiny = Budget {
            presence: 10,
            ..Budget::default()
        };
        let pack = build_pack(&pages, "main", &[], &tiny, &no_stale(), &presence);
        assert!(pack.presence_items < 2, "presence budget must truncate");
    }
}
