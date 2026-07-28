//! F4 — `coopera wiki lint`: enforce the page schema so the wiki stays
//! injectable (small pages, required summaries/anchors).

use coopera_core::gitio::Git;
use coopera_core::wiki;

pub fn lint() -> i32 {
    let Ok(cwd) = std::env::current_dir() else {
        return 1;
    };
    let git = match Git::discover(&cwd) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("coopera wiki lint: {e}");
            return 1;
        }
    };

    let (pages, mut issues) = wiki::load_wiki(&git.root);
    for page in &pages {
        issues.extend(wiki::lint(page));
    }

    if issues.is_empty() {
        println!("coopera wiki lint: {} page(s), no issues", pages.len());
        0
    } else {
        for issue in &issues {
            eprintln!("{}: {}", issue.path.display(), issue.message);
        }
        eprintln!(
            "coopera wiki lint: {} issue(s) in {} page(s)",
            issues.len(),
            pages.len()
        );
        1
    }
}
