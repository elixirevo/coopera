use serde::{Deserialize, Serialize};

/// Hook input (stdin JSON). Field names follow Claude Code's hook payload;
/// Codex uses the same vocabulary (verified in spike ③). All fields optional —
/// a hook must never fail because a field is missing (fail-open).
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct HookInput {
    pub session_id: Option<String>,
    pub transcript_path: Option<String>,
    pub cwd: Option<String>,
    pub hook_event_name: Option<String>,
    pub source: Option<String>,
    pub prompt: Option<String>,
}

impl HookInput {
    /// Parse leniently: empty or invalid stdin yields defaults.
    pub fn from_stdin_str(s: &str) -> HookInput {
        serde_json::from_str(s).unwrap_or_default()
    }
}

#[derive(Debug, Serialize)]
pub struct HookSpecificOutput {
    #[serde(rename = "hookEventName")]
    pub hook_event_name: String,
    #[serde(rename = "additionalContext", skip_serializing_if = "String::is_empty")]
    pub additional_context: String,
}

/// Hook output (stdout JSON) — `additionalContext` goes into model context,
/// `systemMessage` is the one-line human-visible trace (visibility principle).
#[derive(Debug, Serialize)]
pub struct HookOutput {
    #[serde(rename = "hookSpecificOutput")]
    pub hook_specific_output: HookSpecificOutput,
    #[serde(rename = "systemMessage", skip_serializing_if = "Option::is_none")]
    pub system_message: Option<String>,
}

impl HookOutput {
    pub fn session_start(context: String, message: Option<String>) -> Self {
        Self {
            hook_specific_output: HookSpecificOutput {
                hook_event_name: "SessionStart".to_string(),
                additional_context: context,
            },
            system_message: message,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lenient_parse_of_empty_input() {
        let input = HookInput::from_stdin_str("");
        assert!(input.session_id.is_none());
        let input = HookInput::from_stdin_str("{\"session_id\":\"s1\",\"unknown_field\":1}");
        assert_eq!(input.session_id.as_deref(), Some("s1"));
    }

    #[test]
    fn output_serializes_with_camel_case_keys() {
        let out = HookOutput::session_start("CTX".into(), Some("coopera: injected 1 item".into()));
        let json = serde_json::to_string(&out).unwrap();
        assert!(json.contains("hookSpecificOutput"));
        assert!(json.contains("additionalContext"));
        assert!(json.contains("systemMessage"));
    }
}
