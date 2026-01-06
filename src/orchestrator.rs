use std::error::Error;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use std::io::{self, Write};
use base64::{Engine as _, engine::general_purpose};
use image::{ImageBuffer, Rgba};
use serde_json::Value;

use crate::dsl::{parser, validator};
use crate::llm::gpt52::{self, LLMMode};
use crate::dsl::model::{ClickEvent, Command, EventEnvelope, RenderEnvelope};
use crate::state::hit_test::{HitTarget, HitTestIndex};
use crate::x11::{backend, events, renderer};

pub fn run() -> Result<(), Box<dyn Error>> {
    let mut hit_test = HitTestIndex::new();
    let (primary, emoji) = backend::load_fonts();
    let is_debug = std::env::var("AGD_DEBUG").map(|v| v == "1").unwrap_or(false);

    if is_debug {
        let _ = std::fs::create_dir_all("debug_out");
    }

    println!("AGD UI Bridge active.");
    
    // 等待用户输入后再开始
    print!(">> ");
    io::stdout().flush()?;
    let mut initial_input = String::new();
    io::stdin().read_line(&mut initial_input)?;
    
    let initial_dsl = gpt52::request_render(None, Some(initial_input.trim()), LLMMode::Generate)?;
    let parsed = iterate_to_final(&initial_dsl, None, Some(initial_input.trim()), primary.as_ref(), emoji.as_ref(), is_debug)?;
    
    let mut last_render_seq = parsed.seq;
    let mut event_seq = 0u64;
    let mut current_render = parsed.clone();
    
    let x11 = backend::X11Backend::connect(
        parsed.window.width as u16,
        parsed.window.height as u16,
        &parsed.window.title,
    )?;
    
    renderer::render_frame(&x11, &parsed)?;
    build_hit_test(&mut hit_test, &parsed);

    let (tx, rx) = mpsc::channel::<String>();
    thread::spawn(move || {
        let mut line = String::new();
        loop {
            print!(">> ");
            let _ = io::stdout().flush();
            line.clear();
            if io::stdin().read_line(&mut line).is_err() { break; }
            let trimmed = line.trim();
            if trimmed.is_empty() { continue; }
            if tx.send(trimmed.to_string()).is_err() { break; }
        }
    });

    loop {
        while let Ok(text) = rx.try_recv() {
            let next_dsl = gpt52::request_render(None, Some(text.as_str()), LLMMode::Generate)?;
            let parsed = iterate_to_final(&next_dsl, None, Some(text.as_str()), primary.as_ref(), emoji.as_ref(), is_debug)?;
            update_ui(&x11, &parsed, &mut last_render_seq, &mut hit_test)?;
            current_render = parsed.clone();
        }

        if let Some(click) = events::poll_for_click(&x11)? {
            if let Some(target) = hit_test.hit_target(click.x, click.y) {
                render_pressed_feedback(&x11, &current_render, target)?;
                event_seq += 1;
                let event_json = build_click_event_json(target.id.as_str(), click.x, click.y, event_seq)?;
                let next_dsl = gpt52::request_render(Some(event_json.as_str()), None, LLMMode::Generate)?;
                let parsed = iterate_to_final(&next_dsl, Some(&event_json), None, primary.as_ref(), emoji.as_ref(), is_debug)?;
                update_ui(&x11, &parsed, &mut last_render_seq, &mut hit_test)?;
                current_render = parsed.clone();
            }
        }

        thread::sleep(Duration::from_millis(16));
    }
}

