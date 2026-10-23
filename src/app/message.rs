use crate::recorder::{AudioSource, DisplayInfo};
use crate::selector::{SelMsg, Shot};
use crate::settings::{AppTheme, Content, Effort, Filter, Preset, RateMode, ResMode};
use crate::theme::Tone;
use iced::theme::Mode;
use iced::window;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Field {
    Fps,
    Percent,
    Width,
    Height,
    Bpp,
    Bitrate,
    QpMin,
    QpMax,
    Keyframe,
    Threads,
    AudioKbps,
    RegX,
    RegY,
    RegW,
    RegH,
}

impl Field {
    /// Fields that accept a decimal point.
    pub fn is_float(self) -> bool {
        matches!(self, Field::Percent | Field::Bpp | Field::Keyframe)
    }

    /// Longest input accepted, so a stray paste cannot overflow a number.
    pub fn max_len(self) -> usize {
        if self.is_float() { 7 } else { 6 }
    }

    /// The settings tab where this field is edited.
    pub fn tab(self) -> Tab {
        match self {
            Field::Fps | Field::Percent | Field::Width | Field::Height => Tab::Video,
            Field::Bpp
            | Field::Bitrate
            | Field::QpMin
            | Field::QpMax
            | Field::Keyframe
            | Field::Threads => Tab::Encoder,
            Field::AudioKbps => Tab::Audio,
            Field::RegX | Field::RegY | Field::RegW | Field::RegH => Tab::Capture,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flag {
    KeepAspect,
    AutoBitrate,
    SkipFrames,
    AdaptiveQuant,
    BgDetect,
    SceneDetect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureMode {
    Full,
    Region,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Capture,
    Video,
    Encoder,
    Audio,
    Theme,
}

impl Tab {
    pub(super) const ALL: [Tab; 5] = [Tab::Capture, Tab::Video, Tab::Encoder, Tab::Audio, Tab::Theme];

    pub(super) fn label(self) -> &'static str {
        match self {
            Tab::Capture => "Capture",
            Tab::Video => "Video",
            Tab::Encoder => "Encoder",
            Tab::Audio => "Audio",
            Tab::Theme => "Theme",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Record,
    Settings(Tab),
}

#[derive(Debug, Clone, Copy)]
pub enum KeyAction {
    Confirm,
    Cancel,
}

#[derive(Debug, Clone)]
pub enum Message {
    MainOpened,
    SelectorOpened,
    Page(Page),
    WindowClosed(window::Id),
    Quit,

    Theme(AppTheme),
    SystemTheme(Mode),
    Focus { forward: bool },
    OpenFolder,
    ResetAsk,
    ResetConfirm,
    ResetCancel,

    Start,
    Stop,
    Tick(Instant),
    Finished(Result<String, String>),

    Source(AudioSource),
    Display(DisplayInfo),
    RefreshDisplays,
    Mode(CaptureMode),

    SelectArea,
    Shot(Result<Shot, String>),
    Sel(SelMsg),
    Key(KeyAction),

    Preset(Preset),
    ResMode(ResMode),
    Filter(Filter),
    Rate(RateMode),
    Effort(Effort),
    Content(Content),
    Edit(Field, String),
    Toggle(Flag),
    SetFps(u32),
}

pub enum State {
    Idle,
    Recording { since: Instant },
    Saving,
}

/// The one-line message under the record button.
#[derive(Debug, Clone)]
pub struct Status {
    pub tone: Tone,
    pub text: String,
}

impl Status {
    pub fn info(text: impl Into<String>) -> Self {
        Status { tone: Tone::Neutral, text: text.into() }
    }
    pub fn ok(text: impl Into<String>) -> Self {
        Status { tone: Tone::Success, text: text.into() }
    }
    pub fn error(text: impl Into<String>) -> Self {
        Status { tone: Tone::Danger, text: text.into() }
    }
}
