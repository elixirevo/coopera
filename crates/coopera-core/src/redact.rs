use regex::Regex;
use std::sync::LazyLock;

/// Secret patterns stripped from anything coopera writes or publishes
/// (digests, wiki drafts, presence). Non-functional requirement: redaction
/// runs before any persistence.
static PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        // key = value style assignments for sensitive names
        r#"(?i)\b(api[_-]?key|secret|token|passwd|password|credential)s?\b\s*[=:]\s*['"]?[^\s'"]{6,}"#,
        // well-known token shapes
        r"\bsk-[A-Za-z0-9_-]{16,}\b",
        r"\bghp_[A-Za-z0-9]{20,}\b",
        r"\bgho_[A-Za-z0-9]{20,}\b",
        r"\bAKIA[0-9A-Z]{16}\b",
        r"(?i)\bbearer\s+[A-Za-z0-9._-]{16,}\b",
    ]
    .iter()
    .map(|p| Regex::new(p).expect("static regex"))
    .collect()
});

pub const REPLACEMENT: &str = "[REDACTED]";

pub fn redact(text: &str) -> String {
    let mut out = text.to_string();
    for re in PATTERNS.iter() {
        out = re.replace_all(&out, REPLACEMENT).into_owned();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_common_secret_shapes() {
        let input = "set API_KEY=abc123secret and use sk-abcdefghijklmnopqrstuvwx plus ghp_ABCDEFGHIJKLMNOPQRSt1234";
        let out = redact(input);
        assert!(!out.contains("abc123secret"), "{out}");
        assert!(!out.contains("sk-abcdefghijklmnop"), "{out}");
        assert!(!out.contains("ghp_"), "{out}");
        assert!(out.contains(REPLACEMENT));
    }

    #[test]
    fn leaves_normal_text_alone() {
        let input = "Use DB unique constraints instead of Redis locks.";
        assert_eq!(redact(input), input);
    }
}
