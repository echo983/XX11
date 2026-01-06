use std::error::Error;
use base64::{Engine as _, engine::general_purpose};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    Char2b, ConnectionExt, ImageFormat,
};
use crate::dsl::model::{Command, Point, PathSegment, RenderEnvelope};
use crate::x11::backend::X11Backend;

/// 渲染一帧到 X11 窗口
pub fn render_frame(backend: &X11Backend, render: &RenderEnvelope) -> Result<(), Box<dyn Error>> {
    let conn = backend.connection();
    let window = backend.window();
    let gc = backend.gc();

    // 我们先在内存中生成完整的位图，然后一次性发给 X11，这样可以保持显示和“草稿截图”完全一致
    let (width, height, pixels) = render_to_buffer(render, backend.font_primary(), backend.font_emoji())?;

    conn.put_image(
        ImageFormat::Z_PIXMAP,
        window,
        gc,
        width as u16,
        height as u16,
        0,
        0,
        0,
        backend.depth(),
        &pixels,
    )?;

    Connection::flush(conn)?;
    Ok(())
}

pub fn render_frame_with_press(
    backend: &X11Backend,
    render: &RenderEnvelope,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
) -> Result<(), Box<dyn Error>> {
    let conn = backend.connection();
    let window = backend.window();
    let gc = backend.gc();
    let (width, height, mut pixels) = render_to_buffer(render, backend.font_primary(), backend.font_emoji())?;

    // Local-only pressed feedback: emphasize the clicked rect with a bold outline.
    let press_color = (32u8, 32u8, 32u8);
    let press_thickness = 2u32;
    draw_rect_outline(&mut pixels, width, height, x, y, w, h, press_color, press_thickness);

    conn.put_image(
        ImageFormat::Z_PIXMAP,
        window,
        gc,
        width as u16,
        height as u16,
        0,
        0,
        0,
        backend.depth(),
        &pixels,
    )?;

    Connection::flush(conn)?;
    Ok(())
}

