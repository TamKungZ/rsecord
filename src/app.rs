use crate::recorder::{
    list_displays, screenshot, AudioSource, DisplayInfo, RecordConfig, Recorder,
};
use crate::selector::{SelMsg, Selector, Shot};
use crate::settings::{
    Content, Effort, Filter, Preset, RateMode, Region, ResMode, Settings, MIN_DIM,
};
use iced::keyboard::{self, key::Named, Key};
use iced::widget::{
    button, container, image, pick_list, row, scrollable, text, text_input, Column, Row,
};
use iced::{
    event, window, Alignment, Element, Event, Length, Padding, Size, Subscription, Task, Theme,
};
use std::collections::HashMap;
use std::time::{Duration, Instant};

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
    const ALL: [Tab; 4] = [Tab::Capture, Tab::Video, Tab::Encoder, Tab::Audio];

    fn label(self) -> &'static str {
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

pub struct App {
    main: window::Id,
    page: Page,
    selector: Option<Selector>,

    state: State,
    recorder: Option<Recorder>,
    elapsed: Duration,
    status: String,

    source: AudioSource,
    displays: Vec<DisplayInfo>,
    display: Option<DisplayInfo>,
    mode: CaptureMode,
    region: Option<Region>,

    settings: Settings,
    txt: HashMap<Field, String>,
    error: Option<String>,
}

impl App {
    pub fn new() -> (Self, Task<Message>) {
        let icon =
            window::icon::from_file_data(include_bytes!("../assets/icon-256.png"), None).ok();
        let (main, open) = window::open(window::Settings {
            size: Size::new(480.0, 800.0),
            min_size: Some(Size::new(430.0, 520.0)),
            icon,
            ..Default::default()
        });

        let (settings, audio) = Settings::load();
        let source = audio
            .and_then(|k| AudioSource::all().into_iter().find(|a| a.to_string() == k))
            .unwrap_or(AudioSource::Mic);
        let displays = list_displays();
        let display = displays
            .iter()
            .find(|d| d.primary)
            .or_else(|| displays.first())
            .cloned();

        let mut app = App {
            main,
            page: Page::Record,
            selector: None,
            state: State::Idle,
            recorder: None,
            elapsed: Duration::ZERO,
            status: "Ready to record".into(),
            source,
            displays,
            display,
            mode: CaptureMode::Full,
            region: None,
            settings,
            txt: HashMap::new(),
            error: None,
        };
        app.sync_texts();
        (app, open.map(|_| Message::MainOpened))
    }

    pub fn title(&self, _: window::Id) -> String {
        "rsecord".into()
    }

    pub fn theme(&self, _: window::Id) -> Theme {
        Theme::Dark
    }

    fn txt(&self, f: Field) -> &str {
        self.txt.get(&f).map(String::as_str).unwrap_or("")
    }

    fn sync_texts(&mut self) {
        let s = &self.settings;
        let t = &mut self.txt;
        t.insert(Field::Fps, s.fps.to_string());
        t.insert(Field::Percent, s.res_percent.to_string());
        t.insert(Field::Width, s.res_w.to_string());
        t.insert(Field::Height, s.res_h.to_string());
        t.insert(Field::Bpp, s.bpp.to_string());
        t.insert(Field::Bitrate, s.bitrate_kbps.to_string());
        t.insert(Field::QpMin, s.qp_min.to_string());
        t.insert(Field::QpMax, s.qp_max.to_string());
        t.insert(Field::Keyframe, s.keyframe_secs.to_string());
        t.insert(Field::Threads, s.threads.to_string());
        t.insert(Field::AudioKbps, s.audio_kbps.to_string());
        self.sync_region_texts();
    }

    fn sync_region_texts(&mut self) {
        let r = self.region.unwrap_or(Region {
            x: 0,
            y: 0,
            w: 0,
            h: 0,
        });
        self.txt.insert(Field::RegX, r.x.to_string());
        self.txt.insert(Field::RegY, r.y.to_string());
        self.txt.insert(Field::RegW, r.w.to_string());
        self.txt.insert(Field::RegH, r.h.to_string());
    }

    fn pu32(&self, f: Field, name: &str) -> Result<u32, String> {
        self.txt(f)
            .trim()
            .parse::<u32>()
            .map_err(|_| format!("{name}: enter a whole number"))
    }

    fn pf32(&self, f: Field, name: &str) -> Result<f32, String> {
        self.txt(f)
            .trim()
            .parse::<f32>()
            .map_err(|_| format!("{name}: enter a number"))
    }

