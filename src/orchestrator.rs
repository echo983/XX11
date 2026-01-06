use std::error::Error;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crate::dsl::{parser, validator};
use crate::llm::gpt52;
use crate::dsl::model::{ClickEvent, Command, EventEnvelope};
use crate::state::hit_test::{HitTarget, HitTestIndex};
use crate::x11::{backend, events, renderer};

pub fn run() -> Result<(), Box<dyn Error>> {
    let mut hit_test = HitTestIndex::new();

    let initial = gpt52::request_render(None, None)?;
    let parsed = parser::parse_render(initial.as_str())?;
    validator::validate_render(&parsed)?;
    let mut last_render_seq = parsed.seq;
    let mut event_seq = 0u64;
    let x11 = backend::X11Backend::connect(
        parsed.window.width as u16,
        parsed.window.height as u16,
        &parsed.window.title,
    )?;
    renderer::render_frame(&x11, &parsed)?;
    build_hit_test(&mut hit_test, &parsed);

    let (tx, rx) = mpsc::channel::<String>();
    thread::spawn(move || {
        let stdin = std::io::stdin();
        let mut line = String::new();
        loop {
            line.clear();
            if stdin.read_line(&mut line).is_err() {
                break;
            }
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if tx.send(trimmed.to_string()).is_err() {
                break;
            }
        }
    });

    loop {
        while let Ok(text) = rx.try_recv() {
            let next = gpt52::request_render(None, Some(text.as_str()))?;
            let parsed = parser::parse_render(next.as_str())?;
            validator::validate_render(&parsed)?;
            if parsed.seq < last_render_seq {
                eprintln!(
                    "warn: non-monotonic render seq {} (last {})",
                    parsed.seq, last_render_seq
                );
            } else if parsed.seq > last_render_seq {
                last_render_seq = parsed.seq;
            }
            renderer::render_frame(&x11, &parsed)?;
            build_hit_test(&mut hit_test, &parsed);
        }

        if let Some(click) = events::poll_for_click(&x11)? {
            if let Some(target_id) = hit_test.hit(click.x, click.y) {
                event_seq += 1;
                let event_json =
                    build_click_event_json(target_id, click.x, click.y, event_seq)?;
                let next = gpt52::request_render(Some(event_json.as_str()), None)?;
                let parsed = parser::parse_render(next.as_str())?;
                validator::validate_render(&parsed)?;
                if parsed.seq < last_render_seq {
                    eprintln!(
                        "warn: non-monotonic render seq {} (last {})",
                        parsed.seq, last_render_seq
                    );
                } else if parsed.seq > last_render_seq {
                    last_render_seq = parsed.seq;
                }
                renderer::render_frame(&x11, &parsed)?;
                build_hit_test(&mut hit_test, &parsed);
            }
        }

        thread::sleep(Duration::from_millis(16));
    }
}

fn build_click_event_json(
    target_id: &str,
    x: i32,
    y: i32,
    seq: u64,
) -> Result<String, Box<dyn Error>> {
    let event = EventEnvelope {
        version: "AGD/0.1".to_string(),
        event_type: "event".to_string(),
        seq,
        event: ClickEvent {
            kind: "click".to_string(),
            target_id: target_id.to_string(),
            x,
            y,
        },
    };
    Ok(serde_json::to_string(&event)?)
}

fn build_hit_test(index: &mut HitTestIndex, render: &crate::dsl::model::RenderEnvelope) {
    index.reset();
    for command in &render.commands {
        if let Command::Rect {
            id,
            x,
            y,
            w,
            h,
            clickable,
            ..
        } = command
        {
            if *clickable {
                if let Some(id) = id {
                    index.add(HitTarget {
                        id: id.clone(),
                        x: *x,
                        y: *y,
                        w: *w,
                        h: *h,
                    });
                }
            }
        }
    }
}
