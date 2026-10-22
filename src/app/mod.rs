mod form;
mod helpers;
mod message;
mod update;
mod view;

use crate::recorder::{list_displays, AudioSource, DisplayInfo, Recorder};
use crate::selector::Selector;
use crate::settings::{Region, Settings};
use self::message::{CaptureMode, Field, KeyAction, Message, Page, State};
use iced::keyboard::{self, key::Named, Key};
use iced::{event, window, Event, Size, Subscription, Task, Theme};
use std::collections::HashMap;
use std::time::Duration;

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
            window::icon::from_file_data(include_bytes!("../../assets/icon-256.png"), None).ok();
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
