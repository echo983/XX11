use std::error::Error;
use std::fmt;
use std::collections::HashSet;

use crate::dsl::model::{Command, RenderEnvelope};

#[derive(Debug)]
struct ValidationError(String);

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Error for ValidationError {}

pub fn validate_render(render: &RenderEnvelope) -> Result<(), Box<dyn Error>> {
    if render.version != "AGD/0.1" {
        return Err(Box::new(ValidationError("unsupported version".to_string())));
    }
    if render.render_type != "render" {
        return Err(Box::new(ValidationError("unsupported type".to_string())));
    }
    if render.window.width == 0 || render.window.height == 0 {
        return Err(Box::new(ValidationError("window size must be positive".to_string())));
    }
    if render.window.title.trim().is_empty() {
        return Err(Box::new(ValidationError("window title must not be empty".to_string())));
    }
    if render.commands.is_empty() {
        return Err(Box::new(ValidationError("commands must not be empty".to_string())));
    }

    let mut has_clear = false;
    let mut ids = HashSet::new();
    for command in &render.commands {
        match command {
            Command::Clear { color } => {
                has_clear = true;
                validate_color(color, "clear.color")?;
            }
            Command::Rect { id, clickable, .. } => {
                if *clickable {
                    let id = id.as_ref().ok_or_else(|| {
                        Box::new(ValidationError("clickable rect requires id".to_string()))
                            as Box<dyn Error>
                    })?;
                    if id.trim().is_empty() {
                        return Err(Box::new(ValidationError("id must not be empty".to_string())));
                    }
                    if !ids.insert(id.clone()) {
                        return Err(Box::new(ValidationError("duplicate id".to_string())));
                    }
                } else if let Some(id) = id {
                    if id.trim().is_empty() {
                        return Err(Box::new(ValidationError("id must not be empty".to_string())));
                    }
                    if !ids.insert(id.clone()) {
                        return Err(Box::new(ValidationError("duplicate id".to_string())));
                    }
                }
                validate_rect(command)?;
            }
            Command::Text { text, color, .. } => {
                if text.trim().is_empty() {
                    continue;
                }
                if let Some(color) = color {
                    validate_color(color, "text.color")?;
                }
            }
            Command::Line { color, width, .. } => {
                if let Some(color) = color {
                    validate_color(color, "line.color")?;
                }
                if let Some(width) = width {
                    if *width == 0 {
                        return Err(Box::new(ValidationError("line.width must be positive".to_string())));
                    }
                }
            }
        }
    }

    if !has_clear {
        return Err(Box::new(ValidationError("commands must include clear".to_string())));
    }

    Ok(())
}

fn validate_rect(command: &Command) -> Result<(), Box<dyn Error>> {
    if let Command::Rect {
        w,
        h,
        fill,
        stroke,
        stroke_width,
        ..
    } = command
    {
        if *w == 0 || *h == 0 {
            return Err(Box::new(ValidationError("rect must have positive size".to_string())));
        }
        if let Some(fill) = fill {
            validate_color(fill, "rect.fill")?;
        }
        if let Some(stroke) = stroke {
            validate_color(stroke, "rect.stroke")?;
        }
        if let Some(stroke_width) = stroke_width {
            if *stroke_width == 0 {
                return Err(Box::new(ValidationError("rect.stroke_width must be positive".to_string())));
            }
        }
    }
    Ok(())
}

fn validate_color(value: &str, field: &str) -> Result<(), Box<dyn Error>> {
    if is_hex_color(value) {
        Ok(())
    } else {
        Err(Box::new(ValidationError(format!(
            "{field} must be #RRGGBB"
        ))))
    }
}

fn is_hex_color(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 7 || bytes[0] != b'#' {
        return false;
    }
    bytes[1..].iter().all(|b| match b {
        b'0'..=b'9' | b'a'..=b'f' | b'A'..=b'F' => true,
        _ => false,
    })
}
