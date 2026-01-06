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
    if render.version != "AGD/0.2" {
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
            Command::Circle { cx, cy, r, fill, stroke, stroke_width } => {
                require_i32(cx, "circle.cx")?;
                require_i32(cy, "circle.cy")?;
                require_u32(r, "circle.r")?;
                validate_fill_stroke(fill, stroke, stroke_width, "circle")?;
            }
            Command::Ellipse { cx, cy, rx, ry, fill, stroke, stroke_width } => {
                require_i32(cx, "ellipse.cx")?;
                require_i32(cy, "ellipse.cy")?;
                require_u32(rx, "ellipse.rx")?;
                require_u32(ry, "ellipse.ry")?;
                validate_fill_stroke(fill, stroke, stroke_width, "ellipse")?;
            }
            Command::RoundRect { x, y, w, h, r, fill, stroke, stroke_width } => {
                require_i32(x, "round_rect.x")?;
                require_i32(y, "round_rect.y")?;
                require_u32(w, "round_rect.w")?;
                require_u32(h, "round_rect.h")?;
                require_u32(r, "round_rect.r")?;
                validate_fill_stroke(fill, stroke, stroke_width, "round_rect")?;
            }
            Command::Arc { cx, cy, r, start_angle, end_angle, color, width } => {
                require_i32(cx, "arc.cx")?;
                require_i32(cy, "arc.cy")?;
                require_u32(r, "arc.r")?;
                require_f32(start_angle, "arc.start_angle")?;
                require_f32(end_angle, "arc.end_angle")?;
                if let Some(color) = color {
                    validate_color(color, "arc.color")?;
                }
                if let Some(width) = width {
                    if *width == 0 {
                        return Err(Box::new(ValidationError("arc.width must be positive".to_string())));
                    }
                }
            }
            Command::Polyline { points, color, width } => {
                validate_points(points, "polyline.points", 2)?;
                if let Some(color) = color {
                    validate_color(color, "polyline.color")?;
                }
                if let Some(width) = width {
                    if *width == 0 {
                        return Err(Box::new(ValidationError("polyline.width must be positive".to_string())));
                    }
                }
            }
            Command::Polygon { points, fill, stroke, stroke_width } => {
                validate_points(points, "polygon.points", 3)?;
                validate_fill_stroke(fill, stroke, stroke_width, "polygon")?;
            }
            Command::Image { x, y, w, h, src_type, src } => {
                require_i32(x, "image.x")?;
                require_i32(y, "image.y")?;
                require_u32(w, "image.w")?;
                require_u32(h, "image.h")?;
                let src_type = src_type.as_deref().ok_or_else(|| {
                    Box::new(ValidationError("image.src_type is required".to_string())) as Box<dyn Error>
                })?;
                if src_type != "path" && src_type != "base64" {
                    return Err(Box::new(ValidationError("image.src_type must be path|base64".to_string())));
                }
                let src = src.as_deref().ok_or_else(|| {
                    Box::new(ValidationError("image.src is required".to_string())) as Box<dyn Error>
                })?;
                if src.trim().is_empty() {
                    return Err(Box::new(ValidationError("image.src must not be empty".to_string())));
                }
            }
            Command::Path { segments, fill, stroke, stroke_width } => {
                validate_segments(segments, "path.segments")?;
                validate_fill_stroke(fill, stroke, stroke_width, "path")?;
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

fn require_i32(value: &Option<i32>, field: &str) -> Result<i32, Box<dyn Error>> {
    value.ok_or_else(|| Box::new(ValidationError(format!("{field} is required"))) as Box<dyn Error>)
}

fn require_u32(value: &Option<u32>, field: &str) -> Result<u32, Box<dyn Error>> {
    let v = value.ok_or_else(|| Box::new(ValidationError(format!("{field} is required"))) as Box<dyn Error>)?;
    if v == 0 {
        return Err(Box::new(ValidationError(format!("{field} must be positive"))));
    }
    Ok(v)
}

fn require_f32(value: &Option<f32>, field: &str) -> Result<f32, Box<dyn Error>> {
    value.ok_or_else(|| Box::new(ValidationError(format!("{field} is required"))) as Box<dyn Error>)
}

fn validate_fill_stroke(
    fill: &Option<String>,
    stroke: &Option<String>,
    stroke_width: &Option<u32>,
    prefix: &str,
) -> Result<(), Box<dyn Error>> {
    if let Some(fill) = fill {
        validate_color(fill, &format!("{prefix}.fill"))?;
    }
    if let Some(stroke) = stroke {
        validate_color(stroke, &format!("{prefix}.stroke"))?;
    }
    if let Some(width) = stroke_width {
        if *width == 0 {
            return Err(Box::new(ValidationError(format!("{prefix}.stroke_width must be positive"))));
        }
    }
    if fill.is_none() && stroke.is_none() {
        return Err(Box::new(ValidationError(format!("{prefix} must have fill or stroke"))));
    }
    Ok(())
}

fn validate_points(points: &Option<Vec<crate::dsl::model::Point>>, field: &str, min_len: usize) -> Result<(), Box<dyn Error>> {
    let points = points.as_ref().ok_or_else(|| {
        Box::new(ValidationError(format!("{field} is required"))) as Box<dyn Error>
    })?;
    if points.len() < min_len {
        return Err(Box::new(ValidationError(format!("{field} must have at least {min_len} points"))));
    }
    Ok(())
}

fn validate_segments(
    segments: &Option<Vec<crate::dsl::model::PathSegment>>,
    field: &str,
) -> Result<(), Box<dyn Error>> {
    let segments = segments.as_ref().ok_or_else(|| {
        Box::new(ValidationError(format!("{field} is required"))) as Box<dyn Error>
    })?;
    if segments.is_empty() {
        return Err(Box::new(ValidationError(format!("{field} must not be empty"))));
    }
    let mut has_move = false;
    for seg in segments {
        match seg.cmd.as_str() {
            "M" | "L" => {
                if seg.x.is_none() || seg.y.is_none() {
                    return Err(Box::new(ValidationError(format!("{field} M/L must include x,y"))));
                }
                has_move = true;
            }
            "Z" => {}
            _ => return Err(Box::new(ValidationError(format!("{field} cmd must be M|L|Z")))),
        }
    }
    if !has_move {
        return Err(Box::new(ValidationError(format!("{field} must include M"))));
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
