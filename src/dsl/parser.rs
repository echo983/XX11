use std::error::Error;

use crate::dsl::model::RenderEnvelope;

pub fn parse_render(raw: &str) -> Result<RenderEnvelope, Box<dyn Error>> {
    let mut cleaned = raw.trim();
    if cleaned.starts_with("```") {
        if let Some(end) = cleaned.rfind("```") {
            let start = cleaned.find('\n').unwrap_or(0);
            if start < end {
                cleaned = &cleaned[start..end].trim();
            }
        }
    }
    let render: RenderEnvelope = serde_json::from_str(cleaned)?;
    Ok(render)
}
