use serde::{Deserialize, Serialize};

/// Hook input (stdin JSON). Field names follow Claude Code's hook payload;
/// Codex uses the same vocabulary (verified in spike ③), and Antigravity's
/// camelCase payload (protojson) maps onto the same fields via serde aliases
/// (contract from the embedded agy hooks guide, 2026-08-02). All fields
/// optional — a hook must never fail because a field is missing (fail-open).
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct HookInput {
    #[serde(alias = "conversationId")]
    pub session_id: Option<String>,
    #[serde(alias = "transcriptPath")]
    pub transcript_path: Option<String>,
    pub cwd: Option<String>,
    pub hook_event_name: Option<String>,
    pub source: Option<String>,
    pub prompt: Option<String>,
    /// Antigravity PreInvocation: 1-based model-call counter within a
    /// conversation — the injection pack goes in on the first invocation.
    #[serde(alias = "invocationNum")]
    pub invocation_num: Option<u64>,
    /// Antigravity: workspace roots (used as cwd fallback — agy payloads
    /// carry no `cwd`).
    #[serde(alias = "workspacePaths")]
    pub workspace_paths: Vec<String>,
}

/// Antigravity PreInvocation reply: inject team context as a transient
/// system message. Empty text serializes to `{}` (no-op per the contract).
pub fn agy_pre_invocation_output(ephemeral: &str) -> String {
    if ephemeral.trim().is_empty() {
        return "{}".to_string();
    }
    serde_json::json!({
        "injectSteps": [{ "ephemeralMessage": ephemeral }]
    })
    .to_string()
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
    pub fn for_event(event: &str, context: String, message: Option<String>) -> Self {
        Self {
            hook_specific_output: HookSpecificOutput {
                hook_event_name: event.to_string(),
                additional_context: context,
            },
            system_message: message,
        }
    }

    pub fn session_start(context: String, message: Option<String>) -> Self {
        Self::for_event("SessionStart", context, message)
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
    fn antigravity_camel_case_payload_maps_onto_the_same_fields() {
        let input = HookInput::from_stdin_str(
            r#"{"conversationId":"conv-1","transcriptPath":"/t.jsonl","invocationNum":3,"workspacePaths":["/ws"],"modelName":"auto"}"#,
        );
        assert_eq!(input.session_id.as_deref(), Some("conv-1"));
        assert_eq!(input.transcript_path.as_deref(), Some("/t.jsonl"));
        assert_eq!(input.invocation_num, Some(3));
        assert_eq!(input.workspace_paths, vec!["/ws"]);
    }

    #[test]
    fn agy_output_injects_or_noops() {
        assert_eq!(agy_pre_invocation_output(""), "{}");
        let out = agy_pre_invocation_output("team ctx");
        assert!(out.contains("injectSteps") && out.contains("ephemeralMessage"));
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
