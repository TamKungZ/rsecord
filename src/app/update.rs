use crate::app::App;
use crate::app::form::sanitize;
use crate::app::helpers::open_folder;
use crate::app::message::{CaptureMode, Field, Flag, KeyAction, Message, Page, State, Status};
use crate::recorder::{list_displays, screenshot, RecordConfig, Recorder};
use crate::selector::{Selector, SelMsg, Shot};
use crate::settings::{Preset, Settings};
use iced::{window, Task};
use iced::widget::image;
use std::path::PathBuf;
use std::time::{Duration, Instant};

impl App {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::MainOpened | Message::SelectorOpened => {}
            Message::Page(p) => {
                self.page = p;
                self.confirm_reset = false;
            }

            Message::Theme(t) => {
                self.settings.theme = t;
                self.apply_theme();
                self.save();
            }
            Message::SystemTheme(mode) => {
                self.system_mode = mode;
                self.apply_theme();
            }
            Message::Focus { forward } => {
                return if forward {
                    iced::widget::operation::focus_next()
                } else {
                    iced::widget::operation::focus_previous()
                };
            }
            Message::OpenFolder => {
                if let Some(path) = &self.saved {
                    if let Err(e) = open_folder(path) {
                        self.status = Status::error(e);
                    }
                }
            }
            Message::ResetAsk => self.confirm_reset = true,
            Message::ResetCancel => self.confirm_reset = false,
            Message::ResetConfirm => {
                self.settings = Settings::with_preset(Preset::Balanced, &self.settings);
                self.sync_texts();
                self.confirm_reset = false;
                self.status = Status::info("Video settings reset to the defaults");
            }

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
                    // Already writing the file: exit once it is done.
                    if matches!(self.state, State::Saving) {
                        self.closing = true;
                        return Task::none();
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
                self.status = Status::info("Capturing screen...");
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
                self.status = Status::info("Select the area to record");
                return open.map(|_| Message::SelectorOpened);
            }
            Message::Shot(Err(e)) => {
                self.status = Status::error(format!("Could not capture the screen: {e}"));
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
                // Esc leaves the settings, like the Back button.
                if matches!(k, KeyAction::Cancel) && matches!(self.page, Page::Settings(_)) {
                    self.page = Page::Record;
                    self.confirm_reset = false;
                }
            }

            Message::Preset(p) => {
                self.settings = Settings::with_preset(p, &self.settings);
                self.sync_texts();
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
                self.txt.insert(field, sanitize(field, &raw));
                match field {
                    Field::RegX | Field::RegY | Field::RegW | Field::RegH => self.commit_region(),
                    _ => self.touch(),
                }
            }

            Message::Start => {
                if !matches!(self.state, State::Idle) {
                    return Task::none();
                }
                if let Some(p) = self.problem() {
                    self.status = Status::error(format!("Can't start: {}", p.text));
                    return Task::none();
                }
                let settings = match self.parse_settings() {
                    Ok(s) => s,
                    Err(e) => {
                        self.status = Status::error(format!("Can't start: {e}"));
                        return Task::none();
                    }
                };
                let region = match self.mode {
                    CaptureMode::Full => None,
                    CaptureMode::Region => match self.parse_region() {
                        Ok(r) => Some(r),
                        Err(e) => {
                            self.status = Status::error(format!("Can't start: {e}"));
                            return Task::none();
                        }
                    },
                };
                self.settings = settings.clone();
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
                        self.saved = None;
                        self.state = State::Recording {
                            since: Instant::now(),
                        };
                        self.status = Status::info("Recording...");
                    }
                    Err(e) => self.status = Status::error(format!("Could not start: {e}")),
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
                    self.status = Status::info("Saving file...");
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
                match res {
                    Ok(path) => {
                        self.status = Status::ok(format!("Saved: {path}"));
                        self.saved = Some(PathBuf::from(path));
                    }
                    Err(e) => self.status = Status::error(format!("Recording failed: {e}")),
                }
                if self.closing {
                    return iced::exit();
                }
            }
        }
        Task::none()
    }
}
