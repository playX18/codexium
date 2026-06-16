use serde_json::Value;
use serde_json::json;

/// Detect OpenAI o-series reasoning models (o1, o3, o4-mini, etc.)
pub fn is_openai_o_series(model: &str) -> bool {
    model.len() > 1
        && model.starts_with('o')
        && model.as_bytes().get(1).is_some_and(u8::is_ascii_digit)
}

/// Detect OpenAI models that support `reasoning_effort`.
pub fn supports_reasoning_effort(model: &str) -> bool {
    is_openai_o_series(model)
        || model
            .to_lowercase()
            .strip_prefix("gpt-")
            .and_then(|rest| rest.chars().next())
            .is_some_and(|c| c.is_ascii_digit() && c >= '5')
}

/// Ensure streaming chat requests include usage in the final chunk.
pub fn inject_openai_stream_include_usage(result: &mut Value) {
    let is_stream = result
        .get("stream")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    if !is_stream {
        return;
    }
    match result.get_mut("stream_options") {
        Some(Value::Object(opts)) => {
            opts.insert("include_usage".to_string(), json!(true));
        }
        _ => {
            result["stream_options"] = json!({ "include_usage": true });
        }
    }
}
