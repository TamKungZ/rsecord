mod form;
mod helpers;
mod message;
mod update;
mod view;

use crate::recorder::{list_displays, AudioSource, DisplayInfo, Recorder};
use crate::selector::Selector;
use crate::settings::{Region, Settings};
use self::message::{CaptureMode, Field, KeyAction, Message, Page, State, Status};
use iced::keyboard::{self, key::Named, Key};
use iced::theme::Mode;
use iced::{event, window, Event, Size, Subscription, Task, Theme};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

pub struct App {
    main: window::Id,
    page: Page,
    selector: Option<Selector>,

    state: State,
    recorder: Option<Recorder>,
    elapsed: Duration,
    status: Status,
    /// The last file written, for the "Open folder" button.
    saved: Option<PathBuf>,

    source: AudioSource,
    displays: Vec<DisplayInfo>,
    display: Option<DisplayInfo>,
    mode: CaptureMode,
    region: Option<Region>,

    settings: Settings,
    txt: HashMap<Field, String>,

    /// Resolved from `settings.theme`; rebuilt only when it or the OS mode changes.
    theme: Theme,
    system_mode: Mode,
    confirm_reset: bool,
    /// The window was closed while the file was still being written.
    closing: bool,
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

        let theme = settings.theme.resolve(Mode::None);
        let mut app = App {
            main,
            page: Page::Record,
            selector: None,
            state: State::Idle,
            recorder: None,
            elapsed: Duration::ZERO,
            status: Status::info("Ready to record"),
            saved: None,
            source,
            displays,
            display,
            mode: CaptureMode::Full,
            region: None,
            settings,
            txt: HashMap::new(),
            theme,
            system_mode: Mode::None,
            confirm_reset: false,
            closing: false,
        };
        app.sync_texts();
        (
            app,
            Task::batch([
                open.map(|_| Message::MainOpened),
                iced::system::theme().map(Message::SystemTheme),
            ]),
        )
    }

    pub fn title(&self, _: window::Id) -> String {
        "rsecord".into()
    }

    pub fn theme(&self, _: window::Id) -> Theme {
        self.theme.clone()
    }

    pub(super) fn apply_theme(&mut self) {
        self.theme = self.settings.theme.resolve(self.system_mode);
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let mut subs = vec![
            window::close_events().map(Message::WindowClosed),
            event::listen_with(on_event),
            iced::system::theme_changes().map(Message::SystemTheme),
        ];
        if let State::Recording { .. } = self.state {
            subs.push(iced::time::every(Duration::from_millis(200)).map(Message::Tick));
        }
        Subscription::batch(subs)
    }
}

fn on_event(event: Event, status: event::Status, _window: window::Id) -> Option<Message> {
    let Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) = event else {
        return None;
    };
    // A focused text field handles Escape (to leave the field) itself.
    let free = status == event::Status::Ignored;
    match key {
        Key::Named(Named::Escape) if free => Some(Message::Key(KeyAction::Cancel)),
        Key::Named(Named::Enter) if free => Some(Message::Key(KeyAction::Confirm)),
        Key::Named(Named::Tab) if free => Some(Message::Focus {
            forward: !modifiers.shift(),
        }),
        _ => None,
    }
}
