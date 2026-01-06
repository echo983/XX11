use std::error::Error;

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    ChangeWindowAttributesAux, ConnectionExt, CreateGCAux, CreateWindowAux, EventMask,
    WindowClass,
};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as WrapperConnectionExt;
use fontdue::Font;

pub struct X11Backend {
    conn: RustConnection,
    window: u32,
    gc: u32,
    _cursor: u32,
    font: u32,
    depth: u8,
    bits_per_pixel: u8,
    font_primary: Option<Font>,
    font_emoji: Option<Font>,
}

impl X11Backend {
    pub fn connect(width: u16, height: u16, title: &str) -> Result<Self, Box<dyn Error>> {
        let (conn, screen_num) = x11rb::connect(None)?;
        let screen = &conn.setup().roots[screen_num];

        let window = conn.generate_id()?;
        let gc = conn.generate_id()?;

        let aux = CreateWindowAux::new()
            .background_pixel(screen.white_pixel)
            .event_mask(EventMask::EXPOSURE | EventMask::BUTTON_PRESS | EventMask::BUTTON_RELEASE);

        conn.create_window(
            screen.root_depth,
            window,
            screen.root,
            0,
            0,
            width,
            height,
            0,
            WindowClass::INPUT_OUTPUT,
            0,
            &aux,
        )?;

        conn.create_gc(gc, window, &CreateGCAux::new())?;
        conn.change_property8(
            x11rb::protocol::xproto::PropMode::REPLACE,
            window,
            x11rb::protocol::xproto::AtomEnum::WM_NAME,
            x11rb::protocol::xproto::AtomEnum::STRING,
            title.as_bytes(),
        )?;
        let cursor = create_default_cursor(&conn, window)?;
        let font = open_text_font(&conn)?;
        let (font_primary, font_emoji) = load_fonts();
        let (depth, bits_per_pixel) = query_depth_and_bpp(&conn, screen.root_depth);
        conn.map_window(window)?;
        conn.flush()?;

        Ok(Self {
            conn,
            window,
            gc,
            _cursor: cursor,
            font,
            depth,
            bits_per_pixel,
            font_primary,
            font_emoji,
        })
    }

    pub fn connection(&self) -> &RustConnection {
        &self.conn
    }

    pub fn window(&self) -> u32 {
        self.window
    }

    pub fn gc(&self) -> u32 {
        self.gc
    }

    pub fn font(&self) -> u32 {
        self.font
    }

    pub fn depth(&self) -> u8 {
        self.depth
    }

    pub fn bits_per_pixel(&self) -> u8 {
        self.bits_per_pixel
    }

    pub fn font_primary(&self) -> Option<&Font> {
        self.font_primary.as_ref()
    }

    pub fn font_emoji(&self) -> Option<&Font> {
        self.font_emoji.as_ref()
    }
}

fn create_default_cursor(conn: &RustConnection, window: u32) -> Result<u32, Box<dyn Error>> {
    let font = conn.generate_id()?;
    conn.open_font(font, b"cursor")?;
    let cursor = conn.generate_id()?;
    conn.create_glyph_cursor(
        cursor,
        font,
        font,
        68,
        69,
        0,
        0,
        0,
        0xffff,
        0xffff,
        0xffff,
    )?;
    conn.close_font(font)?;
    conn.change_window_attributes(window, &ChangeWindowAttributesAux::new().cursor(cursor))?;
    Ok(cursor)
}

fn open_text_font(conn: &RustConnection) -> Result<u32, Box<dyn Error>> {
    let font = conn.generate_id()?;
    let iso_font = b"-misc-fixed-*-*-*-*-13-*-*-*-*-*-iso10646-1";
    if conn.open_font(font, iso_font).is_err() {
        conn.open_font(font, b"fixed")?;
    }
    Ok(font)
}

fn query_depth_and_bpp(conn: &RustConnection, depth: u8) -> (u8, u8) {
    let mut bpp = 32;
    for fmt in &conn.setup().pixmap_formats {
        if fmt.depth == depth {
            bpp = fmt.bits_per_pixel;
            break;
        }
    }
    (depth, bpp)
}

pub fn load_fonts() -> (Option<Font>, Option<Font>) {
    let primary_candidates = vec![
        std::env::var("X11_GUI_FONT").ok(),
        Some("C:\\Windows\\Fonts\\msyh.ttc".to_string()),
        Some("C:\\Windows\\Fonts\\simhei.ttf".to_string()),
        Some("C:\\Windows\\Fonts\\segoeui.ttf".to_string()),
        Some("C:\\Windows\\Fonts\\arial.ttf".to_string()),
    ];

    let emoji_candidates = vec![
        std::env::var("X11_GUI_EMOJI_FONT").ok(),
        Some("C:\\Windows\\Fonts\\seguiemj.ttf".to_string()),
    ];

    let mut primary = None;
    for path in primary_candidates.into_iter().flatten() {
        if let Some(font) = load_font_from_path(&path) {
            primary = Some(font);
            break;
        }
    }

    let mut emoji = None;
    for path in emoji_candidates.into_iter().flatten() {
        if let Some(font) = load_font_from_path(&path) {
            emoji = Some(font);
            break;
        }
    }

    (primary, emoji)
}

fn load_font_from_path(path: &str) -> Option<Font> {
    match std::fs::read(path) {
        Ok(bytes) => Font::from_bytes(bytes, fontdue::FontSettings::default()).ok(),
        Err(_) => None,
    }
}
