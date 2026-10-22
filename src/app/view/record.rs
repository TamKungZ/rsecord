use crate::app::App;
use crate::app::message::{CaptureMode, Message, Page, State, Tab};
use super::widgets::info_row;
use iced::{Alignment, Element, Length};
use iced::widget::{button, container, text, Column};

impl App {
    pub(super) fn record_view(&self) -> Element<'_, Message> {
        let secs = self.elapsed.as_secs();
        let clock = text(format!(
            "{:02}:{:02}:{:02}",
            secs / 3600,
            (secs / 60) % 60,
            secs % 60
        ))
        .size(44);

        let idle = matches!(self.state, State::Idle);

        let main_btn = match self.state {
            State::Idle => button(text("Start Recording").center())
                .on_press(Message::Start)
                .style(button::danger),
            State::Recording { .. } => button(text("Stop").center())
                .on_press(Message::Stop)
                .style(button::secondary),
            State::Saving => button(text("Saving...").center()),
        }
        .width(Length::Fill)
        .padding(12);

        let mut col = Column::new()
            .spacing(14)
            .width(Length::Fill)
            .align_x(Alignment::Center)
            .push(clock)
            .push(main_btn)
            .push(text(&self.status).size(13));

        if idle {
            let s = &self.settings;
            let display = self
                .display
                .as_ref()
                .map(|d| d.label.clone())
                .unwrap_or_else(|| "No display found".to_string());
            let area = match (self.mode, self.region) {
                (CaptureMode::Region, Some(r)) => {
                    format!("Area {}x{} at ({}, {})", r.w, r.h, r.x, r.y)
                }
                (CaptureMode::Region, None) => "Selected area (not set yet)".to_string(),
                _ => "Full screen".to_string(),
            };
            let card = container(
                Column::new()
                    .spacing(8)
                    .push(info_row("Display", display))
                    .push(info_row("Capture", area))
                    .push(info_row("Audio", self.source.to_string()))
                    .push(info_row(
                        "Video",
                        format!("{} preset, {}, {} fps", s.preset, s.res_mode, s.fps),
                    )),
            )
            .padding(14)
            .width(Length::Fill)
            .style(container::rounded_box);
            col = col.push(card);
            col = col.push(text(self.summary()).size(12));

            if self.mode == CaptureMode::Region {
                col = col.push(
                    button(text("Select area on screen...").center())
                        .on_press(Message::SelectArea)
                        .width(Length::Fill)
                        .padding(10)
                        .style(button::primary),
                );
            }
            col = col.push(
                button(text("Settings").center())
                    .on_press(Message::Page(Page::Settings(Tab::Capture)))
                    .width(Length::Fill)
                    .padding(10)
                    .style(button::secondary),
            );
        } else {
            col = col.push(text("Settings are locked while recording.").size(13));
            col = col.push(text(self.summary()).size(12));
        }

        container(col).padding(18).into()
    }
}
