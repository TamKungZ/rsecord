use crate::app::App;
use crate::app::message::{Field, Flag, Message};
use iced::{Alignment, Element, Length, Theme};
use iced::widget::{button, container, row, text, text_input, Column};

impl App {
    pub(super) fn num_in(&self, f: Field, hint: &'static str, w: f32) -> Element<'_, Message> {
        text_input(hint, self.txt(f))
            .on_input(move |s| Message::Edit(f, s))
            .padding(5)
            .size(13)
            .width(Length::Fixed(w))
            .into()
    }
}

pub(super) fn group<'a>(
    title: &'static str,
    hint: &'static str,
    rows: Vec<Element<'a, Message>>,
) -> Element<'a, Message> {
    let mut c = Column::new().spacing(8).push(text(title).size(16));
    if !hint.is_empty() {
        c = c.push(text(hint).size(11).style(text::secondary));
    }
    for r in rows {
        c = c.push(r);
    }
    container(c)
        .padding(12)
        .width(Length::Fill)
        .style(container::rounded_box)
        .into()
}

pub(super) fn field_row<'a>(label: &'static str, input: Element<'a, Message>) -> Element<'a, Message> {
    row![lbl(label), input]
        .spacing(8)
        .align_y(Alignment::Center)
        .into()
}

pub(super) fn info_row(label: &'static str, value: String) -> Element<'static, Message> {
    row![
        text(label).size(13).width(Length::Fixed(70.0)),
        text(value).size(13)
    ]
    .spacing(8)
    .into()
}

pub(super) fn lbl(label: &'static str) -> Element<'static, Message> {
    text(label).size(13).width(Length::Fixed(130.0)).into()
}

pub(super) fn toggle(label: &'static str, on: bool, flag: Flag) -> Element<'static, Message> {
    button(
        text(format!("{label}: {}", if on { "ON" } else { "OFF" }))
            .size(12)
            .center(),
    )
    .on_press(Message::Toggle(flag))
    .width(Length::Fill)
    .padding(6)
    .style(move |t: &Theme, st| {
        if on {
            button::success(t, st)
        } else {
            button::secondary(t, st)
        }
    })
    .into()
}