/// 核心逻辑：将所有指令渲染到一个像素缓冲区 (RGBA/BGRA)
pub fn render_to_buffer(
    render: &RenderEnvelope,
    primary: Option<&fontdue::Font>,
    emoji: Option<&fontdue::Font>,
) -> Result<(usize, usize, Vec<u8>), Box<dyn Error>> {
    let width = render.window.width as usize;
    let height = render.window.height as usize;
    let mut pixels = vec![0u8; width * height * 4];

    // 默认背景色（通常第一个指令是 Clear，但这里做个兜底）
    fill_rect(&mut pixels, width, height, 0, 0, width as u32, height as u32, (255, 255, 255));

    for command in &render.commands {
        match command {
            Command::Clear { color } => {
                let rgb = parse_rgb(color)?;
                fill_rect(&mut pixels, width, height, 0, 0, width as u32, height as u32, rgb_tuple(rgb));
            }
            Command::Rect { x, y, w, h, fill, stroke, stroke_width, .. } => {
                if let Some(fill_color) = fill {
                    let rgb = parse_rgb(fill_color)?;
                    fill_rect(&mut pixels, width, height, *x, *y, *w, *h, rgb_tuple(rgb));
                }
                if let Some(stroke_color) = stroke {
                    let rgb = parse_rgb(stroke_color)?;
                    let thickness = stroke_width.unwrap_or(1);
                    draw_rect_outline(&mut pixels, width, height, *x, *y, *w, *h, rgb_tuple(rgb), thickness);
                }
            }
            Command::Text { x, y, text, color, bg } => {
                if let Some(font) = primary {
                    let fg_rgb = rgb_tuple(parse_rgb(color.as_deref().unwrap_or("#000000"))?);
                    let bg_rgb = if let Some(bg_str) = bg {
                        Some(rgb_tuple(parse_rgb(bg_str)?))
                    } else {
                        None
                    };
                    draw_text(&mut pixels, width, height, *x, *y, text, fg_rgb, bg_rgb, font, emoji);
                }
            }
            Command::Line { x1, y1, x2, y2, color, width: line_width } => {
                let rgb = rgb_tuple(parse_rgb(color.as_deref().unwrap_or("#000000"))?);
                let thickness = line_width.unwrap_or(1);
                draw_line(&mut pixels, width, height, *x1, *y1, *x2, *y2, rgb, thickness);
            }
            Command::Circle { cx, cy, r, fill, stroke, stroke_width } => {
                if let (Some(cx), Some(cy), Some(r)) = (cx, cy, r) {
                    if let Some(fill_color) = fill {
                        let rgb = rgb_tuple(parse_rgb(fill_color)?);
                        fill_circle(&mut pixels, width, height, *cx, *cy, *r as i32, rgb);
                    }
                    if let Some(stroke_color) = stroke {
                        let rgb = rgb_tuple(parse_rgb(stroke_color)?);
                        let thickness = stroke_width.unwrap_or(1);
                        draw_circle_outline(&mut pixels, width, height, *cx, *cy, *r as i32, rgb, thickness);
                    }
                }
            }
            Command::Ellipse { cx, cy, rx, ry, fill, stroke, stroke_width } => {
                if let (Some(cx), Some(cy), Some(rx), Some(ry)) = (cx, cy, rx, ry) {
                    if let Some(fill_color) = fill {
                        let rgb = rgb_tuple(parse_rgb(fill_color)?);
                        fill_ellipse(&mut pixels, width, height, *cx, *cy, *rx as i32, *ry as i32, rgb);
                    }
                    if let Some(stroke_color) = stroke {
                        let rgb = rgb_tuple(parse_rgb(stroke_color)?);
                        let thickness = stroke_width.unwrap_or(1);
                        draw_ellipse_outline(&mut pixels, width, height, *cx, *cy, *rx as i32, *ry as i32, rgb, thickness);
                    }
                }
            }
            Command::RoundRect { x, y, w, h, r, fill, stroke, stroke_width } => {
                if let (Some(x), Some(y), Some(w), Some(h), Some(r)) = (x, y, w, h, r) {
                    if let Some(fill_color) = fill {
                        let rgb = rgb_tuple(parse_rgb(fill_color)?);
                        fill_round_rect(&mut pixels, width, height, *x, *y, *w, *h, *r, rgb);
                    }
                    if let Some(stroke_color) = stroke {
                        let rgb = rgb_tuple(parse_rgb(stroke_color)?);
                        let thickness = stroke_width.unwrap_or(1);
                        draw_round_rect_outline(&mut pixels, width, height, *x, *y, *w, *h, *r, rgb, thickness);
                    }
                }
            }
            Command::Arc { cx, cy, r, start_angle, end_angle, color, width: line_width } => {
                if let (Some(cx), Some(cy), Some(r), Some(start), Some(end)) = (cx, cy, r, start_angle, end_angle) {
                    let rgb = rgb_tuple(parse_rgb(color.as_deref().unwrap_or("#000000"))?);
                    let thickness = line_width.unwrap_or(1);
                    draw_arc(&mut pixels, width, height, *cx, *cy, *r as i32, *start, *end, rgb, thickness);
                }
            }
            Command::Polyline { points, color, width: line_width } => {
                if let Some(points) = points {
                    let rgb = rgb_tuple(parse_rgb(color.as_deref().unwrap_or("#000000"))?);
                    let thickness = line_width.unwrap_or(1);
                    draw_polyline(&mut pixels, width, height, points, rgb, thickness);
                }
            }
            Command::Polygon { points, fill, stroke, stroke_width } => {
                if let Some(points) = points {
                    if let Some(fill_color) = fill {
                        let rgb = rgb_tuple(parse_rgb(fill_color)?);
                        fill_polygon(&mut pixels, width, height, points, rgb);
                    }
                    if let Some(stroke_color) = stroke {
                        let rgb = rgb_tuple(parse_rgb(stroke_color)?);
                        let thickness = stroke_width.unwrap_or(1);
                        draw_polyline_closed(&mut pixels, width, height, points, rgb, thickness);
                    }
                }
            }
            Command::Image { x, y, w, h, src_type, src } => {
                if let (Some(x), Some(y), Some(w), Some(h), Some(src_type), Some(src)) = (x, y, w, h, src_type, src) {
                    draw_image(&mut pixels, width, height, *x, *y, *w, *h, src_type, src)?;
                }
            }
            Command::Path { segments, fill, stroke, stroke_width } => {
                if let Some(segments) = segments {
                    let subpaths = segments_to_subpaths(segments);
                    if let Some(fill_color) = fill {
                        let rgb = rgb_tuple(parse_rgb(fill_color)?);
                        for path in &subpaths {
                            if path.len() >= 3 {
                                fill_polygon(&mut pixels, width, height, path, rgb);
                            }
                        }
                    }
                    if let Some(stroke_color) = stroke {
                        let rgb = rgb_tuple(parse_rgb(stroke_color)?);
                        let thickness = stroke_width.unwrap_or(1);
                        for path in &subpaths {
                            if path.len() >= 2 {
                                draw_polyline(&mut pixels, width, height, path, rgb, thickness);
                            }
                        }
                    }
                }
            }
        }
    }

    Ok((width, height, pixels))
}

