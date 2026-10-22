use crate::recorder::{AudioSource, DisplayInfo};
use crate::selector::{SelMsg, Shot};
use crate::settings::{Content, Effort, Filter, Preset, RateMode, ResMode};
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
}

impl Tab {
    pub(super) const ALL: [Tab; 4] = [Tab::Capture, Tab::Video, Tab::Encoder, Tab::Audio];

    pub(super) fn label(self) -> &'static str {
        match self {
            Tab::Capture => "Capture",
            Tab::Video => "Video",
            Tab::Encoder => "Encoder",
            Tab::Audio => "Audio",
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
