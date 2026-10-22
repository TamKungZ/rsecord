use crate::app::App;
use crate::app::message::{Message, Page, Tab};
use iced::{Alignment, Element, Length, Padding, Theme};
use iced::widget::{button, container, row, scrollable, text, Column, Row};

impl App {
    pub(super) fn settings_view(&self, tab: Tab) -> Element<'_, Message> {
        let top = row![
            button(text("< Back").size(13))
                .on_press(Message::Page(Page::Record))
                .padding(7)
                .style(button::secondary),
            text("Settings").size(22),
        ]
        .spacing(12)
        .align_y(Alignment::Center);

        let tabs = Tab::ALL.iter().fold(Row::new().spacing(6), |r, t| {
            let t = *t;
            let active = tab == t;
            r.push(
                button(text(t.label()).size(13).center())
                    .on_press(Message::Page(Page::Settings(t)))
                    .width(Length::Fill)
                    .padding(7)
                    .style(move |th: &Theme, st| {
                        if active {
                            button::primary(th, st)
                        } else {
                            button::secondary(th, st)
                        }
                    }),
            )
        });

        let content = match tab {
            Tab::Capture => self.tab_capture(),
            Tab::Video => self.tab_video(),
            Tab::Encoder => self.tab_encoder(),
            Tab::Audio => self.tab_audio(),
        };

        let mut col = Column::new().spacing(12).height(Length::Fill).push(top).push(tabs);
        if let Some(e) = &self.error {
            col = col.push(
                text(format!("Invalid value - {e}"))
                    .size(12)
                    .style(text::danger),
            );
        }
        col = col.push(
            scrollable(container(content).padding(Padding {
                right: 14.0,
                ..Padding::ZERO
            }))
            .height(Length::Fill),
        );
        col = col.push(text(self.summary()).size(12));

        container(col).padding(18).into()
    }
}
