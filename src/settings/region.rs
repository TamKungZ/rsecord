use super::MIN_DIM;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Region {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl Region {
    pub fn clamp_to(self, max_w: u32, max_h: u32) -> Option<Region> {
        if max_w < MIN_DIM || max_h < MIN_DIM || self.w < MIN_DIM || self.h < MIN_DIM {
            return None;
        }
        let x = self.x.min(max_w - MIN_DIM);
        let y = self.y.min(max_h - MIN_DIM);
        let w = self.w.min(max_w - x) & !1;
        let h = self.h.min(max_h - y) & !1;
        if w < MIN_DIM || h < MIN_DIM {
            return None;
        }
        Some(Region { x, y, w, h })
    }
}
