use super::{SelMsg, Selector};
use crate::settings::MIN_DIM;
use iced::{Alignment, Background, Border, Color, ContentFit, Element, Length, Padding, Size, Theme};
use iced::widget::{button, container, image, mouse_area, responsive, row, text, Stack};

const HANDLE: f32 = 10.0;
const ACCENT: Color = Color::from_rgb(0.20, 0.62, 1.0);

impl Selector {
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
