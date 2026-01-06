use std::error::Error;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    Char2b, ConnectionExt, ImageFormat,
};
use crate::dsl::model::{Command, RenderEnvelope};
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
    // 简单的 Bresenham 或粗暴填充（对于 V0.1 线条够用了）
    if x1 == x2 {
        let start = y1.min(y2);
        let end = y1.max(y2);
        fill_rect(p, pw, ph, x1 - (t as i32 / 2), start, t, (end - start) as u32, rgb);
    } else if y1 == y2 {
        let start = x1.min(x2);
        let end = x1.max(x2);
        fill_rect(p, pw, ph, start, y1 - (t as i32 / 2), (end - start) as u32, t, rgb);
    }
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
