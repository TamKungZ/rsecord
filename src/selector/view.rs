use super::{SelMsg, Selector};
use crate::settings::MIN_DIM;
use crate::theme::{self, Tone};
use iced::widget::{button, container, image, mouse_area, responsive, row, text, Stack};
use iced::{Alignment, Background, Border, Color, ContentFit, Element, Length, Padding, Size, Theme};

const HANDLE: f32 = 10.0;

impl Selector {
    pub fn view(&self) -> Element<'_, SelMsg> {
        let handle = self.handle.clone();
        let rect = self.rect;
        let can_confirm = self.can_confirm();
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

            let mut too_small = false;
            match rect {
                None => {
                    layers.push(abs(0.0, 0.0, dim(size.width, size.height)));
                    layers.push(
                        container(
                            container(
                                text("Drag anywhere to draw the area to record")
                                    .size(22)
                                    .style(|t: &Theme| iced::widget::text::Style {
                                        color: Some(theme::on(overlay_bg(t))),
                                    }),
                            )
                            .padding(Padding::from([12.0, 20.0]))
                            .style(|t: &Theme| container::Style {
                                background: Some(Background::Color(overlay_bg(t))),
                                border: iced::border::rounded(10),
                                ..container::Style::default()
                            }),
                        )
                        .center(Length::Fill)
                        .into(),
                    );
                }
                Some(e) => {
                    let (sx, sy) = (size.width / iw, size.height / ih);
                    let (l, t) = (e.l * sx, e.t * sy);
                    let (w, h) = (e.w() * sx, e.h() * sy);

                    layers.push(abs(0.0, 0.0, dim(size.width, t)));
                    layers.push(abs(0.0, t + h, dim(size.width, size.height - t - h)));
                    layers.push(abs(0.0, t, dim(l, h)));
                    layers.push(abs(l + w, t, dim(size.width - l - w, h)));

                    let (pw, ph) = ((e.w().round() as u32) & !1, (e.h().round() as u32) & !1);
                    too_small = pw < MIN_DIM || ph < MIN_DIM;

                    layers.push(abs(
                        l,
                        t,
                        container("")
                            .width(Length::Fixed(w.max(1.0)))
                            .height(Length::Fixed(h.max(1.0)))
                            .style(move |th: &Theme| frame_style(th, too_small))
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
                                .style(move |th: &Theme| handle_style(th, too_small))
                                .into(),
                        ));
                    }

                    let label = if too_small {
                        format!("{pw} x {ph}  (too small, min {MIN_DIM} x {MIN_DIM})")
                    } else {
                        format!("{pw} x {ph}   at ({}, {})", e.l.round() as i32, e.t.round() as i32)
                    };
                    let (lx, ly) = if t >= 36.0 { (l, t - 34.0) } else { (l + 6.0, t + 6.0) };
                    layers.push(abs(
                        lx,
                        ly,
                        container(text(label).size(14))
                            .padding(Padding::from([5.0, 8.0]))
                            .style(move |th: &Theme| label_style(th, too_small))
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

            let hint = if too_small {
                "The area is too small to record"
            } else {
                "Drag to draw, inside to move, edges to resize"
            };
            let bar = container(
                row![
                    text(hint).size(13),
                    button(text("Select all").size(13))
                        .on_press(SelMsg::All)
                        .padding(7)
                        .style(theme::ghost),
                    button(text("Confirm (Enter)").size(13))
                        .on_press_maybe(can_confirm.then_some(SelMsg::Confirm))
                        .padding(7)
                        .style(theme::solid(Tone::Success)),
                    button(text("Cancel (Esc)").size(13))
                        .on_press(SelMsg::Cancel)
                        .padding(7)
                        .style(theme::ghost),
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

/// The selection colour. It sits on a screenshot, so it is made readable
/// against the black dimming rather than against the page background.
fn accent(theme: &Theme, too_small: bool) -> Color {
    let tone = if too_small { Tone::Danger } else { Tone::Primary };
    theme::legible(tone.fill(theme), Color::BLACK)
}

fn overlay_bg(theme: &Theme) -> Color {
    theme.extended_palette().background.base.color.scale_alpha(0.94)
}

fn dim_style(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.55))),
        ..Default::default()
    }
}

fn frame_style(theme: &Theme, too_small: bool) -> container::Style {
    container::Style {
        border: Border {
            color: accent(theme, too_small),
            width: 2.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    }
}

fn handle_style(theme: &Theme, too_small: bool) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::WHITE)),
        border: Border {
            color: accent(theme, too_small),
            width: 2.0,
            radius: 2.0.into(),
        },
        ..Default::default()
    }
}

fn label_style(theme: &Theme, too_small: bool) -> container::Style {
    let ep = theme.extended_palette();
    container::Style {
        background: Some(Background::Color(overlay_bg(theme))),
        text_color: Some(if too_small {
            Tone::Danger.fg(theme)
        } else {
            ep.background.base.text
        }),
        border: Border {
            color: accent(theme, too_small),
            width: 1.0,
            radius: 6.0.into(),
        },
        ..Default::default()
    }
}

fn bar_style(theme: &Theme) -> container::Style {
    let ep = theme.extended_palette();
    container::Style {
        background: Some(Background::Color(overlay_bg(theme))),
        text_color: Some(ep.background.base.text),
        border: Border {
            color: ep.background.strong.color,
            width: 1.0,
            radius: 10.0.into(),
        },
        ..Default::default()
    }
}