    fn parse_settings(&self) -> Result<Settings, String> {
        let mut s = self.settings.clone();
        s.fps = self.pu32(Field::Fps, "FPS")?;
        if s.res_mode == ResMode::Percent {
            s.res_percent = self.pf32(Field::Percent, "Scale %")?;
        }
        if s.res_mode == ResMode::Custom {
            s.res_w = self.pu32(Field::Width, "Width")?;
            s.res_h = self.pu32(Field::Height, "Height")?;
        }
        if s.auto_bitrate {
            s.bpp = self.pf32(Field::Bpp, "Bits per pixel")?;
        } else {
            s.bitrate_kbps = self.pu32(Field::Bitrate, "Bitrate")?;
        }
        s.qp_min = self.pu32(Field::QpMin, "QP min")?.min(255) as u8;
        s.qp_max = self.pu32(Field::QpMax, "QP max")?.min(255) as u8;
        s.keyframe_secs = self.pf32(Field::Keyframe, "Keyframe interval")?;
        s.threads = self.pu32(Field::Threads, "Threads")?.min(u16::MAX as u32) as u16;
        s.audio_kbps = self.pu32(Field::AudioKbps, "AAC bitrate")?;
        s.validate()?;
        Ok(s)
    }

    fn touch(&mut self) {
        self.settings.preset = Preset::Custom;
        match self.parse_settings() {
            Ok(s) => {
                self.settings = s;
                self.error = None;
            }
            Err(e) => self.error = Some(e),
        }
    }

    fn commit_region(&mut self) {
        let parsed = (
            self.txt(Field::RegX).trim().parse::<u32>().ok(),
            self.txt(Field::RegY).trim().parse::<u32>().ok(),
            self.txt(Field::RegW).trim().parse::<u32>().ok(),
            self.txt(Field::RegH).trim().parse::<u32>().ok(),
        );
        match parsed {
            (Some(x), Some(y), Some(w), Some(h)) => {
                self.region = Some(Region { x, y, w, h });
                self.mode = CaptureMode::Region;
                self.error = None;
            }
            _ => self.error = Some("Area: X, Y, W and H must be whole numbers".into()),
        }
    }

    fn save(&self) {
        self.settings.save(&self.source.to_string());
    }

    fn restore_main(&self) -> Task<Message> {
        Task::batch([
            window::minimize(self.main, false),
            window::gain_focus(self.main),
        ])
    }

    fn finish_select(&mut self, confirm: bool) -> Task<Message> {
        let (id, region) = match self.selector.as_ref() {
            Some(s) => (s.id, s.region()),
            None => return Task::none(),
        };
        if confirm {
            let Some(r) = region else {
                return Task::none();
            };
            self.region = Some(r);
            self.mode = CaptureMode::Region;
            self.sync_region_texts();
            self.status = format!("Area: {}x{} at ({}, {})", r.w, r.h, r.x, r.y);
        } else {
            self.status = "Area selection cancelled".into();
        }
        self.selector = None;
        Task::batch([window::close(id), self.restore_main()])
    }

