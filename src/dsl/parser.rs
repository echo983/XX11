use std::error::Error;
use crate::dsl::model::RenderEnvelope;

pub fn parse_render(raw: &str) -> Result<RenderEnvelope, Box<dyn Error>> {
    let mut cleaned = raw.trim();
    
    // 尝试寻找第一个 { 和最后一个 } 之间的内容，这能过滤掉前后多余的解释文本
    if let (Some(start), Some(end)) = (cleaned.find('{'), cleaned.rfind('}')) {
        cleaned = &cleaned[start..=end];
    } else {
        return Err(format!("No JSON object found in LLM output: {}", raw).into());
    }

    let render: RenderEnvelope = serde_json::from_str(cleaned).map_err(|e| {
        let snippet = if cleaned.len() > 100 { &cleaned[..100] } else { cleaned };
        format!("JSON parse error: {} | Content snippet: {}", e, snippet)
    })?;
    Ok(render)
}