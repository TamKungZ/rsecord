use crate::app::App;
use crate::app::message::{Field, Flag, Message};
use crate::theme::{self, Tone};
use iced::widget::{button, column, container, row, text, text_input, toggler, Column};
use iced::{Alignment, Element, Length, Padding};

pub(super) const LABEL_W: f32 = 140.0;

impl App {
    /// A number input that turns red when its value is not accepted.
    pub(super) fn num_in(&self, f: Field, hint: &'static str, w: f32) -> Element<'_, Message> {
        let bad = self.field_error(f).is_some();
        text_input(hint, self.txt(f))
            .on_input(move |s| Message::Edit(f, s))
            .padding(Padding::from([6.0, 8.0]))
            .size(14)
            .width(Length::Fixed(w))
            .style(move |t, st| theme::input(t, st, bad))
            .into()
    }
}

/// A titled card holding related rows.
pub(super) fn group<'a>(
    title: &'static str,
    hint: &'static str,
    rows: Vec<Element<'a, Message>>,
) -> Element<'a, Message> {
    let mut head = Column::new()
        .spacing(2)
        .push(text(title).size(15).font(theme::BOLD));
    if !hint.is_empty() {
        head = head.push(text(hint).size(12).style(theme::muted));
    }
    let mut c = Column::new().spacing(12).push(head);
    for r in rows {
        c = c.push(r);
    }
    container(c)
        .padding(14)
        .width(Length::Fill)
        .style(theme::card)
        .into()
}

/// `label` on the left, the control on the right, and the problem (if any) below.
pub(super) fn field_row<'a>(
    label: &'static str,
    input: Element<'a, Message>,
    error: Option<String>,
) -> Element<'a, Message> {
    let line = row![lbl(label), input]
        .spacing(8)
        .align_y(Alignment::Center);
    let mut c = Column::new().spacing(4).push(line);
    if let Some(e) = error {
        c = c.push(error_line(e));
    }
    c.into()
}

pub(super) fn error_line<'a>(e: String) -> Element<'a, Message> {
    container(text(e).size(12).style(theme::tone_text(Tone::Danger)))
        .padding(Padding {
            left: LABEL_W + 8.0,
            ..Padding::ZERO
        })
        .into()
}

pub(super) fn kv_row(label: &'static str, value: String) -> Element<'static, Message> {
    row![
        text(label)
            .size(13)
            .style(theme::muted)
            .width(Length::Fixed(78.0)),
        text(value).size(13).width(Length::Fill)
    ]
    .spacing(8)
    .into()
}

pub(super) fn lbl(label: &'static str) -> Element<'static, Message> {
    text(label).size(13).width(Length::Fixed(LABEL_W)).into()
}

/// A labelled on/off switch with a one-line explanation.
pub(super) fn switch(
    label: &'static str,
    hint: &'static str,
    on: bool,
    flag: Flag,
) -> Element<'static, Message> {
    let info = column![
        text(label).size(14),
        text(hint).size(12).style(theme::muted)
    ]
    .spacing(2)
    .width(Length::Fill);
    row![info, toggler(on).on_toggle(move |_| Message::Toggle(flag))]
        .spacing(12)
        .align_y(Alignment::Center)
        .into()
}

/// A row of mutually exclusive options.
pub(super) fn segmented<'a, T: Copy + PartialEq + 'a>(
    items: &[(&'static str, T)],
    active: T,
    msg: impl Fn(T) -> Message + 'a,
) -> Element<'a, Message> {
    let mut r = row![].spacing(6);
    for (label, value) in items {
        let on = *value == active;
        r = r.push(
            button(text(*label).size(13).center())
                .on_press(msg(*value))
                .width(Length::Fill)
                .padding(8)
                .style(theme::chip(on)),
        );
    }
    r.into()
}

/// A coloured message strip, with an optional action on the right.
pub(super) fn banner<'a>(
    tone: Tone,
    message: String,
    action: Option<(&'static str, Message)>,
) -> Element<'a, Message> {
    let mut r = row![text(message).size(13).width(Length::Fill)]
        .spacing(10)
        .align_y(Alignment::Center);
    if let Some((label, msg)) = action {
        r = r.push(
            button(text(label).size(12).center())
                .on_press(msg)
                .padding(Padding::from([5.0, 10.0]))
                .style(theme::ghost),
        );
    }
    container(r)
        .padding(Padding::from([10.0, 12.0]))
        .width(Length::Fill)
        .style(theme::banner(tone))
        .into()
}
