#![cfg_attr(all(not(debug_assertions), windows), windows_subsystem = "windows")]

mod recorder;

use iced::widget::{button, column, container, pick_list, row, text};
use iced::{Alignment, Element, Length, Subscription, Task, Theme};
use recorder::{AudioSource, Recorder};
use std::time::{Duration, Instant};

const FPS: u32 = 30;

fn theme(_: &App) -> Theme {
    Theme::Dark
}

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title("rsecord")
        .subscription(App::subscription)
        .theme(theme)
        .window_size((340.0, 260.0))
        .run()
}

#[derive(Debug, Clone)]
enum Message {
    Start,
    Stop,
    Tick(Instant),
    Source(AudioSource),
    Finished(Result<String, String>),
}

enum State {
    Idle,
    Recording { since: Instant },
    Saving,
}

struct App {
    state: State,
    recorder: Option<Recorder>,
    source: AudioSource,
    elapsed: Duration,
    status: String,
}

impl App {
    fn new() -> (Self, Task<Message>) {
        (
            App {
                state: State::Idle,
                recorder: None,
                source: AudioSource::Mic,
                elapsed: Duration::ZERO,
                status: "Ready to record".into(),
            },
            Task::none(),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Source(s) => self.source = s,
            Message::Start => match Recorder::start(self.source, FPS) {
                Ok(r) => {
                    self.recorder = Some(r);
                    self.elapsed = Duration::ZERO;
                    self.state = State::Recording { since: Instant::now() };
                    self.status = "Recording...".into();
                }
                Err(e) => self.status = format!("Error: {e}"),
            },
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

    fn subscription(&self) -> Subscription<Message> {
        match self.state {
            State::Recording { .. } => iced::time::every(Duration::from_millis(200)).map(Message::Tick),
            _ => Subscription::none(),
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let secs = self.elapsed.as_secs();
        let clock = text(format!("{:02}:{:02}:{:02}", secs / 3600, (secs / 60) % 60, secs % 60)).size(44);

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

        let source = pick_list(AudioSource::all(), Some(self.source), Message::Source);

        container(
            column![
                clock,
                main_btn,
                row![text("Audio:"), if idle { source } else { source.placeholder("") }]
                    .spacing(10)
                    .align_y(Alignment::Center),
                text(&self.status).size(13),
            ]
            .spacing(14)
            .align_x(Alignment::Center),
        )
        .padding(20)
        .center_x(Length::Fill)
        .into()
    }
}