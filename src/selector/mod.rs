//! Full-screen overlay for choosing the recording area. The selection is kept
//! in physical pixels of a frozen screenshot, so it matches what is captured.

mod view;

use crate::settings::{MIN_DIM, Region};
use iced::{window, Point, Size};
use iced::widget::image;

const GRAB: f32 = 12.0;

#[derive(Debug, Clone)]
pub struct Shot {
    pub handle: image::Handle,
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Clone)]
pub enum SelMsg {
    Moved(Point, Size),
    Pressed,
    Released,
    All,
    Confirm,
    Cancel,
}

#[derive(Debug, Clone, Copy)]
struct Edges {
    l: f32,
    t: f32,
    r: f32,
    b: f32,
}

impl Edges {
    fn w(&self) -> f32 {
        self.r - self.l
    }
    fn h(&self) -> f32 {
        self.b - self.t
    }
}

#[derive(Debug, Clone, Copy)]
enum Drag {
    Create { ax: f32, ay: f32 },
    Move { dx: f32, dy: f32, orig: Edges },
    Resize {
        left: bool,
        right: bool,
        top: bool,
        bottom: bool,
        orig: Edges,
    },
}

pub struct Selector {
    pub id: window::Id,
    handle: image::Handle,
    img_w: u32,
    img_h: u32,
    rect: Option<Edges>,
    drag: Option<Drag>,
    cursor: Point,
    view: Size,
}

impl Selector {
    pub fn new(id: window::Id, shot: Shot, current: Option<Region>) -> Self {
        let rect = current.map(|r| Edges {
            l: r.x as f32,
            t: r.y as f32,
            r: (r.x + r.w) as f32,
            b: (r.y + r.h) as f32,
        });
        Selector {
            id,
            handle: shot.handle,
            img_w: shot.w,
            img_h: shot.h,
            rect,
            drag: None,
            cursor: Point::ORIGIN,
            view: Size::ZERO,
        }
    }

    /// An area that is big enough to record has been drawn.
    pub fn can_confirm(&self) -> bool {
        self.region().is_some()
    }

    pub fn region(&self) -> Option<Region> {
        let e = self.rect?;
        Region {
            x: e.l.round().max(0.0) as u32,
            y: e.t.round().max(0.0) as u32,
            w: (e.w().round().max(0.0) as u32) & !1,
            h: (e.h().round().max(0.0) as u32) & !1,
        }
        .clamp_to(self.img_w, self.img_h)
    }

    fn scale(&self) -> (f32, f32) {
        (
            self.img_w as f32 / self.view.width.max(1.0),
            self.img_h as f32 / self.view.height.max(1.0),
        )
    }

    fn cursor_img(&self) -> (f32, f32) {
        let (sx, sy) = self.scale();
        (
            (self.cursor.x * sx).clamp(0.0, self.img_w as f32),
            (self.cursor.y * sy).clamp(0.0, self.img_h as f32),
        )
    }

    pub fn update(&mut self, msg: SelMsg) {
        match msg {
            SelMsg::Moved(p, size) => {
                self.cursor = p;
                self.view = size;
                if let Some(d) = self.drag {
                    self.apply_drag(d);
                }
            }
            SelMsg::Pressed => self.begin_drag(),
            SelMsg::Released => self.drag = None,
            SelMsg::All => {
                self.rect = Some(Edges {
                    l: 0.0,
                    t: 0.0,
                    r: self.img_w as f32,
                    b: self.img_h as f32,
                });
            }
            // Handled by the app (they close the window).
            SelMsg::Confirm | SelMsg::Cancel => {}
        }
    }

    fn begin_drag(&mut self) {
        if self.view.width < 1.0 || self.view.height < 1.0 {
            return;
        }
        let (sx, sy) = self.scale();
        let p = self.cursor;
        let (ix, iy) = self.cursor_img();

        if let Some(e) = self.rect {
            let (l, t, r, b) = (e.l / sx, e.t / sy, e.r / sx, e.b / sy);
            let near = p.x >= l - GRAB && p.x <= r + GRAB && p.y >= t - GRAB && p.y <= b + GRAB;
            if near {
                let left = (p.x - l).abs() <= GRAB;
                let right = !left && (p.x - r).abs() <= GRAB;
                let top = (p.y - t).abs() <= GRAB;
                let bottom = !top && (p.y - b).abs() <= GRAB;
                self.drag = Some(if left || right || top || bottom {
                    Drag::Resize {
                        left,
                        right,
                        top,
                        bottom,
                        orig: e,
                    }
                } else {
                    Drag::Move {
                        dx: ix - e.l,
                        dy: iy - e.t,
                        orig: e,
                    }
                });
                return;
            }
        }
        self.drag = Some(Drag::Create { ax: ix, ay: iy });
        self.rect = None;
    }

    fn apply_drag(&mut self, drag: Drag) {
        let (iw, ih) = (self.img_w as f32, self.img_h as f32);
        let (ix, iy) = self.cursor_img();
        match drag {
            Drag::Create { ax, ay } => {
                self.rect = Some(Edges {
                    l: ax.min(ix),
                    t: ay.min(iy),
                    r: ax.max(ix),
                    b: ay.max(iy),
                });
            }
            Drag::Move { dx, dy, orig } => {
                let (w, h) = (orig.w(), orig.h());
                let l = (ix - dx).clamp(0.0, (iw - w).max(0.0));
                let t = (iy - dy).clamp(0.0, (ih - h).max(0.0));
                self.rect = Some(Edges {
                    l,
                    t,
                    r: l + w,
                    b: t + h,
                });
            }
            Drag::Resize {
                left,
                right,
                top,
                bottom,
                orig,
            } => {
                let mut e = orig;
                if left {
                    e.l = ix;
                }
                if right {
                    e.r = ix;
                }
                if top {
                    e.t = iy;
                }
                if bottom {
                    e.b = iy;
                }
                // Dragging an edge past the opposite one flips the box.
                let n = Edges {
                    l: e.l.min(e.r),
                    r: e.l.max(e.r),
                    t: e.t.min(e.b),
                    b: e.t.max(e.b),
                };
                if n.w() >= MIN_DIM as f32 && n.h() >= MIN_DIM as f32 {
                    self.rect = Some(n);
                }
            }
        }
    }
}
