use crate::app::App;
use crate::app::message::{CaptureMode, Field, Flag, KeyAction, Message, State};
use crate::recorder::{list_displays, screenshot, RecordConfig, Recorder};
use crate::selector::{Selector, SelMsg, Shot};
use crate::settings::Settings;
use iced::{window, Task};
use iced::widget::image;
use std::time::{Duration, Instant};

impl App {
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
}
