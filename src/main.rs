#![cfg_attr(all(not(debug_assertions), windows), windows_subsystem = "windows")]

mod app;
mod recorder;
mod selector;
mod settings;

use app::App;

// `daemon` allows a second window for the area-selection overlay.
fn main() -> iced::Result {
    iced::daemon(App::new, App::update, App::view)
        .title(App::title)
        .subscription(App::subscription)
        .theme(App::theme)
        .run()
}
