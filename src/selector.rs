//! Full-screen overlay for choosing the recording area. The selection is kept
//! in physical pixels of a frozen screenshot, so it matches what is captured.

use crate::settings::{Region, MIN_DIM};
use iced::widget::{button, container, image, mouse_area, responsive, row, text, Stack};
use iced::{window, Alignment, Background, Border, Color, ContentFit, Element, Length, Padding, Point, Size, Theme};

const GRAB: f32 = 12.0;
const HANDLE: f32 = 10.0;
const ACCENT: Color = Color::from_rgb(0.20, 0.62, 1.0);

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

    pub fn view(&self) -> Element<'_, SelMsg> {
        let handle = self.handle.clone();
        let rect = self.rect;
        let (iw, ih) = (self.img_w as f32, self.img_h as f32);

        responsive(move |size: Size| {
            let mut layers: Vec<Element<'static, SelMsg>> = Vec::new();

            layers.push(
                image(handle.clone())
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .content_fit(ContentFit::Fill)
                    .into(),
            );

            match rect {
                None => {
                    layers.push(abs(0.0, 0.0, dim(size.width, size.height)));
                }
                Some(e) => {
                    let (sx, sy) = (size.width / iw, size.height / ih);
                    let (l, t) = (e.l * sx, e.t * sy);
                    let (w, h) = (e.w() * sx, e.h() * sy);

                    layers.push(abs(0.0, 0.0, dim(size.width, t)));
                    layers.push(abs(0.0, t + h, dim(size.width, size.height - t - h)));
                    layers.push(abs(0.0, t, dim(l, h)));
                    layers.push(abs(l + w, t, dim(size.width - l - w, h)));

                    layers.push(abs(
                        l,
                        t,
                        container("")
                            .width(Length::Fixed(w.max(1.0)))
                            .height(Length::Fixed(h.max(1.0)))
                            .style(frame_style)
                            .into(),
                    ));

                    let pts = [
                        (l, t),
                        (l + w / 2.0, t),
                        (l + w, t),
                        (l, t + h / 2.0),
                        (l + w, t + h / 2.0),
                        (l, t + h),
                        (l + w / 2.0, t + h),
                        (l + w, t + h),
                    ];
                    for (px, py) in pts {
                        layers.push(abs(
                            px - HANDLE / 2.0,
                            py - HANDLE / 2.0,
                            container("")
                                .width(Length::Fixed(HANDLE))
                                .height(Length::Fixed(HANDLE))
                                .style(handle_style)
                                .into(),
                        ));
                    }

                    let (pw, ph) = ((e.w().round() as u32) & !1, (e.h().round() as u32) & !1);
                    let small = pw < MIN_DIM || ph < MIN_DIM;
                    let label = if small {
                        format!("{pw} x {ph}  (too small, min {MIN_DIM} x {MIN_DIM})")
                    } else {
                        format!("{pw} x {ph}   at ({}, {})", e.l.round() as i32, e.t.round() as i32)
                    };
                    let (lx, ly) = if t >= 32.0 { (l, t - 30.0) } else { (l + 6.0, t + 6.0) };
                    layers.push(abs(
                        lx,
                        ly,
                        container(text(label).size(14).color(Color::WHITE))
                            .padding(5)
                            .style(label_style)
                            .into(),
                    ));
                }
            }

            layers.push(
                mouse_area(container("").width(Length::Fill).height(Length::Fill))
                    .on_press(SelMsg::Pressed)
                    .on_release(SelMsg::Released)
                    .on_move(move |p| SelMsg::Moved(p, size))
                    .interaction(iced::mouse::Interaction::Crosshair)
                    .into(),
            );

            let bar = container(
                row![
                    text("Drag to draw, inside to move, edges to resize").size(13),
                    button(text("Select all").size(13))
                        .on_press(SelMsg::All)
                        .padding(6)
                        .style(button::secondary),
                    button(text("Confirm (Enter)").size(13))
                        .on_press(SelMsg::Confirm)
                        .padding(6)
                        .style(button::success),
                    button(text("Cancel (Esc)").size(13))
                        .on_press(SelMsg::Cancel)
                        .padding(6)
                        .style(button::danger),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
            )
            .padding(10)
            .style(bar_style);
            layers.push(
                container(bar)
                    .center_x(Length::Fill)
                    .padding(Padding {
                        top: 14.0,
                        ..Padding::ZERO
                    })
                    .into(),
            );

            Stack::with_children(layers)
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        })
        .into()
    }
}

fn abs(x: f32, y: f32, child: Element<'static, SelMsg>) -> Element<'static, SelMsg> {
    container(child)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(Padding {
            top: y.max(0.0),
            left: x.max(0.0),
            right: 0.0,
            bottom: 0.0,
        })
        .into()
}

fn dim(w: f32, h: f32) -> Element<'static, SelMsg> {
    container("")
        .width(Length::Fixed(w.max(0.0)))
        .height(Length::Fixed(h.max(0.0)))
        .style(dim_style)
        .into()
}

fn dim_style(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.55))),
        ..Default::default()
    }
}

fn frame_style(_: &Theme) -> container::Style {
    container::Style {
        border: Border {
            color: ACCENT,
            width: 2.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    }
}

fn handle_style(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::WHITE)),
        border: Border {
            color: ACCENT,
            width: 1.5,
            radius: 2.0.into(),
        },
        ..Default::default()
    }
}

fn label_style(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.8))),
        border: Border {
            radius: 4.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn bar_style(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba(0.08, 0.08, 0.10, 0.92))),
        border: Border {
            color: Color::from_rgba(1.0, 1.0, 1.0, 0.15),
            width: 1.0,
            radius: 8.0.into(),
        },
        ..Default::default()
    }
}
