#[derive(Debug, Default)]
pub struct HitTestIndex {
    items: Vec<HitTarget>,
}

#[derive(Debug, Clone)]
pub struct HitTarget {
    pub id: String,
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

impl HitTestIndex {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn reset(&mut self) {
        self.items.clear();
    }

    pub fn add(&mut self, target: HitTarget) {
        self.items.push(target);
    }

    pub fn hit(&self, x: i32, y: i32) -> Option<&str> {
        for item in &self.items {
            if x >= item.x
                && y >= item.y
                && x < item.x + item.w as i32
                && y < item.y + item.h as i32
            {
                return Some(item.id.as_str());
            }
        }
        None
    }

    pub fn hit_target(&self, x: i32, y: i32) -> Option<&HitTarget> {
        for item in &self.items {
            if x >= item.x
                && y >= item.y
                && x < item.x + item.w as i32
                && y < item.y + item.h as i32
            {
                return Some(item);
            }
        }
        None
    }
}
