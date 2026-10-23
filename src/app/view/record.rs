use crate::app::App;
use crate::app::message::{CaptureMode, Message, Page, State, Tab};
use super::widgets::{banner, group, kv_row, segmented};
use crate::recorder::AudioSource;
use crate::theme::{self, Tone};
use iced::widget::{button, column, container, pick_list, row, scrollable, text, Column, Space};
use iced::{Alignment, Element, Font, Length, Padding};

impl App {
    pub(super) fn record_view(&self) -> Element<'_, Message> {
        let idle = matches!(self.state, State::Idle);
        let problem = if idle { self.problem() } else { None };

        let mut col = Column::new()
            .spacing(14)
            .width(Length::Fill)
            .push(self.header())
            .push(self.clock())
            .push(self.main_button(problem.is_some()));

        col = col.push(self.status_banner());
        if let Some(p) = problem {
            col = col.push(banner(Tone::Warning, p.text, Some((p.label, p.action))));
        }

        if idle {
            col = col.push(self.capture_card());
        } else {
            col = col.push(banner(
                Tone::Neutral,
                "Settings are locked while recording.".into(),
                None,
            ));
        }
        col = col.push(self.output_card());

        let body = container(col).max_width(560).padding(Padding {
            top: 18.0,
            right: 18.0,
            bottom: 18.0,
            left: 18.0,
        });
        scrollable(container(body).center_x(Length::Fill))
            .height(Length::Fill)
            .into()
    }

    fn header(&self) -> Element<'_, Message> {
        let badge: Element<'_, Message> = match self.state {
            State::Idle => badge(Tone::Neutral, None, "Ready"),
            State::Recording { .. } => {
                // Blink the dot about once a second.
                let lit = (self.elapsed.as_millis() / 500) % 2 == 0;
                badge(Tone::Danger, Some(lit), "REC")
            }
            State::Saving => badge(Tone::Warning, None, "Saving"),
        };
        let mut r = row![
            text("rsecord").size(20).font(theme::BOLD),
            badge,
            Space::new().width(Length::Fill),
        ]
        .spacing(10)
        .align_y(Alignment::Center);
        if matches!(self.state, State::Idle) {
            r = r.push(
                button(text("Settings").size(13))
                    .on_press(Message::Page(Page::Settings(Tab::Capture)))
                    .padding(Padding::from([6.0, 12.0]))
                    .style(theme::ghost),
            );
        }
        r.into()
    }

    fn clock(&self) -> Element<'_, Message> {
        let secs = self.elapsed.as_secs();
        let t = text(format!(
            "{:02}:{:02}:{:02}",
            secs / 3600,
            (secs / 60) % 60,
            secs % 60
        ))
        .size(60)
        .font(Font::MONOSPACE);
        let t = if matches!(self.state, State::Idle) {
            t.style(theme::muted)
        } else {
            t
        };
        container(t).center_x(Length::Fill).padding([8, 0]).into()
    }

    fn main_button(&self, blocked: bool) -> Element<'_, Message> {
        let (label, msg, tone) = match self.state {
            State::Idle => (
                "Start recording",
                (!blocked).then_some(Message::Start),
                Tone::Danger,
            ),
            State::Recording { .. } => ("Stop and save", Some(Message::Stop), Tone::Neutral),
            State::Saving => ("Saving...", None, Tone::Neutral),
        };
        button(text(label).size(18).font(theme::BOLD).width(Length::Fill).center())
            .on_press_maybe(msg)
            .width(Length::Fill)
            .padding(16)
            .style(theme::solid(tone))
            .into()
    }

    fn status_banner(&self) -> Element<'_, Message> {
        let action = (self.status.tone == Tone::Success && self.saved.is_some())
            .then_some(("Open folder", Message::OpenFolder));
        banner(self.status.tone, self.status.text.clone(), action)
    }

    /// Quick access to the two choices people change most.
    fn capture_card(&self) -> Element<'_, Message> {
        let area: Element<'_, Message> = match (self.mode, self.region) {
            (CaptureMode::Region, Some(r)) => row![
                text(format!("{} x {} at ({}, {})", r.w, r.h, r.x, r.y))
                    .size(13)
                    .width(Length::Fill),
                button(text("Change area").size(12))
                    .on_press(Message::SelectArea)
                    .padding(Padding::from([5.0, 10.0]))
                    .style(theme::ghost),
            ]
            .align_y(Alignment::Center)
            .into(),
            (CaptureMode::Region, None) => button(text("Select area on screen...").width(Length::Fill).center())
                .on_press(Message::SelectArea)
                .width(Length::Fill)
                .padding(10)
                .style(theme::solid(Tone::Primary))
                .into(),
            _ => text(
                self.display
                    .as_ref()
                    .map(|d| d.label.clone())
                    .unwrap_or_else(|| "No display found".into()),
            )
            .size(13)
            .style(theme::muted)
            .into(),
        };

        let audio = row![
            text("Audio").size(13).width(Length::Fixed(60.0)),
            pick_list(AudioSource::all(), Some(self.source), Message::Source).text_size(14).padding(Padding::from([6.0, 8.0])).width(Length::Fill),
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        group(
            "Capture",
            "",
            vec![
                segmented(
                    &[
                        ("Full screen", CaptureMode::Full),
                        ("Selected area", CaptureMode::Region),
                    ],
                    self.mode,
                    Message::Mode,
                ),
                area,
                audio.into(),
            ],
        )
    }

    fn output_card(&self) -> Element<'_, Message> {
        let s = &self.settings;
        let (size, rate) = self.summary_parts();
        let rows = column![
            kv_row("Output", size),
            kv_row("Bitrate", rate),
            kv_row("Preset", s.preset.to_string()),
        ]
        .spacing(6);
        group("Expected output", "", vec![rows.into()])
    }
}

/// A small status badge; `dot` adds a dot that is lit or dim.
fn badge(tone: Tone, dot: Option<bool>, label: &'static str) -> Element<'static, Message> {
    let mut r = row![].spacing(6).align_y(Alignment::Center);
    if let Some(lit) = dot {
        r = r.push(
            container(Space::new())
                .width(8)
                .height(8)
                .style(move |t: &iced::Theme| {
                    let c = tone.fg(t);
                    theme::dot(if lit { c } else { c.scale_alpha(0.25) })(t)
                }),
        );
    }
    r = r.push(text(label).size(12).font(theme::BOLD));
    container(r)
        .padding(Padding::from([3.0, 10.0]))
        .style(theme::pill(tone))
        .into()
}
