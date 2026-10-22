use crate::app::App;
use crate::app::message::{CaptureMode, Field, Flag, Message};
use crate::recorder::AudioSource;
use crate::settings::{Content, Effort, Filter, Preset, RateMode, ResMode};
use super::widgets::{field_row, group, lbl, toggle};
use iced::{Alignment, Element, Length, Theme};
use iced::widget::{button, pick_list, row, text, Column};

impl App {
    pub(super) fn tab_capture(&self) -> Column<'_, Message> {
        let display_row: Element<'_, Message> = row![
            pick_list(
                self.displays.clone(),
                self.display.clone(),
                Message::Display
            )
            .width(Length::Fill),
            button(text("Refresh").size(12))
                .on_press(Message::RefreshDisplays)
                .padding(5)
                .style(button::secondary),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into();

        let mode_btn = |label: &'static str, m: CaptureMode| {
            let active = self.mode == m;
            button(text(label).size(13).center())
                .on_press(Message::Mode(m))
                .width(Length::Fill)
                .padding(7)
                .style(move |t: &Theme, st| {
                    if active {
                        button::primary(t, st)
                    } else {
                        button::secondary(t, st)
                    }
                })
        };
        let mut area_rows: Vec<Element<'_, Message>> = vec![row![
            mode_btn("Full screen", CaptureMode::Full),
            mode_btn("Selected area", CaptureMode::Region)
        ]
        .spacing(8)
        .into()];

        if self.mode == CaptureMode::Region {
            area_rows.push(
                row![
                    text("X").size(13),
                    self.num_in(Field::RegX, "0", 64.0),
                    text("Y").size(13),
                    self.num_in(Field::RegY, "0", 64.0),
                    text("W").size(13),
                    self.num_in(Field::RegW, "0", 64.0),
                    text("H").size(13),
                    self.num_in(Field::RegH, "0", 64.0),
                ]
                .spacing(6)
                .align_y(Alignment::Center)
                .into(),
            );
            area_rows.push(
                button(text("Select area on screen...").center())
                    .on_press(Message::SelectArea)
                    .width(Length::Fill)
                    .padding(8)
                    .style(button::primary)
                    .into(),
            );
            area_rows.push(
                text(match self.region {
                    Some(r) => format!(
                        "Recording {}x{} px starting at ({}, {}) on the chosen display",
                        r.w, r.h, r.x, r.y
                    ),
                    None => "No area yet. Use the button above, or type X/Y/W/H in pixels."
                        .to_string(),
                })
                .size(12)
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

        let fps_btn = |n: u32| {
            button(text(n.to_string()).size(12))
                .on_press(Message::SetFps(n))
                .padding(5)
                .style(button::secondary)
        };
        let fps_row: Element<'_, Message> = row![
            self.num_in(Field::Fps, "30", 70.0),
            fps_btn(15),
            fps_btn(24),
            fps_btn(30),
            fps_btn(60),
            fps_btn(120),
        ]
        .spacing(6)
        .align_y(Alignment::Center)
        .into();

        let mut res_rows: Vec<Element<'_, Message>> = vec![pick_list(
            ResMode::ALL,
            Some(s.res_mode),
            Message::ResMode,
        )
        .width(Length::Fill)
        .into()];
        match s.res_mode {
            ResMode::Percent => res_rows.push(field_row(
                "Scale (%)",
                self.num_in(Field::Percent, "100", 90.0),
            )),
            ResMode::Custom => {
                res_rows.push(
                    row![
                        lbl("Size (px)"),
                        self.num_in(Field::Width, "1920", 80.0),
                        text("x").size(13),
                        self.num_in(Field::Height, "1080", 80.0),
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center)
                    .into(),
                );
                res_rows.push(toggle("Keep aspect ratio", s.keep_aspect, Flag::KeepAspect));
            }
            _ => {}
        }
        res_rows.push(field_row(
            "Scaling filter",
            pick_list(Filter::ALL, Some(s.filter), Message::Filter)
                .width(Length::Fill)
                .into(),
        ));

        Column::new()
            .spacing(14)
            .push(group(
                "Preset",
                "One-click starting points. Changing any value afterwards switches to Custom.",
                vec![pick_list(Preset::ALL, Some(s.preset), Message::Preset)
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
                    .width(Length::Fill)
                    .into(),
            ),
            toggle(
                "Auto bitrate (bits per pixel)",
                s.auto_bitrate,
                Flag::AutoBitrate,
            ),
        ];
        if s.auto_bitrate {
            rate_rows.push(field_row(
                "Bits per pixel",
                self.num_in(Field::Bpp, "0.10", 90.0),
            ));
        } else {
            rate_rows.push(field_row(
                "Bitrate (kbps)",
                self.num_in(Field::Bitrate, "6000", 90.0),
            ));
        }
        rate_rows.push(
            row![
                lbl("QP range (0-51)"),
                self.num_in(Field::QpMin, "0", 60.0),
                text("to").size(13),
                self.num_in(Field::QpMax, "51", 60.0),
            ]
            .spacing(8)
            .align_y(Alignment::Center)
            .into(),
        );

        let tune_rows: Vec<Element<'_, Message>> = vec![
            field_row(
                "Content type",
                pick_list(Content::ALL, Some(s.content), Message::Content)
                    .width(Length::Fill)
                    .into(),
            ),
            field_row(
                "Effort",
                pick_list(Effort::ALL, Some(s.effort), Message::Effort)
                    .width(Length::Fill)
                    .into(),
            ),
            field_row(
                "Keyframe every (s)",
                self.num_in(Field::Keyframe, "2", 90.0),
            ),
            field_row(
                "Threads (0 = auto)",
                self.num_in(Field::Threads, "0", 90.0),
            ),
        ];

        let adv_rows: Vec<Element<'_, Message>> = vec![
            row![
                toggle("Skip frames", s.skip_frames, Flag::SkipFrames),
                toggle("Adaptive quant.", s.adaptive_quant, Flag::AdaptiveQuant),
            ]
            .spacing(8)
            .into(),
            row![
                toggle("Background detect", s.bg_detect, Flag::BgDetect),
                toggle("Scene-change detect", s.scene_detect, Flag::SceneDetect),
            ]
            .spacing(8)
            .into(),
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
        Column::new()
            .spacing(14)
            .push(group(
                "Audio source",
                "None, microphone, or system audio.",
                vec![pick_list(AudioSource::all(), Some(self.source), Message::Source)
                    .width(Length::Fill)
                    .into()],
            ))
            .push(group(
                "Audio quality",
                "AAC bitrate, 32-320 kbps.",
                vec![field_row(
                    "AAC bitrate (kbps)",
                    self.num_in(Field::AudioKbps, "128", 90.0),
                )],
            ))
    }
}
