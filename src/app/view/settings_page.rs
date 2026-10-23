use crate::app::App;
use crate::app::message::{Message, Page, Tab};
use super::widgets::banner;
use crate::theme::{self, Tone};
use iced::widget::{button, container, row, scrollable, text, Column, Row, Space};
use iced::{Alignment, Element, Length, Padding};

impl App {
    pub(super) fn settings_view(&self, tab: Tab) -> Element<'_, Message> {
        let top = row![
            button(text("Back").size(13))
                .on_press(Message::Page(Page::Record))
                .padding(Padding::from([6.0, 12.0]))
                .style(theme::ghost),
            text("Settings").size(22).font(theme::BOLD),
        ]
        .spacing(12)
        .align_y(Alignment::Center);

        let tabs = Tab::ALL.iter().fold(Row::new().spacing(4), |r, t| {
            let t = *t;
            r.push(
                button(text(t.label()).size(13).center())
                    .on_press(Message::Page(Page::Settings(t)))
                    .width(Length::Fill)
                    .padding(8)
                    .style(theme::tab(tab == t)),
            )
        });

        let content = match tab {
            Tab::Capture => self.tab_capture(),
            Tab::Video => self.tab_video(),
            Tab::Encoder => self.tab_encoder(),
            Tab::Audio => self.tab_audio(),
            Tab::Theme => self.tab_theme(),
        };

        let mut col = Column::new()
            .spacing(12)
            .height(Length::Fill)
            .push(top)
            .push(tabs);

        // Say so when the problem is on another tab, and offer a way there.
        if let Some(p) = self.problem() {
            let here = match &p.action {
                Message::Page(Page::Settings(t)) => *t == tab,
                Message::SelectArea => tab == Tab::Capture,
                _ => false,
            };
            if !here {
                col = col.push(banner(Tone::Warning, p.text, Some((p.label, p.action))));
            }
        }

        col = col.push(
            scrollable(container(content).padding(Padding {
                right: 14.0,
                ..Padding::ZERO
            }))
            .height(Length::Fill),
        );
        col = col.push(self.footer());

        container(col)
            .padding(18)
            .max_width(640)
            .into()
    }

    fn footer(&self) -> Element<'_, Message> {
        let reset: Element<'_, Message> = if self.confirm_reset {
            row![
                text("Reset video settings?").size(12),
                button(text("Cancel").size(12))
                    .on_press(Message::ResetCancel)
                    .padding(Padding::from([5.0, 10.0]))
                    .style(theme::ghost),
                button(text("Reset").size(12))
                    .on_press(Message::ResetConfirm)
                    .padding(Padding::from([5.0, 10.0]))
                    .style(theme::solid(Tone::Danger)),
            ]
            .spacing(8)
            .align_y(Alignment::Center)
            .into()
        } else {
            button(text("Reset to defaults").size(12))
                .on_press(Message::ResetAsk)
                .padding(Padding::from([5.0, 10.0]))
                .style(theme::ghost)
                .into()
        };
        let (size, rate) = self.summary_parts();
        row![
            text(format!("{size}\n{rate}"))
                .size(12)
                .style(theme::muted)
                .width(Length::Fill),
            Space::new().width(8),
            reset,
        ]
        .align_y(Alignment::Center)
        .into()
    }
}
