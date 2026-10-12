#![cfg_attr(all(not(debug_assertions), windows), windows_subsystem = "windows")]

mod app;
mod recorder;

use app::App;

fn theme(_: &App) -> iced::Theme {
    iced::Theme::Dark
}

fn main() -> iced::Result {
    let icon = iced::window::icon::from_file_data(
        include_bytes!("../assets/icon-256.png"),
        None,
    )
    .ok();

    iced::application(App::new, App::update, App::view)
        .title("rsecord")
        .subscription(App::subscription)
        .theme(theme)
        .window(iced::window::Settings {
            size: iced::Size::new(340.0, 260.0),
            icon,
            ..Default::default()
        })
        .run()
}