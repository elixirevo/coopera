use serde::{Deserialize, Serialize};

/// Session digest — the fixed schema (≤30 rendered lines) that distillation
/// produces. Fixed shape is the quality equalizer across different
/// developers' models (01-idea-brief, implementation decision 1). Filled by
/// `coopera distill` from the hosting tool's own headless agent, rendered to
/// coopera/sessions/, and staged onto the working branch with any wiki diff.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Digest {
    pub session: String,
    pub author: String,
    pub tool: String,
    pub timestamp: String,
    /// One line: what this session was trying to do.
    pub intent: String,
    /// Up to 2 lines each: what was decided and why.
    pub decisions: Vec<String>,
    /// One line each: facts learned about the codebase.
    pub learnings: Vec<String>,
    /// Touched surface: files/modules.
    pub touched: Vec<String>,
}

pub const MAX_RENDERED_LINES: usize = 30;

impl Digest {
    /// Render to markdown, hard-capped at MAX_RENDERED_LINES.
    pub fn render(&self) -> String {
        let mut lines: Vec<String> = Vec::new();
        lines.push(format!(
            "# Session digest — {} ({})",
            self.session, self.tool
        ));
        lines.push(format!("author: {} · {}", self.author, self.timestamp));
        lines.push(format!("intent: {}", self.intent));
        if !self.decisions.is_empty() {
            lines.push("decisions:".to_string());
            for d in &self.decisions {
                lines.push(format!("- {d}"));
            }
        }
        if !self.learnings.is_empty() {
            lines.push("learnings:".to_string());
            for l in &self.learnings {
                lines.push(format!("- {l}"));
            }
        }
        if !self.touched.is_empty() {
            lines.push(format!("touched: {}", self.touched.join(", ")));
        }
        lines.truncate(MAX_RENDERED_LINES);
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_respects_line_cap() {
        let d = Digest {
            session: "s1".into(),
            decisions: (0..50).map(|i| format!("decision {i}")).collect(),
            ..Default::default()
        };
        assert!(d.render().lines().count() <= MAX_RENDERED_LINES);
    }
}