fn iterate_to_final(
    initial_dsl: &str,
    event_json: Option<&str>,
    user_text: Option<&str>,
    primary: Option<&fontdue::Font>,
    emoji: Option<&fontdue::Font>,
    is_debug: bool,
) -> Result<RenderEnvelope, Box<dyn Error>> {
    let mut current_dsl = initial_dsl.to_string();
    let max_iterations = 4;

    for i in 0..max_iterations {
        let parsed = parser::parse_render(&current_dsl)?;
        validator::validate_render(&parsed)?;

        let (w, h, pixels) = renderer::render_to_buffer(&parsed, primary, emoji)?;
        let jpg_data = buffer_to_scaled_jpg(w, h, &pixels, 0.3)?;
        let jpg_base64 = general_purpose::STANDARD.encode(&jpg_data);
        
        if is_debug {
            let _ = std::fs::write(format!("debug_out/iter_{}_draft.json", i), &current_dsl);
            let _ = std::fs::write(format!("debug_out/iter_{}_draft.jpg", i), &jpg_data);
        }

        println!("Iteration {}: Evaluating UI quality...", i + 1);
        let feedback_json = gpt52::request_render(event_json, user_text, LLMMode::Evaluate {
            image_base64: jpg_base64,
            dsl_code: current_dsl.clone(),
        })?;

        if is_debug {
            let _ = std::fs::write(format!("debug_out/iter_{}_feedback.json", i), &feedback_json);
        }

        let v: Value = serde_json::from_str(&feedback_json)?;
        let is_final = v["is_final"].as_bool().unwrap_or(false);
        let reason = v["rejection_reason"].as_str().unwrap_or("No reason provided");
        let render_val = v["render"].clone();

        if is_final {
            println!("UI Finalized in {} iterations.", i + 1);
            return Ok(serde_json::from_value(render_val)?);
        } else {
            println!("LLM REJECTED DRAFT. Reason: {}", reason);
            if is_debug {
                let _ = std::fs::write(format!("debug_out/iter_{}_reason.txt", i), reason);
            }
            current_dsl = serde_json::to_string(&render_val)?;
        }
    }

    parser::parse_render(&current_dsl)
}

fn buffer_to_scaled_jpg(w: usize, h: usize, pixels: &[u8], scale: f32) -> Result<Vec<u8>, Box<dyn Error>> {
    let sw = (w as f32 * scale) as u32;
    let sh = (h as f32 * scale) as u32;
    let mut rgba = vec![0u8; w * h * 4];
    for i in 0..(w * h) {
        rgba[i*4] = pixels[i*4+2];
        rgba[i*4+1] = pixels[i*4+1];
        rgba[i*4+2] = pixels[i*4];
        rgba[i*4+3] = 255;
    }
    let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_raw(w as u32, h as u32, rgba).ok_or("buffer size mismatch")?;
    let scaled = image::imageops::resize(&img, sw, sh, image::imageops::FilterType::Lanczos3);
    let mut cursor = std::io::Cursor::new(Vec::new());
    scaled.write_to(&mut cursor, image::ImageFormat::Jpeg)?;
    Ok(cursor.into_inner())
}

fn update_ui(
    x11: &backend::X11Backend,
    parsed: &RenderEnvelope,
    last_seq: &mut u64,
    hit_test: &mut HitTestIndex,
) -> Result<(), Box<dyn Error>> {
    validator::validate_render(parsed)?;
    if parsed.seq > *last_seq { *last_seq = parsed.seq; }
    renderer::render_frame(x11, parsed)?;
    build_hit_test(hit_test, parsed);
    Ok(())
}

fn build_click_event_json(target_id: &str, x: i32, y: i32, seq: u64) -> Result<String, Box<dyn Error>> {
    let event = EventEnvelope {
        version: "AGD/0.2".to_string(),
        event_type: "event".to_string(),
        seq,
        event: ClickEvent { kind: "click".to_string(), target_id: target_id.to_string(), x, y },
    };
    Ok(serde_json::to_string(&event)?)
}

fn build_hit_test(index: &mut HitTestIndex, render: &RenderEnvelope) {
    index.reset();
    for command in &render.commands {
        if let Command::Rect { id, x, y, w, h, clickable, .. } = command {
            if *clickable {
                if let Some(id) = id {
                    index.add(HitTarget { id: id.clone(), x: *x, y: *y, w: *w, h: *h });
                }
            }
        }
    }
}

fn render_pressed_feedback(
    x11: &backend::X11Backend,
    render: &RenderEnvelope,
    target: &HitTarget,
) -> Result<(), Box<dyn Error>> {
    renderer::render_frame_with_press(x11, render, target.x, target.y, target.w, target.h)?;
    thread::sleep(Duration::from_millis(60));
    renderer::render_frame(x11, render)?;
    Ok(())
}