    fn summary(&self) -> String {
        let (sw, sh) = match (self.mode, self.region, &self.display) {
            (CaptureMode::Region, Some(r), _) => (r.w, r.h),
            (_, _, Some(d)) => (d.width, d.height),
            _ => (1920, 1080),
        };
        let (sw, sh) = ((sw & !1).max(MIN_DIM), (sh & !1).max(MIN_DIM));
        let s = &self.settings;
        let (ow, oh) = s.output_size(sw, sh);
        let bps = s.bitrate_bps(ow, oh);
        let audio = if self.source == AudioSource::Off {
            0
        } else {
            s.audio_kbps as u64 * 1000
        };
        let mb_min = (bps as u64 + audio) as f64 / 8.0 * 60.0 / 1_048_576.0;
        let tag = match s.rate {
            RateMode::Bitrate => "",
            RateMode::Quality => " (target)",
        };
        format!(
            "Output {ow}x{oh} @ {} fps, {:.1} Mbps{tag}, about {:.0} MB/min",
            s.fps,
            bps as f64 / 1e6,
            mb_min
        )
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::MainOpened | Message::SelectorOpened => {}
            Message::Page(p) => self.page = p,

            Message::Quit => return iced::exit(),

            Message::WindowClosed(id) => {
                if id == self.main {
                    self.save();
                    // Finish the file first so a recording is never left corrupt.
                    if let Some(rec) = self.recorder.take() {
                        return Task::perform(
                            async move {
                                let _ = tokio::task::spawn_blocking(move || rec.finish()).await;
                            },
                            |_| Message::Quit,
                        );
                    }
                    return iced::exit();
                }
                if self.selector.as_ref().map(|s| s.id) == Some(id) {
                    self.selector = None;
                    return self.restore_main();
                }
            }

            Message::Source(s) => self.source = s,
            Message::Display(d) => self.display = Some(d),
            Message::RefreshDisplays => {
                self.displays = list_displays();
                let keep = self
                    .display
                    .as_ref()
                    .and_then(|d| self.displays.iter().find(|x| x.label == d.label))
                    .cloned();
                self.display = keep
                    .or_else(|| self.displays.iter().find(|d| d.primary).cloned())
                    .or_else(|| self.displays.first().cloned());
            }
            Message::Mode(m) => self.mode = m,

            Message::SelectArea => {
                if self.selector.is_some() || !matches!(self.state, State::Idle) {
                    return Task::none();
                }
                let display = self.display.as_ref().map(|d| d.index).unwrap_or(0);
                self.status = "Capturing screen...".into();
                return Task::batch([
                    // Get the rsecord window out of the screenshot.
                    window::minimize(self.main, true),
                    Task::perform(
                        async move {
                            tokio::task::spawn_blocking(move || {
                                std::thread::sleep(Duration::from_millis(450));
                                screenshot(display)
                                    .map(|img| {
                                        let (w, h) = (img.width(), img.height());
                                        let mut raw = img.into_raw();
                                        // Some capture backends leave alpha at 0.
                                        for px in raw.chunks_exact_mut(4) {
                                            px[3] = 255;
                                        }
                                        Shot {
                                            handle: image::Handle::from_rgba(w, h, raw),
                                            w,
                                            h,
                                        }
                                    })
                                    .map_err(|e| e.to_string())
                            })
                            .await
                            .map_err(|e| e.to_string())
                            .and_then(|r| r)
                        },
                        Message::Shot,
                    ),
                ]);
            }
            Message::Shot(Ok(shot)) => {
                let (id, open) = window::open(window::Settings {
                    fullscreen: true,
                    decorations: false,
                    resizable: false,
                    level: window::Level::AlwaysOnTop,
                    ..Default::default()
                });
                self.selector = Some(Selector::new(id, shot, self.region));
                self.status = "Select the area to record".into();
                return open.map(|_| Message::SelectorOpened);
            }
            Message::Shot(Err(e)) => {
                self.status = format!("Error: {e}");
                return self.restore_main();
            }
            Message::Sel(SelMsg::Confirm) => return self.finish_select(true),
            Message::Sel(SelMsg::Cancel) => return self.finish_select(false),
            Message::Sel(m) => {
                if let Some(sel) = self.selector.as_mut() {
                    sel.update(m);
                }
            }
            Message::Key(k) => {
                if self.selector.is_some() {
                    return self.finish_select(matches!(k, KeyAction::Confirm));
                }
            }

            Message::Preset(p) => {
                self.settings = Settings::with_preset(p, &self.settings);
                self.sync_texts();
                self.error = None;
            }
            Message::ResMode(m) => {
                self.settings.res_mode = m;
                self.touch();
            }
            Message::Filter(f) => {
                self.settings.filter = f;
                self.touch();
            }
            Message::Rate(r) => {
                self.settings.rate = r;
                self.touch();
            }
            Message::Effort(e) => {
                self.settings.effort = e;
                self.touch();
            }
            Message::Content(c) => {
                self.settings.content = c;
                self.touch();
            }
            Message::SetFps(n) => {
                self.txt.insert(Field::Fps, n.to_string());
                self.touch();
            }
            Message::Toggle(flag) => {
                let s = &mut self.settings;
                match flag {
                    Flag::KeepAspect => s.keep_aspect = !s.keep_aspect,
                    Flag::AutoBitrate => s.auto_bitrate = !s.auto_bitrate,
                    Flag::SkipFrames => s.skip_frames = !s.skip_frames,
                    Flag::AdaptiveQuant => s.adaptive_quant = !s.adaptive_quant,
                    Flag::BgDetect => s.bg_detect = !s.bg_detect,
                    Flag::SceneDetect => s.scene_detect = !s.scene_detect,
                }
                self.touch();
            }
            Message::Edit(field, raw) => {
                let clean: String = raw
                    .chars()
                    .filter(|c| c.is_ascii_digit() || *c == '.')
                    .collect();
                self.txt.insert(field, clean);
                match field {
                    Field::RegX | Field::RegY | Field::RegW | Field::RegH => self.commit_region(),
                    _ => self.touch(),
                }
            }

            Message::Start => {
                if !matches!(self.state, State::Idle) {
                    return Task::none();
                }
                let settings = match self.parse_settings() {
                    Ok(s) => s,
                    Err(e) => {
                        self.status = format!("Error: {e}");
                        return Task::none();
                    }
                };
                self.settings = settings.clone();
                let region = match self.mode {
                    CaptureMode::Full => None,
                    CaptureMode::Region => match self.region {
                        Some(r) => Some(r),
                        None => {
                            self.status = "Error: select an area first".into();
                            return Task::none();
                        }
                    },
                };
                self.save();
                let display = self.display.as_ref().map(|d| d.index).unwrap_or(0);
                match Recorder::start(RecordConfig {
                    source: self.source,
                    display,
                    region,
                    settings,
                }) {
                    Ok(r) => {
                        self.recorder = Some(r);
                        self.elapsed = Duration::ZERO;
                        self.state = State::Recording {
                            since: Instant::now(),
                        };
                        self.status = "Recording...".into();
                    }
                    Err(e) => self.status = format!("Error: {e}"),
                }
            }
            Message::Tick(now) => {
                if let State::Recording { since } = self.state {
                    self.elapsed = now.duration_since(since);
                }
            }
            Message::Stop => {
                if let Some(rec) = self.recorder.take() {
                    self.state = State::Saving;
                    self.status = "Saving file...".into();
                    return Task::perform(
                        async move {
                            tokio::task::spawn_blocking(move || rec.finish())
                                .await
                                .map_err(|e| e.to_string())
                                .and_then(|r| r.map_err(|e| e.to_string()))
                                .map(|p| p.display().to_string())
                        },
                        Message::Finished,
                    );
                }
            }
            Message::Finished(res) => {
                self.state = State::Idle;
                self.status = match res {
                    Ok(path) => format!("Saved: {path}"),
                    Err(e) => format!("Error: {e}"),
                };
            }
        }
        Task::none()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let mut subs = vec![
            window::close_events().map(Message::WindowClosed),
            event::listen_with(on_event),
        ];
        if let State::Recording { .. } = self.state {
            subs.push(iced::time::every(Duration::from_millis(200)).map(Message::Tick));
        }
        Subscription::batch(subs)
    }

    pub fn view(&self, id: window::Id) -> Element<'_, Message> {
        if let Some(sel) = &self.selector {
            if sel.id == id {
                return sel.view().map(Message::Sel);
            }
        }
        self.main_view()
    }

    fn main_view(&self) -> Element<'_, Message> {
        match self.page {
            Page::Settings(tab) if matches!(self.state, State::Idle) => self.settings_view(tab),
            _ => self.record_view(),
        }
    }

    fn record_view(&self) -> Element<'_, Message> {
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

    fn settings_view(&self, tab: Tab) -> Element<'_, Message> {
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

    fn num_in(&self, f: Field, hint: &'static str, w: f32) -> Element<'_, Message> {
        text_input(hint, self.txt(f))
            .on_input(move |s| Message::Edit(f, s))
            .padding(5)
            .size(13)
            .width(Length::Fixed(w))
            .into()
    }

    fn tab_capture(&self) -> Column<'_, Message> {
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

    fn tab_video(&self) -> Column<'_, Message> {
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

    fn tab_encoder(&self) -> Column<'_, Message> {
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

    fn tab_audio(&self) -> Column<'_, Message> {
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

fn on_event(event: Event, _status: event::Status, _window: window::Id) -> Option<Message> {
    match event {
        Event::Keyboard(keyboard::Event::KeyPressed { key, .. }) => match key {
            Key::Named(Named::Escape) => Some(Message::Key(KeyAction::Cancel)),
            Key::Named(Named::Enter) => Some(Message::Key(KeyAction::Confirm)),
            _ => None,
        },
        _ => None,
    }
}

fn group<'a>(
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

fn field_row<'a>(label: &'static str, input: Element<'a, Message>) -> Element<'a, Message> {
    row![lbl(label), input]
        .spacing(8)
        .align_y(Alignment::Center)
        .into()
}

fn info_row(label: &'static str, value: String) -> Element<'static, Message> {
    row![
        text(label).size(13).width(Length::Fixed(70.0)),
        text(value).size(13)
    ]
    .spacing(8)
    .into()
}

fn lbl(label: &'static str) -> Element<'static, Message> {
    text(label).size(13).width(Length::Fixed(130.0)).into()
}

fn toggle(label: &'static str, on: bool, flag: Flag) -> Element<'static, Message> {
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