// --- 基础绘图辅助函数 ---

fn fill_rect(p: &mut [u8], pw: usize, ph: usize, x: i32, y: i32, w: u32, h: u32, rgb: (u8, u8, u8)) {
    for iy in y..(y + h as i32) {
        for ix in x..(x + w as i32) {
            if ix >= 0 && ix < pw as i32 && iy >= 0 && iy < ph as i32 {
                let idx = (iy as usize * pw + ix as usize) * 4;
                p[idx] = rgb.2;     // B
                p[idx + 1] = rgb.1; // G
                p[idx + 2] = rgb.0; // R
                p[idx + 3] = 0;     // Alpha
            }
        }
    }
}

fn draw_rect_outline(p: &mut [u8], pw: usize, ph: usize, x: i32, y: i32, w: u32, h: u32, rgb: (u8, u8, u8), t: u32) {
    for i in 0..t as i32 {
        draw_line(p, pw, ph, x, y + i, x + w as i32, y + i, rgb, 1); // Top
        draw_line(p, pw, ph, x, y + h as i32 - 1 - i, x + w as i32, y + h as i32 - 1 - i, rgb, 1); // Bottom
        draw_line(p, pw, ph, x + i, y, x + i, y + h as i32, rgb, 1); // Left
        draw_line(p, pw, ph, x + w as i32 - 1 - i, y, x + w as i32 - 1 - i, y + h as i32, rgb, 1); // Right
    }
}

