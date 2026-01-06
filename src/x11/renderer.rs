use std::error::Error;

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    ChangeGCAux, Char2b, ConnectionExt, CoordMode, ImageFormat, Point, Rectangle,
};

use crate::dsl::model::{Command, RenderEnvelope};
use crate::x11::backend::X11Backend;

pub fn render_frame(backend: &X11Backend, render: &RenderEnvelope) -> Result<(), Box<dyn Error>> {
    let conn = backend.connection();
    let window = backend.window();
    let gc = backend.gc();

    let mut clear_pixel = parse_rgb("#ffffff")?;
    for command in &render.commands {
        match command {
            Command::Clear { color } => {
                let pixel = parse_rgb(color)?;
                clear_pixel = pixel;
                let gc_aux = ChangeGCAux::new().foreground(pixel);
                conn.change_gc(gc, &gc_aux)?;
                let rect = Rectangle {
                    x: 0,
                    y: 0,
                    width: render.window.width as u16,
                    height: render.window.height as u16,
                };
                conn.poly_fill_rectangle(window, gc, &[rect])?;
            }
            Command::Rect {
                x,
                y,
                w,
                h,
                fill,
                stroke,
                stroke_width,
                ..
            } => {
                let rect = Rectangle {
                    x: *x as i16,
                    y: *y as i16,
                    width: *w as u16,
                    height: *h as u16,
                };
                if let Some(fill) = fill {
                    let pixel = parse_rgb(fill)?;
                    let gc_aux = ChangeGCAux::new().foreground(pixel);
                    conn.change_gc(gc, &gc_aux)?;
                    conn.poly_fill_rectangle(window, gc, &[rect])?;
                }
                if let Some(stroke) = stroke {
                    let pixel = parse_rgb(stroke)?;
                    let mut gc_aux = ChangeGCAux::new().foreground(pixel);
                    if let Some(width) = stroke_width {
                        gc_aux = gc_aux.line_width(*width as u32);
                    }
                    conn.change_gc(gc, &gc_aux)?;
                    conn.poly_rectangle(window, gc, &[rect])?;
                }
            }
            Command::Text { x, y, text, color, bg } => {
                if let Some(primary) = backend.font_primary() {
                    let text_color = color.as_deref().unwrap_or("#000000");
                    let bg_color = bg.as_deref();
                    
                    render_text_bitmap(
                        conn,
                        backend,
                        window,
                        gc,
                        *x,
                        *y,
                        text,
                        text_color,
                        bg_color,
                        clear_pixel,
                        primary,
                        backend.font_emoji(),
                    )?;
                } else {
                    let glyphs = utf8_to_char2b(text);
                    if glyphs.is_empty() {
                        continue;
                    }
                    let pixel = parse_rgb(color.as_deref().unwrap_or("#000000"))?;
                    let gc_aux = ChangeGCAux::new()
                        .foreground(pixel)
                        .font(backend.font());
                    conn.change_gc(gc, &gc_aux)?;
                    conn.image_text16(window, gc, *x as i16, *y as i16, &glyphs)?;
                }
            }
            Command::Line {
                x1,
                y1,
                x2,
                y2,
                color,
                width,
            } => {
                let pixel = parse_rgb(color.as_deref().unwrap_or("#000000"))?;
                let mut gc_aux = ChangeGCAux::new().foreground(pixel);
                if let Some(width) = width {
                    gc_aux = gc_aux.line_width(*width as u32);
                }
                conn.change_gc(gc, &gc_aux)?;
                let points = [
                    Point { x: *x1 as i16, y: *y1 as i16 },
                    Point { x: *x2 as i16, y: *y2 as i16 },
                ];
                conn.poly_line(CoordMode::ORIGIN, window, gc, &points)?;
            }
        }
    }

    conn.flush()?;
    Ok(())
}

fn parse_rgb(value: &str) -> Result<u32, Box<dyn Error>> {
    let value = value.strip_prefix('#').ok_or("color must start with #")?;
    if value.len() != 6 {
        return Err("color must be #RRGGBB".into());
    }
    let rgb = u32::from_str_radix(value, 16)?;
    Ok(rgb)
}

fn utf8_to_char2b(text: &str) -> Vec<Char2b> {
    text.encode_utf16()
        .map(|code_unit| Char2b {
            byte1: (code_unit >> 8) as u8,
            byte2: (code_unit & 0xff) as u8,
        })
        .collect()
}

fn render_text_bitmap(
    conn: &x11rb::rust_connection::RustConnection,
    backend: &X11Backend,
    window: u32,
    gc: u32,
    x: i32,
    y: i32,
    text: &str,
    color: &str,
    bg_override: Option<&str>,
    clear_pixel: u32,
    primary: &fontdue::Font,
    emoji: Option<&fontdue::Font>,
) -> Result<(), Box<dyn Error>> {
    let fg = rgb_tuple(parse_rgb(color)?);
    let bg = if let Some(bg_str) = bg_override {
        rgb_tuple(parse_rgb(bg_str)?)
    } else {
        rgb_tuple(clear_pixel)
    };
    let size = font_size_px();
    let line_height = line_height_px(primary, size);

    for (line_index, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let (width, height, pixels) = rasterize_line(line, size, fg, bg, primary, emoji);
        if width == 0 || height == 0 {
            continue;
        }
        let dst_x = x;
        let dst_y = y + line_index as i32 * line_height;
        conn.put_image(
            ImageFormat::Z_PIXMAP,
            window,
            gc,
            width as u16,
            height as u16,
            dst_x as i16,
            dst_y as i16,
            0,
            backend.depth(),
            &pixels,
        )?;
    }
    Ok(())
}

