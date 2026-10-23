use crate::app::App;
use crate::app::message::{CaptureMode, Field, Flag, Message};
use crate::recorder::AudioSource;
use crate::settings::{AppTheme, Content, Effort, Filter, Preset, RateMode, ResMode};
use super::widgets::{field_row, group, segmented, switch};
use crate::theme::{self, Tone};
use iced::widget::{button, column, container, pick_list, row, text, Column, Space};
use iced::{Alignment, Element, Length, Padding};

impl App {
    /// The first error among several fields, shown once under the row.
    fn first_error(&self, fields: &[Field]) -> Option<String> {
        fields.iter().find_map(|f| self.field_error(*f))
    }

    pub(super) fn tab_capture(&self) -> Column<'_, Message> {
        let display_row: Element<'_, Message> = row![
            pick_list(
                self.displays.clone(),
                self.display.clone(),
                Message::Display
            )
            .text_size(14)
            .padding(Padding::from([6.0, 8.0]))
            .width(Length::Fill),
            button(text("Refresh").size(12))
                .on_press(Message::RefreshDisplays)
                .padding(Padding::from([6.0, 10.0]))
                .style(theme::ghost),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into();

        let mut area_rows: Vec<Element<'_, Message>> = vec![segmented(
            &[
                ("Full screen", CaptureMode::Full),
                ("Selected area", CaptureMode::Region),
            ],
            self.mode,
            Message::Mode,
        )];

        if self.mode == CaptureMode::Region {
            let coord = |label: &'static str, f: Field| -> Element<'_, Message> {
                column![
                    text(label).size(12).style(theme::muted),
                    self.num_in(f, "0", 76.0)
                ]
                .spacing(3)
                .into()
            };
            area_rows.push(
                row![
                    coord("X", Field::RegX),
                    coord("Y", Field::RegY),
                    coord("Width", Field::RegW),
                    coord("Height", Field::RegH),
                ]
                .spacing(8)
                .into(),
            );
            if let Some(e) = self.first_error(&[Field::RegX, Field::RegY, Field::RegW, Field::RegH])
            {
                area_rows.push(
                    text(e)
                        .size(12)
                        .style(theme::tone_text(Tone::Danger))
                        .into(),
                );
            }
            area_rows.push(
                button(text("Select area on screen...").width(Length::Fill).center())
                    .on_press(Message::SelectArea)
                    .width(Length::Fill)
                    .padding(9)
                    .style(theme::solid(Tone::Primary))
                    .into(),
            );
            area_rows.push(
                text(match self.region {
                    Some(r) => format!(
                        "Recording {}x{} px starting at ({}, {}) on the chosen display. \
                         Width and height are rounded down to even numbers.",
                        r.w, r.h, r.x, r.y
                    ),
                    None => "No area yet. Use the button above, or type X, Y, width and height in pixels."
                        .to_string(),
                })
                .size(12)
                .style(theme::muted)
                .into(),
            );
        }

        Column::new()
            .spacing(14)
            .push(group(
                "Display",
                "Which screen to record.",
                vec![display_row],
            ))
            .push(group(
                "Capture area",
                "Record the whole display, or only a region you draw on screen.",
                area_rows,
            ))
    }

    pub(super) fn tab_video(&self) -> Column<'_, Message> {
        let s = &self.settings;

        let current_fps = self.txt(Field::Fps).trim().parse::<u32>().ok();
        let fps_btn = |n: u32| {
            button(text(n.to_string()).size(12).center())
                .on_press(Message::SetFps(n))
                .padding(Padding::from([6.0, 10.0]))
                .style(theme::chip(current_fps == Some(n)))
        };
        let mut fps_col = Column::new().spacing(4).push(
            row![
                self.num_in(Field::Fps, "30", 70.0),
                fps_btn(15),
                fps_btn(24),
                fps_btn(30),
                fps_btn(60),
                fps_btn(120),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        );
        if let Some(e) = self.field_error(Field::Fps) {
            fps_col = fps_col.push(text(e).size(12).style(theme::tone_text(Tone::Danger)));
        }
        let fps_row: Element<'_, Message> = fps_col.into();

        let mut res_rows: Vec<Element<'_, Message>> = vec![pick_list(
            ResMode::ALL,
            Some(s.res_mode),
            Message::ResMode,
        )
        .text_size(14)
        .padding(Padding::from([6.0, 8.0]))
        .width(Length::Fill)
        .into()];
        match s.res_mode {
            ResMode::Percent => res_rows.push(field_row(
                "Scale (%)",
                self.num_in(Field::Percent, "100", 90.0),
                self.field_error(Field::Percent),
            )),
            ResMode::Custom => {
                res_rows.push(field_row(
                    "Size (px)",
                    row![
                        self.num_in(Field::Width, "1920", 80.0),
                        text("x").size(13),
                        self.num_in(Field::Height, "1080", 80.0),
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center)
                    .into(),
                    self.first_error(&[Field::Width, Field::Height]),
                ));
                res_rows.push(switch(
                    "Keep aspect ratio",
                    "Fit inside the size above without stretching.",
                    s.keep_aspect,
                    Flag::KeepAspect,
                ));
            }
            _ => {}
        }
        res_rows.push(field_row(
            "Scaling filter",
            pick_list(Filter::ALL, Some(s.filter), Message::Filter)
                .text_size(14)
                .padding(Padding::from([6.0, 8.0]))
                .width(Length::Fill)
                .into(),
            None,
        ));

        Column::new()
            .spacing(14)
            .push(group(
                "Preset",
                "One-click starting points. Changing any value afterwards switches to Custom.",
                vec![pick_list(Preset::ALL, Some(s.preset), Message::Preset)
                    .text_size(14)
                    .padding(Padding::from([6.0, 8.0]))
                    .width(Length::Fill)
                    .into()],
            ))
            .push(group(
                "Frame rate",
                "Frames per second, 1-120.",
                vec![fps_row],
            ))
            .push(group(
                "Resolution",
                "Output size. Fixed heights never upscale; the filter is used only when resizing.",
                res_rows,
            ))
    }

    pub(super) fn tab_encoder(&self) -> Column<'_, Message> {
        let s = &self.settings;

        let mut rate_rows: Vec<Element<'_, Message>> = vec![
            field_row(
                "Rate control",
                pick_list(RateMode::ALL, Some(s.rate), Message::Rate)
                    .text_size(14)
                    .padding(Padding::from([6.0, 8.0]))
                    .width(Length::Fill)
                    .into(),
                None,
            ),
            switch(
                "Auto bitrate",
                "Derive the bitrate from bits per pixel.",
                s.auto_bitrate,
                Flag::AutoBitrate,
            ),
        ];
        if s.auto_bitrate {
            rate_rows.push(field_row(
                "Bits per pixel",
                self.num_in(Field::Bpp, "0.10", 90.0),
                self.field_error(Field::Bpp),
            ));
        } else {
            rate_rows.push(field_row(
                "Bitrate (kbps)",
                self.num_in(Field::Bitrate, "6000", 90.0),
                self.field_error(Field::Bitrate),
            ));
        }
        rate_rows.push(field_row(
            "QP range (0-51)",
            row![
                self.num_in(Field::QpMin, "0", 60.0),
                text("to").size(13),
                self.num_in(Field::QpMax, "51", 60.0),
            ]
            .spacing(8)
            .align_y(Alignment::Center)
            .into(),
            self.first_error(&[Field::QpMin, Field::QpMax]),
        ));

        let tune_rows: Vec<Element<'_, Message>> = vec![
            field_row(
                "Content type",
                pick_list(Content::ALL, Some(s.content), Message::Content)
                    .text_size(14)
                    .padding(Padding::from([6.0, 8.0]))
                    .width(Length::Fill)
                    .into(),
                None,
            ),
            field_row(
                "Effort",
                pick_list(Effort::ALL, Some(s.effort), Message::Effort)
                    .text_size(14)
                    .padding(Padding::from([6.0, 8.0]))
                    .width(Length::Fill)
                    .into(),
                None,
            ),
            field_row(
                "Keyframe every (s)",
                self.num_in(Field::Keyframe, "2", 90.0),
                self.field_error(Field::Keyframe),
            ),
            field_row(
                "Threads (0 = auto)",
                self.num_in(Field::Threads, "0", 90.0),
                self.field_error(Field::Threads),
            ),
        ];

        let adv_rows: Vec<Element<'_, Message>> = vec![
            switch(
                "Skip frames",
                "Let the encoder drop frames to hold the bitrate.",
                s.skip_frames,
                Flag::SkipFrames,
            ),
            switch(
                "Adaptive quantization",
                "Spend more bits on detailed areas.",
                s.adaptive_quant,
                Flag::AdaptiveQuant,
            ),
            switch(
                "Background detection",
                "Save bits on static parts of the screen.",
                s.bg_detect,
                Flag::BgDetect,
            ),
            switch(
                "Scene-change detection",
                "Insert a keyframe when the picture changes abruptly.",
                s.scene_detect,
                Flag::SceneDetect,
            ),
        ];

        Column::new()
            .spacing(14)
            .push(group(
                "Bitrate and quality",
                "Lower QP means better quality and larger files.",
                rate_rows,
            ))
            .push(group(
                "Encoder tuning",
                "Trade CPU time for quality, and set how often a full frame is written.",
                tune_rows,
            ))
            .push(group(
                "Advanced switches",
                "Defaults suit most screen recordings.",
                adv_rows,
            ))
    }

    pub(super) fn tab_audio(&self) -> Column<'_, Message> {
        let quality: Element<'_, Message> = if self.source == AudioSource::Off {
            text("Audio is off, so the bitrate is not used.")
                .size(13)
                .style(theme::muted)
                .into()
        } else {
            field_row(
                "AAC bitrate (kbps)",
                self.num_in(Field::AudioKbps, "128", 90.0),
                self.field_error(Field::AudioKbps),
            )
        };
        Column::new()
            .spacing(14)
            .push(group(
                "Audio source",
                "None, microphone, or system audio.",
                vec![pick_list(AudioSource::all(), Some(self.source), Message::Source)
                    .text_size(14)
                    .padding(Padding::from([6.0, 8.0]))
                    .width(Length::Fill)
                    .into()],
            ))
            .push(group("Audio quality", "AAC bitrate, 32-320 kbps.", vec![quality]))
    }

    pub(super) fn tab_theme(&self) -> Column<'_, Message> {
        let card = |t: AppTheme| -> Element<'_, Message> {
            let pal = t.resolve(self.system_mode).palette();
            let chip = |c: iced::Color| {
                container(Space::new())
                    .width(16)
                    .height(16)
                    .style(theme::dot(c))
            };
            let swatches = row![
                chip(pal.primary),
                chip(pal.success),
                chip(pal.warning),
                chip(pal.danger),
            ]
            .spacing(5);
            button(
                column![text(t.to_string()).size(13).color(pal.text), swatches].spacing(8),
            )
            .on_press(Message::Theme(t))
            .width(Length::Fill)
            .padding(12)
            .style(theme::theme_card(pal, self.settings.theme == t))
            .into()
        };

        let mut grid = Column::new().spacing(8);
        for pair in AppTheme::ALL.chunks(2) {
            let mut r = row![].spacing(8);
            for t in pair {
                r = r.push(card(*t));
            }
            if pair.len() == 1 {
                r = r.push(Space::new().width(Length::Fill));
            }
            grid = grid.push(r);
        }

        Column::new().spacing(14).push(group(
            "Theme",
            "Saved automatically. \"Follow system\" switches between light and dark with your OS.",
            vec![grid.into()],
        ))
    }
}