fn draw_line(p: &mut [u8], pw: usize, ph: usize, x1: i32, y1: i32, x2: i32, y2: i32, rgb: (u8, u8, u8), t: u32) {
    let thickness = t.max(1) as i32;
    let half = thickness / 2;
    let mut x = x1;
    let mut y = y1;
    let dx = (x2 - x1).abs();
    let dy = -(y2 - y1).abs();
    let sx = if x1 < x2 { 1 } else { -1 };
    let sy = if y1 < y2 { 1 } else { -1 };
    let mut err = dx + dy;
    loop {
        fill_rect(p, pw, ph, x - half, y - half, thickness as u32, thickness as u32, rgb);
        if x == x2 && y == y2 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

fn draw_polyline(p: &mut [u8], pw: usize, ph: usize, points: &[Point], rgb: (u8, u8, u8), t: u32) {
    for pair in points.windows(2) {
        draw_line(p, pw, ph, pair[0].x, pair[0].y, pair[1].x, pair[1].y, rgb, t);
    }
}

fn draw_polyline_closed(p: &mut [u8], pw: usize, ph: usize, points: &[Point], rgb: (u8, u8, u8), t: u32) {
    if points.len() < 2 {
        return;
    }
    draw_polyline(p, pw, ph, points, rgb, t);
    let first = &points[0];
    let last = &points[points.len() - 1];
    draw_line(p, pw, ph, last.x, last.y, first.x, first.y, rgb, t);
}

fn fill_polygon(p: &mut [u8], pw: usize, ph: usize, points: &[Point], rgb: (u8, u8, u8)) {
    if points.len() < 3 {
        return;
    }
    let min_y = points.iter().map(|pt| pt.y).min().unwrap_or(0);
    let max_y = points.iter().map(|pt| pt.y).max().unwrap_or(0);
    for y in min_y..=max_y {
        let mut intersections = Vec::new();
        for i in 0..points.len() {
            let p1 = &points[i];
            let p2 = &points[(i + 1) % points.len()];
            if p1.y == p2.y {
                continue;
            }
            let (y1, y2) = (p1.y, p2.y);
            if (y >= y1 && y < y2) || (y >= y2 && y < y1) {
                let t = (y - y1) as f32 / (y2 - y1) as f32;
                let x = p1.x as f32 + t * (p2.x - p1.x) as f32;
                intersections.push(x.round() as i32);
            }
        }
        intersections.sort_unstable();
        for pair in intersections.chunks(2) {
            if pair.len() == 2 {
                let x_start = pair[0].min(pair[1]);
                let x_end = pair[0].max(pair[1]);
                if x_end >= x_start {
                    fill_rect(p, pw, ph, x_start, y, (x_end - x_start + 1) as u32, 1, rgb);
                }
            }
        }
    }
}

fn draw_circle_outline(p: &mut [u8], pw: usize, ph: usize, cx: i32, cy: i32, r: i32, rgb: (u8, u8, u8), t: u32) {
    draw_arc(p, pw, ph, cx, cy, r, 0.0, 360.0, rgb, t);
}

fn fill_circle(p: &mut [u8], pw: usize, ph: usize, cx: i32, cy: i32, r: i32, rgb: (u8, u8, u8)) {
    let r2 = (r * r) as f32;
    for dy in -r..=r {
        let y = cy + dy;
        let dx = (r2 - (dy * dy) as f32).sqrt() as i32;
        fill_rect(p, pw, ph, cx - dx, y, (dx * 2 + 1) as u32, 1, rgb);
    }
}

fn draw_ellipse_outline(p: &mut [u8], pw: usize, ph: usize, cx: i32, cy: i32, rx: i32, ry: i32, rgb: (u8, u8, u8), t: u32) {
    let mut angle: f32 = 0.0;
    let step = 2.0_f32;
    let mut prev = None;
    while angle <= 360.0 {
        let rad = angle.to_radians();
        let x = cx + (rx as f32 * rad.cos()).round() as i32;
        let y = cy + (ry as f32 * rad.sin()).round() as i32;
        if let Some((px, py)) = prev {
            draw_line(p, pw, ph, px, py, x, y, rgb, t);
        }
        prev = Some((x, y));
        angle += step;
    }
}

fn fill_ellipse(p: &mut [u8], pw: usize, ph: usize, cx: i32, cy: i32, rx: i32, ry: i32, rgb: (u8, u8, u8)) {
    let rx2 = (rx * rx) as f32;
    let ry2 = (ry * ry) as f32;
    for dy in -ry..=ry {
        let y = cy + dy;
        let term = 1.0 - (dy * dy) as f32 / ry2;
        if term < 0.0 {
            continue;
        }
        let dx = (rx2 * term).sqrt() as i32;
        fill_rect(p, pw, ph, cx - dx, y, (dx * 2 + 1) as u32, 1, rgb);
    }
}

fn fill_round_rect(p: &mut [u8], pw: usize, ph: usize, x: i32, y: i32, w: u32, h: u32, r: u32, rgb: (u8, u8, u8)) {
    let r = r.min((w.min(h) / 2) as u32) as i32;
    let w_i = w as i32;
    let h_i = h as i32;
    fill_rect(p, pw, ph, x + r, y, (w_i - 2 * r).max(0) as u32, h, rgb);
    fill_rect(p, pw, ph, x, y + r, r as u32, (h_i - 2 * r).max(0) as u32, rgb);
    fill_rect(p, pw, ph, x + w_i - r, y + r, r as u32, (h_i - 2 * r).max(0) as u32, rgb);
    fill_circle_quadrant(p, pw, ph, x + r, y + r, r, rgb, -1, -1);
    fill_circle_quadrant(p, pw, ph, x + w_i - r - 1, y + r, r, rgb, 1, -1);
    fill_circle_quadrant(p, pw, ph, x + r, y + h_i - r - 1, r, rgb, -1, 1);
    fill_circle_quadrant(p, pw, ph, x + w_i - r - 1, y + h_i - r - 1, r, rgb, 1, 1);
}

fn draw_round_rect_outline(p: &mut [u8], pw: usize, ph: usize, x: i32, y: i32, w: u32, h: u32, r: u32, rgb: (u8, u8, u8), t: u32) {
    let r = r.min((w.min(h) / 2) as u32) as i32;
    let w_i = w as i32;
    let h_i = h as i32;
    draw_line(p, pw, ph, x + r, y, x + w_i - r - 1, y, rgb, t);
    draw_line(p, pw, ph, x + r, y + h_i - 1, x + w_i - r - 1, y + h_i - 1, rgb, t);
    draw_line(p, pw, ph, x, y + r, x, y + h_i - r - 1, rgb, t);
    draw_line(p, pw, ph, x + w_i - 1, y + r, x + w_i - 1, y + h_i - r - 1, rgb, t);
    draw_arc(p, pw, ph, x + r, y + r, r, 180.0, 270.0, rgb, t);
    draw_arc(p, pw, ph, x + w_i - r - 1, y + r, r, 270.0, 360.0, rgb, t);
    draw_arc(p, pw, ph, x + w_i - r - 1, y + h_i - r - 1, r, 0.0, 90.0, rgb, t);
    draw_arc(p, pw, ph, x + r, y + h_i - r - 1, r, 90.0, 180.0, rgb, t);
}

fn fill_circle_quadrant(
    p: &mut [u8],
    pw: usize,
    ph: usize,
    cx: i32,
    cy: i32,
    r: i32,
    rgb: (u8, u8, u8),
    sx: i32,
    sy: i32,
) {
    let r2 = (r * r) as f32;
    for dy in 0..=r {
        let dx = (r2 - (dy * dy) as f32).sqrt() as i32;
        let y = cy + sy * dy;
        let x_start = if sx < 0 { cx - dx } else { cx };
        let width = dx + 1;
        fill_rect(p, pw, ph, x_start, y, width as u32, 1, rgb);
    }
}

fn draw_arc(p: &mut [u8], pw: usize, ph: usize, cx: i32, cy: i32, r: i32, start_deg: f32, end_deg: f32, rgb: (u8, u8, u8), t: u32) {
    let mut angle = start_deg;
    let step = if end_deg >= start_deg { 1.0 } else { -1.0 };
    let mut prev = None;
    while (step > 0.0 && angle <= end_deg) || (step < 0.0 && angle >= end_deg) {
        let rad = angle.to_radians();
        let x = cx + (r as f32 * rad.cos()).round() as i32;
        let y = cy + (r as f32 * rad.sin()).round() as i32;
        if let Some((px, py)) = prev {
            draw_line(p, pw, ph, px, py, x, y, rgb, t);
        }
        prev = Some((x, y));
        angle += step;
    }
}

fn segments_to_subpaths(segments: &[PathSegment]) -> Vec<Vec<Point>> {
    let mut paths = Vec::new();
    let mut current: Vec<Point> = Vec::new();
    for seg in segments {
        match seg.cmd.as_str() {
            "M" => {
                if !current.is_empty() {
                    paths.push(current);
                    current = Vec::new();
                }
                if let (Some(x), Some(y)) = (seg.x, seg.y) {
                    current.push(Point { x, y });
                }
            }
            "L" => {
                if let (Some(x), Some(y)) = (seg.x, seg.y) {
                    current.push(Point { x, y });
                }
            }
            "Z" => {
                if current.len() > 2 {
                    let first = current[0].clone();
                    current.push(first);
                }
                if !current.is_empty() {
                    paths.push(current);
                    current = Vec::new();
                }
            }
            _ => {}
        }
    }
    if !current.is_empty() {
        paths.push(current);
    }
    paths
}

fn draw_image(
    p: &mut [u8],
    pw: usize,
    ph: usize,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
    src_type: &str,
    src: &str,
) -> Result<(), Box<dyn Error>> {
    let img = match src_type {
        "path" => image::open(src)?,
        "base64" => {
            let bytes = general_purpose::STANDARD.decode(src.as_bytes())?;
            image::load_from_memory(&bytes)?
        }
        _ => return Err("unsupported image src_type".into()),
    };
    let resized = image::imageops::resize(&img, w, h, image::imageops::FilterType::Lanczos3);
    let (iw, ih) = resized.dimensions();
    for iy in 0..ih {
        for ix in 0..iw {
            let px = x + ix as i32;
            let py = y + iy as i32;
            if px < 0 || py < 0 || px >= pw as i32 || py >= ph as i32 {
                continue;
            }
            let rgba = resized.get_pixel(ix, iy).0;
            let alpha = rgba[3] as u16;
            let idx = (py as usize * pw + px as usize) * 4;
            if alpha == 255 {
                p[idx] = rgba[2];
                p[idx + 1] = rgba[1];
                p[idx + 2] = rgba[0];
                p[idx + 3] = 0;
            } else if alpha > 0 {
                let inv = 255 - alpha;
                p[idx] = ((rgba[2] as u16 * alpha + p[idx] as u16 * inv) / 255) as u8;
                p[idx + 1] = ((rgba[1] as u16 * alpha + p[idx + 1] as u16 * inv) / 255) as u8;
                p[idx + 2] = ((rgba[0] as u16 * alpha + p[idx + 2] as u16 * inv) / 255) as u8;
                p[idx + 3] = 0;
            }
        }
    }
    Ok(())
}

fn draw_text(
    p: &mut [u8], pw: usize, ph: usize,
    x: i32, y: i32, text: &str,
    fg: (u8, u8, u8), bg: Option<(u8, u8, u8)>,
    primary: &fontdue::Font,
    emoji: Option<&fontdue::Font>
) {
    let size = font_size_px();
    let line_height = line_height_px(primary, size);
    
    for (line_index, line) in text.lines().enumerate() {
        if line.trim().is_empty() { continue; }
        
        let cursor_y = y + line_index as i32 * line_height;
        let mut cursor_x = x as f32;
        
        let metrics = primary.horizontal_line_metrics(size).unwrap_or(fontdue::LineMetrics { ascent: size, descent: 0.0, line_gap: 0.0, new_line_size: size * 1.2 });
        let baseline_y = cursor_y as f32 + metrics.ascent;

        for ch in line.chars() {
            let font = if primary.lookup_glyph_index(ch) != 0 { primary } else { emoji.unwrap_or(primary) };
            let (g_metrics, bitmap) = font.rasterize(ch, size);
            
            let gx = cursor_x as i32 + g_metrics.xmin;
            let gy = baseline_y as i32 - (g_metrics.ymin + g_metrics.height as i32);
            
            for by in 0..g_metrics.height {
                for bx in 0..g_metrics.width {
                    let alpha = bitmap[by * g_metrics.width + bx];
                    if alpha == 0 && bg.is_none() { continue; }
                    
                    let px = gx + bx as i32;
                    let py = gy + by as i32;
                    
                    if px >= 0 && px < pw as i32 && py >= 0 && py < ph as i32 {
                        let idx = (py as usize * pw + px as usize) * 4;
                        let real_bg = bg.unwrap_or_else(|| (p[idx+2], p[idx+1], p[idx]));
                        
                        let a = alpha as u16;
                        let inv = 255 - a;
                        
                        p[idx] = ((fg.2 as u16 * a + real_bg.2 as u16 * inv) / 255) as u8;
                        p[idx+1] = ((fg.1 as u16 * a + real_bg.1 as u16 * inv) / 255) as u8;
                        p[idx+2] = ((fg.0 as u16 * a + real_bg.0 as u16 * inv) / 255) as u8;
                        p[idx+3] = 0;
                    }
                }
            }
            cursor_x += g_metrics.advance_width;
        }
    }
}

// --- 现有的辅助函数迁移 ---

fn parse_rgb(value: &str) -> Result<u32, Box<dyn Error>> {
    let value = value.strip_prefix('#').ok_or("color must start with #")?;
    Ok(u32::from_str_radix(value, 16)?)
}

fn rgb_tuple(pixel: u32) -> (u8, u8, u8) {
    (((pixel >> 16) & 0xff) as u8, ((pixel >> 8) & 0xff) as u8, (pixel & 0xff) as u8)
}

fn font_size_px() -> f32 {
    std::env::var("X11_GUI_FONT_SIZE").ok().and_then(|v| v.parse().ok()).unwrap_or(24.0)
}

fn line_height_px(font: &fontdue::Font, size: f32) -> i32 {
    if let Some(m) = font.horizontal_line_metrics(size) {
        ((m.ascent + m.descent.abs() + m.line_gap) * 1.2) as i32
    } else {
        (size * 1.5) as i32
    }
}

fn utf8_to_char2b(text: &str) -> Vec<Char2b> {
    text.encode_utf16().map(|c| Char2b { byte1: (c >> 8) as u8, byte2: (c & 0xff) as u8 }).collect()
}
