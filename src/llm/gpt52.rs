use std::error::Error;
use reqwest::blocking::Client;
use serde_json::{json, Value};

const OPENAI_API_URL: &str = "https://api.openai.com/v1/responses";

pub enum LLMMode {
    Generate,
    Evaluate { image_base64: String, dsl_code: String },
}

pub fn request_render(
    event_json: Option<&str>,
    user_text: Option<&str>,
    mode: LLMMode,
) -> Result<String, Box<dyn Error>> {
    let api_key = std::env::var("OPENAI_API_KEY")?;
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .connect_timeout(std::time::Duration::from_secs(10))
        .build()?;

    let mut user_content = Vec::new();

    // 动态选择模型
    let model_name = match &mode {
        LLMMode::Generate => "gpt-5.2",
        LLMMode::Evaluate { .. } => "gpt-5-mini-2025-08-07",
    };

    match &mode {
        LLMMode::Generate => {
            let base_prompt = std::fs::read_to_string("prompts/generate.txt").unwrap_or_default();
            user_content.push(json!({ "type": "input_text", "text": base_prompt }));
            
            if let Some(event) = event_json {
                user_content.push(json!({ "type": "input_text", "text": format!("Event JSON:\n{}", event) }));
            } else if let Some(text) = user_text {
                user_content.push(json!({ "type": "input_text", "text": format!("User text:\n{}", text) }));
            } else {
                user_content.push(json!({ "type": "input_text", "text": "Initial request." }));
            };
        }
        LLMMode::Evaluate { image_base64, dsl_code } => {
            let base_prompt = std::fs::read_to_string("prompts/evaluate.txt").unwrap_or_default();
            user_content.push(json!({ "type": "input_text", "text": base_prompt }));
            user_content.push(json!({
                "type": "input_image",
                "image_url": format!("data:image/jpeg;base64,{}", image_base64)
            }));
            user_content.push(json!({ "type": "input_text", "text": format!("DSL CODE TO EVALUATE:\n{}", dsl_code) }));
        }
    }

    let schema = get_condensed_schema(&mode);
    let system_prompt = std::fs::read_to_string("prompts/system.txt").unwrap_or_else(|_| "You are a UI renderer.".to_string());

    let reasoning_effort = match &mode {
        LLMMode::Generate => "none",
        LLMMode::Evaluate { .. } => "minimal",
    };

    let mut payload_map = serde_json::Map::new();
    payload_map.insert("model".to_string(), json!(model_name));
    payload_map.insert("prompt_cache_key".to_string(), json!(format!("agd_v0.2_{}", model_name.replace('.', "_").replace('-', "_"))));
    
    // 仅为 gpt-5.2 开启 24h 缓存保留
    if model_name == "gpt-5.2" {
        payload_map.insert("prompt_cache_retention".to_string(), json!("24h"));
    }

    payload_map.insert("input".to_string(), json!([
        {
            "role": "system",
            "content": [{ "type": "input_text", "text": system_prompt }]
        },
        {
            "role": "user",
            "content": user_content
        }
    ]));

    payload_map.insert("text".to_string(), json!({
        "verbosity": "low",
        "format": {
            "type": "json_schema",
            "name": "gui_response",
            "strict": true,
            "schema": schema
        }
    }));

    payload_map.insert("reasoning".to_string(), json!({ "effort": reasoning_effort }));

    let payload = Value::Object(payload_map);

    let mut attempts = 0;
    let max_attempts = 3;

    loop {
        let response = client
            .post(OPENAI_API_URL)
            .bearer_auth(&api_key)
            .json(&payload)
            .send();

        match response {
            Ok(resp) => {
                if resp.status().is_success() {
                    let value: Value = resp.json()?;
                    
                    if std::env::var("AGD_DEBUG").map(|v| v == "1").unwrap_or(false) {
                        if let Some(usage) = value.get("usage") {
                            println!("[DEBUG] [{}] Raw Usage: {}", model_name, usage);
                            
                            let total = usage.get("total_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                            let input = usage.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                            let output = usage.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                            
                            let mut cached = 0;
                            if let Some(details) = usage.get("input_tokens_details") {
                                cached = details.get("cached_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                            }

                            println!("[DEBUG] [{}] Tokens: Total={}, Input={}, Output={}, Cached={}", 
                                     model_name, total, input, output, cached);
                        }
                    }

                    if let Some(output_text) = extract_output_text(&value) {
                        return Ok(output_text.trim().to_string());
                    } else {
                        // 如果提取失败，打印整个响应 body
                        println!("[ERROR] [{}] Failed to extract output text. Full response: {}", model_name, value);
                        return Err("missing output text from responses API".into());
                    }
                } else if resp.status().is_server_error() && attempts < max_attempts {
                    attempts += 1;
                    eprintln!("warn: HTTP {}, retrying (attempt {}/{})...", resp.status(), attempts, max_attempts);
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    continue;
                } else {
                    let status = resp.status();
                    let body = resp.text().unwrap_or_default();
                    return Err(format!("HTTP {}: {}\n", status, body).into());
                }
            }
            Err(e) if attempts < max_attempts => {
                attempts += 1;
                eprintln!("warn: Network error {}, retrying...", e);
                std::thread::sleep(std::time::Duration::from_secs(2));
                continue;
            }
            Err(e) => return Err(e.into()),
        }
    }
}

fn get_condensed_schema(mode: &LLMMode) -> Value {
    let xdsl_schema = json!({
        "type": "object",
        "properties": {
            "version": { "type": "string", "const": "X-DSL/0.2" }
        },
        "required": ["version"],
        "additionalProperties": false
    });

    let render_envelope_schema = json!({
        "type": "object",
        "properties": {
            "version": { "type": "string", "const": "AGD/0.2" },
            "type": { "type": "string", "const": "render" },
            "seq": { "type": "integer" },
            "window": {
                "type": "object",
                "properties": {
                    "width": { "type": "integer" },
                    "height": { "type": "integer" },
                    "title": { "type": "string" }
                },
                "required": ["width", "height", "title"],
                "additionalProperties": false
            },
            "commands": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "cmd": { "type": "string", "enum": ["clear", "rect", "text", "line", "circle", "ellipse", "round_rect", "arc", "polyline", "polygon", "image", "path"] },
                        "id": { "type": ["string", "null"] },
                        "x": { "type": ["integer", "null"] },
                        "y": { "type": ["integer", "null"] },
                        "w": { "type": ["integer", "null"] },
                        "h": { "type": ["integer", "null"] },
                        "cx": { "type": ["integer", "null"] },
                        "cy": { "type": ["integer", "null"] },
                        "r": { "type": ["integer", "null"] },
                        "rx": { "type": ["integer", "null"] },
                        "ry": { "type": ["integer", "null"] },
                        "start_angle": { "type": ["number", "null"] },
                        "end_angle": { "type": ["number", "null"] },
                        "x1": { "type": ["integer", "null"] },
                        "y1": { "type": ["integer", "null"] },
                        "x2": { "type": ["integer", "null"] },
                        "y2": { "type": ["integer", "null"] },
                        "points": {
                            "type": ["array", "null"],
                            "items": {
                                "type": "object",
                                "properties": {
                                    "x": { "type": "integer" },
                                    "y": { "type": "integer" }
                                },
                                "required": ["x", "y"],
                                "additionalProperties": false
                            }
                        },
                        "segments": {
                            "type": ["array", "null"],
                            "items": {
                                "type": "object",
                                "properties": {
                                    "cmd": { "type": "string", "enum": ["M", "L", "Z"] },
                                    "x": { "type": ["integer", "null"] },
                                    "y": { "type": ["integer", "null"] }
                                },
                                "required": ["cmd", "x", "y"],
                                "additionalProperties": false
                            }
                        },
                        "src_type": { "type": ["string", "null"], "enum": ["path", "base64", null] },
                        "src": { "type": ["string", "null"] },
                        "text": { "type": ["string", "null"] },
                        "color": { "type": ["string", "null"] },
                        "bg": { "type": ["string", "null"] },
                        "fill": { "type": ["string", "null"] },
                        "stroke": { "type": ["string", "null"] },
                        "stroke_width": { "type": ["integer", "null"] },
                        "width": { "type": ["integer", "null"] },
                        "clickable": { "type": "boolean" }
                    },
                    "required": [
                        "cmd", "id", "x", "y", "w", "h", "cx", "cy", "r", "rx", "ry",
                        "start_angle", "end_angle", "x1", "y1", "x2", "y2",
                        "points", "segments", "src_type", "src", "text", "color", "bg",
                        "fill", "stroke", "stroke_width", "width", "clickable"
                    ],
                    "additionalProperties": false
                }
            },
            "xdsl": {
                "anyOf": [
                    xdsl_schema,
                    { "type": "null" }
                ]
            }
        },
        "required": ["version", "type", "seq", "window", "commands", "xdsl"],
        "additionalProperties": false
    });

    match mode {
        LLMMode::Generate => render_envelope_schema,
        LLMMode::Evaluate { .. } => {
            json!({
                "type": "object",
                "properties": {
                    "is_final": { "type": "boolean" },
                    "rejection_reason": { "type": ["string", "null"] },
                    "render": render_envelope_schema
                },
                "required": ["is_final", "rejection_reason", "render"],
                "additionalProperties": false
            })
        }
    }
}

fn extract_output_text(value: &Value) -> Option<String> {
    let outputs = value.get("output")?.as_array()?;
    for item in outputs {
        if let Some(contents) = item.get("content").and_then(|v| v.as_array()) {
            for content in contents {
                let content_type = content.get("type").and_then(|v| v.as_str());
                
                // 处理正常文本输出
                if content_type == Some("output_text") || content_type == Some("text") {
                    if let Some(text) = content.get("text").and_then(|v| v.as_str()) {
                        return Some(text.to_string());
                    }
                }
                
                // 处理模型拒绝的情况
                if content_type == Some("refusal") {
                    if let Some(refusal) = content.get("refusal").and_then(|v| v.as_str()) {
                        println!("[WARN] Model refused to respond: {}", refusal);
                        return None;
                    }
                }
            }
        }
    }
    None
}