fn rasterize_line(
    line: &str,
    size: f32,
    fg: (u8, u8, u8),
    bg: (u8, u8, u8),
    primary: &fontdue::Font,
    emoji: Option<&fontdue::Font>,
) -> (usize, usize, Vec<u8>) {
    struct Glyph {
        x: i32,
        y: i32,
        width: usize,
        height: usize,
        bitmap: Vec<u8>,
    }

    let mut glyphs = Vec::new();
    let mut cursor_x = 0.0f32;
    let metrics = primary.horizontal_line_metrics(size);
    let ascent = metrics.map(|m| m.ascent).unwrap_or(size);
    let descent = metrics.map(|m| m.descent.abs()).unwrap_or(0.0);
    let line_gap = metrics.map(|m| m.line_gap).unwrap_or(0.0);
    let baseline_y = ascent;
    let mut max_x = 0i32;

    for ch in line.chars() {
        let font = select_font(primary, emoji, ch);
        let (metrics, bitmap) = font.rasterize(ch, size);
        let gx = cursor_x as i32 + metrics.xmin;
        let gy = (baseline_y as i32) - (metrics.ymin + metrics.height as i32);
        let gw = metrics.width as i32;
        let gh = metrics.height as i32;

        max_x = max_x.max(gx + gw);

        glyphs.push(Glyph {
            x: gx,
            y: gy,
            width: metrics.width,
            height: metrics.height,
            bitmap,
        });

        cursor_x += metrics.advance_width;
    }

    let width = (max_x.max(0) as usize) + 1;
    let height = (ascent + descent + line_gap).ceil().max(0.0) as usize;
    if width == 0 || height == 0 {
        return (0, 0, Vec::new());
    }

    let mut pixels = vec![0u8; width * height * 4];
    fill_background(&mut pixels, width, height, bg);

    for glyph in glyphs {
        if glyph.width == 0 || glyph.height == 0 {
            continue;
        }
        for gy in 0..glyph.height {
            for gx in 0..glyph.width {
                let alpha = glyph.bitmap[gy * glyph.width + gx];
                if alpha == 0 {
                    continue;
                }
                let px = glyph.x + gx as i32;
                let py = glyph.y + gy as i32;
                if px < 0 || py < 0 || px >= width as i32 || py >= height as i32 {
                    continue;
                }
                blend_pixel(&mut pixels, width, px as usize, py as usize, fg, bg, alpha);
            }
        }
    }

    (width, height, pixels)
}

fn line_height_px(font: &fontdue::Font, size: f32) -> i32 {
    if let Some(metrics) = font.horizontal_line_metrics(size) {
        let height = (metrics.ascent + metrics.descent.abs() + metrics.line_gap).ceil();
        (height * 1.2).max(1.0) as i32
    } else {
        (size * 1.5) as i32
    }
}

fn font_ascent(font: &fontdue::Font, size: f32) -> i32 {
    if let Some(metrics) = font.horizontal_line_metrics(size) {
        metrics.ascent.ceil().max(1.0) as i32
    } else {
        size.ceil().max(1.0) as i32
    }
}

fn font_size_px() -> f32 {
    if let Ok(value) = std::env::var("X11_GUI_FONT_SIZE") {
        if let Ok(parsed) = value.parse::<f32>() {
            if parsed >= 8.0 && parsed <= 72.0 {
                return parsed;
            }
        }
    }
    24.0
}

fn select_font<'a>(
    primary: &'a fontdue::Font,
    emoji: Option<&'a fontdue::Font>,
    ch: char,
) -> &'a fontdue::Font {
    let primary_has = primary.lookup_glyph_index(ch) != 0;
    if primary_has {
        return primary;
    }
    if let Some(emoji) = emoji {
        if emoji.lookup_glyph_index(ch) != 0 {
            return emoji;
        }
    }
    primary
}

fn fill_background(pixels: &mut [u8], width: usize, height: usize, bg: (u8, u8, u8)) {
    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) * 4;
            pixels[idx] = bg.2;
            pixels[idx + 1] = bg.1;
            pixels[idx + 2] = bg.0;
            pixels[idx + 3] = 0;
        }
    }
}

fn blend_pixel(
    pixels: &mut [u8],
    width: usize,
    x: usize,
    y: usize,
    fg: (u8, u8, u8),
    bg: (u8, u8, u8),
    alpha: u8,
) {
    let idx = (y * width + x) * 4;
    let a = alpha as u16;
    let inv = 255u16 - a;
    let r = (fg.0 as u16 * a + bg.0 as u16 * inv) / 255;
    let g = (fg.1 as u16 * a + bg.1 as u16 * inv) / 255;
    let b = (fg.2 as u16 * a + bg.2 as u16 * inv) / 255;
    pixels[idx] = b as u8;
    pixels[idx + 1] = g as u8;
    pixels[idx + 2] = r as u8;
    pixels[idx + 3] = 0;
}

fn rgb_tuple(pixel: u32) -> (u8, u8, u8) {
    let r = ((pixel >> 16) & 0xff) as u8;
    let g = ((pixel >> 8) & 0xff) as u8;
    let b = (pixel & 0xff) as u8;
    (r, g, b)
}