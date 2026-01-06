use std::error::Error;

use x11rb::connection::Connection;
use x11rb::protocol::Event;

use crate::x11::backend::X11Backend;

pub struct ClickEvent {
    pub x: i32,
    pub y: i32,
}

pub fn poll_for_click(backend: &X11Backend) -> Result<Option<ClickEvent>, Box<dyn Error>> {
    let conn = backend.connection();
    if let Some(event) = conn.poll_for_event()? {
        if let Event::ButtonRelease(ev) = event {
            return Ok(Some(ClickEvent {
                x: ev.event_x.into(),
                y: ev.event_y.into(),
            }));
        }
    }
    Ok(None)
}
