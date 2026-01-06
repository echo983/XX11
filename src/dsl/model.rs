use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct RenderEnvelope {
    pub version: String,
    #[serde(rename = "type")]
    pub render_type: String,
    pub seq: u64,
    pub window: WindowSpec,
    pub commands: Vec<Command>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WindowSpec {
    pub width: u32,
    pub height: u32,
    pub title: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EventEnvelope {
    pub version: String,
    #[serde(rename = "type")]
    pub event_type: String,
    pub seq: u64,
    pub event: ClickEvent,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClickEvent {
    pub kind: String,
    pub target_id: String,
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PathSegment {
    pub cmd: String,
    pub x: Option<i32>,
    pub y: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "cmd")]
pub enum Command {
    #[serde(rename = "clear")]
    Clear { color: String },
    #[serde(rename = "rect")]
    Rect {
        id: Option<String>,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        fill: Option<String>,
        stroke: Option<String>,
        stroke_width: Option<u32>,
        #[serde(default)]
        clickable: bool,
    },
    #[serde(rename = "text")]
    Text {
        x: i32,
        y: i32,
        text: String,
        color: Option<String>,
        bg: Option<String>,
    },
    #[serde(rename = "line")]
    Line {
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        color: Option<String>,
        width: Option<u32>,
    },
    #[serde(rename = "circle")]
    Circle {
        cx: Option<i32>,
        cy: Option<i32>,
        r: Option<u32>,
        fill: Option<String>,
        stroke: Option<String>,
        stroke_width: Option<u32>,
    },
    #[serde(rename = "ellipse")]
    Ellipse {
        cx: Option<i32>,
        cy: Option<i32>,
        rx: Option<u32>,
        ry: Option<u32>,
        fill: Option<String>,
        stroke: Option<String>,
        stroke_width: Option<u32>,
    },
    #[serde(rename = "round_rect")]
    RoundRect {
        x: Option<i32>,
        y: Option<i32>,
        w: Option<u32>,
        h: Option<u32>,
        r: Option<u32>,
        fill: Option<String>,
        stroke: Option<String>,
        stroke_width: Option<u32>,
    },
    #[serde(rename = "arc")]
    Arc {
        cx: Option<i32>,
        cy: Option<i32>,
        r: Option<u32>,
        start_angle: Option<f32>,
        end_angle: Option<f32>,
        color: Option<String>,
        width: Option<u32>,
    },
    #[serde(rename = "polyline")]
    Polyline {
        #[serde(default)]
        points: Option<Vec<Point>>,
        color: Option<String>,
        width: Option<u32>,
    },
    #[serde(rename = "polygon")]
    Polygon {
        #[serde(default)]
        points: Option<Vec<Point>>,
        fill: Option<String>,
        stroke: Option<String>,
        stroke_width: Option<u32>,
    },
    #[serde(rename = "image")]
    Image {
        x: Option<i32>,
        y: Option<i32>,
        w: Option<u32>,
        h: Option<u32>,
        src_type: Option<String>,
        src: Option<String>,
    },
    #[serde(rename = "path")]
    Path {
        #[serde(default)]
        segments: Option<Vec<PathSegment>>,
        fill: Option<String>,
        stroke: Option<String>,
        stroke_width: Option<u32>,
    },
}
