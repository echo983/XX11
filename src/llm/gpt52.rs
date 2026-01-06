use std::error::Error;

use reqwest::blocking::Client;
use serde_json::{json, Value};

const OPENAI_API_URL: &str = "https://api.openai.com/v1/responses";

const DEFAULT_SYSTEM_PROMPT: &str = "You are a renderer that outputs JSON only. \
Use AGD/0.1 render envelopes exactly. \
Output must be a single JSON object (no markdown, no commentary). \
Rules: version=\"AGD/0.1\", type=\"render\", include seq (u64), \
include window {width,height,title}, and commands array. \
Commands allowed: clear, rect, text, line. \
Every render must include clear. \
clickable rects must have unique id. \
Use explicit coordinates, no layout inference. \
When user text is provided, respond with a simple UI answer; do not echo the user text unless necessary.";

pub fn request_render(
    event_json: Option<&str>,
    user_text: Option<&str>,
) -> Result<String, Box<dyn Error>> {
    let api_key = std::env::var("OPENAI_API_KEY")?;
    let client = Client::new();

    let user = if let Some(event) = event_json {
        format!(
            "Event JSON:\\n{}\\n\\nReturn the next render JSON.",
            event
        )
    } else if let Some(text) = user_text {
        format!(
            "User text:\\n{}\\n\\nRespond with a simple UI answer (do not echo the user text).",
            text
        )
    } else {
        "Return the initial render JSON.".to_string()
    };
    let system_prompt = load_system_prompt();

    let payload = json!({
        "model": "gpt-5.2",
        "input": [
            {
                "role": "system",
                "content": [{"type": "input_text", "text": system_prompt}]
            },
            {
                "role": "user",
                "content": [{"type": "input_text", "text": user}]
            }
        ],
        "reasoning": { "effort": "none" },
        "text": { "verbosity": "low" }
    });

    let response = client
        .post(OPENAI_API_URL)
        .bearer_auth(api_key)
        .json(&payload)
        .send()?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("HTTP {status}: {body}").into());
    }

    let value: Value = response.json()?;
    let output = extract_output_text(&value)
        .ok_or_else(|| "missing output text from responses API")?;
    Ok(output.trim().to_string())
}

fn load_system_prompt() -> String {
    std::fs::read_to_string("prompts/system.txt").unwrap_or_else(|_| DEFAULT_SYSTEM_PROMPT.to_string())
}

fn extract_output_text(value: &Value) -> Option<String> {
    let outputs = value.get("output")?.as_array()?;
    for item in outputs {
        let contents = item.get("content")?.as_array()?;
        for content in contents {
            let content_type = content.get("type").and_then(|v| v.as_str());
            if content_type == Some("output_text") || content_type == Some("text") {
                if let Some(text) = content.get("text").and_then(|v| v.as_str()) {
                    return Some(text.to_string());
                }
            }
        }
    }
    None
}
