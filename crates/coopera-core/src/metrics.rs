//! Local-only instrumentation (PRD "지표와 이벤트"): append-only JSONL under
//! `.coopera/cache/` so a human can answer "is the loop actually running?"
//! without archaeology. Never leaves the machine (no-telemetry NFR) and never
//! fails the caller.

use std::io::Write as _;
use std::path::{Path, PathBuf};

/// One rotation generation keeps the newest ~1MB of history.
const MAX_BYTES: u64 = 1024 * 1024;

pub fn metrics_path(repo_root: &Path) -> PathBuf {
    repo_root.join(".coopera/cache/metrics.jsonl")
}

/// Append one event line: `{"event": ..., "ts": ..., <fields>}`. Fail-open.
pub fn record(repo_root: &Path, event: &str, fields: &[(&str, serde_json::Value)]) {
    let path = metrics_path(repo_root);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(meta) = path.metadata() {
        if meta.len() > MAX_BYTES {
            let _ = std::fs::rename(&path, path.with_extension("jsonl.1"));
        }
    }
    let mut obj = serde_json::Map::new();
    obj.insert(
        "ts".to_string(),
        serde_json::Value::String(jiff::Timestamp::now().to_string()),
    );
    obj.insert(
        "event".to_string(),
        serde_json::Value::String(event.to_string()),
    );
    for (k, v) in fields {
        obj.insert((*k).to_string(), v.clone());
    }
    let Ok(line) = serde_json::to_string(&serde_json::Value::Object(obj)) else {
        return;
    };
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = writeln!(f, "{line}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_valid_jsonl_events() {
        let tmp = tempfile::tempdir().unwrap();
        record(
            tmp.path(),
            "inject",
            &[("items", 3.into()), ("tokens", 420.into())],
        );
        record(tmp.path(), "distill", &[("decisions", 2.into())]);

        let content = std::fs::read_to_string(metrics_path(tmp.path())).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);
        for line in &lines {
            let v: serde_json::Value = serde_json::from_str(line).unwrap();
            assert!(v["ts"].is_string(), "{line}");
            assert!(v["event"].is_string(), "{line}");
        }
        let first: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(first["event"], "inject");
        assert_eq!(first["items"], 3);
    }

    #[test]
    fn rotates_past_the_size_cap() {
        let tmp = tempfile::tempdir().unwrap();
        let path = metrics_path(tmp.path());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "x".repeat((MAX_BYTES + 10) as usize)).unwrap();
        record(tmp.path(), "inject", &[]);
        assert!(path.with_extension("jsonl.1").exists(), "rotated file");
        let fresh = std::fs::read_to_string(&path).unwrap();
        assert!(fresh.len() < 1024, "new file starts small: {}", fresh.len());
        assert!(fresh.contains("\"event\":\"inject\""));
    }
}
