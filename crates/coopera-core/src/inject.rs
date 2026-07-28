use crate::config::Budget;
use crate::wiki::Page;

/// Guidance line — the exact wording proven in spike ② (00-spikes.md):
/// activity map + instruction produced full compliance even on the weakest model.
pub const GUIDANCE: &str = "Before planning or making design decisions, review the team context above. \
Avoid conflicting with or duplicating in-flight teammate work, and align with recorded team decisions. \
For details read the referenced wiki pages (start from wiki/INDEX.md; do not bulk-read the whole wiki).";

#[derive(Debug)]
pub struct InjectionPack {
    pub text: String,
    pub items: usize,
    pub tokens: usize,
}

/// Rough token estimate (chars/4). Good enough for hard-cap budgeting.
pub fn estimate_tokens(s: &str) -> usize {
    s.chars().count() / 4 + 1
}

fn anchor_prefix(anchor: &str) -> &str {
    anchor.trim_end_matches("**").trim_end_matches('*')
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

/// Build the SessionStart injection pack (L0 index + L1 matched summaries)
/// under the hard token budget. Presence section is added in M2 (F5).
pub fn build_pack(
    pages: &[Page],
    branch: &str,
    changed: &[String],
    budget: &Budget,
) -> InjectionPack {
    let mut ranked: Vec<(&Page, i64)> = pages
        .iter()
        .map(|p| (p, score(p, branch, changed)))
        .collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.path.cmp(&b.0.path)));

    let mut lines: Vec<String> = Vec::new();
    let mut wiki_tokens = 0usize;
    let mut items = 0usize;
    for (page, sc) in &ranked {
        let rel = page.path.to_string_lossy();
        let rel = rel
            .split("wiki/")
            .last()
            .map(|s| format!("wiki/{s}"))
            .unwrap_or_else(|| rel.to_string());
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
            rel
        );
        let t = estimate_tokens(&line);
        if wiki_tokens + t > budget.wiki {
            break;
        }
        // Pages with zero anchor relevance still get listed while budget allows —
        // that is the L0 index role — but stop early if nothing matched and we
        // already listed a handful.
        if *sc <= 3 && items >= 5 {
            break;
        }
        wiki_tokens += t;
        items += 1;
        lines.push(line);
    }

    if items == 0 {
        return InjectionPack {
            text: String::new(),
            items: 0,
            tokens: 0,
        };
    }

    let mut text = String::from("<team-context source=\"coopera\">\n## Team knowledge (wiki/)\n");
    text.push_str(&lines.join("\n"));
    text.push_str("\n\n## Guidance\n");
    text.push_str(GUIDANCE);
    text.push_str("\n</team-context>");

    // Enforce the total hard cap defensively.
    let mut tokens = estimate_tokens(&text);
    while tokens > budget.total && lines.len() > 1 {
        lines.pop();
        items -= 1;
        text = format!(
            "<team-context source=\"coopera\">\n## Team knowledge (wiki/)\n{}\n\n## Guidance\n{}\n</team-context>",
            lines.join("\n"),
            GUIDANCE
        );
        tokens = estimate_tokens(&text);
    }

    InjectionPack {
        text,
        items,
        tokens,
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
        parse_page(Path::new(&format!("wiki/{ptype}s/{name}.md")), &src).unwrap()
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
        let pack = build_pack(&pages, "main", &["src/x.rs".into()], &budget);
        assert!(pack.tokens <= 400, "total cap violated: {}", pack.tokens);
        assert!(pack.items < 100);
        assert!(pack.items >= 1);
    }

    #[test]
    fn empty_wiki_injects_nothing() {
        let pack = build_pack(&[], "main", &[], &Budget::default());
        assert_eq!(pack.items, 0);
        assert!(pack.text.is_empty());
    }
}
