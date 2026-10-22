mod record;
mod settings_page;
mod tabs;
mod widgets;

use crate::app::App;
use crate::app::message::{Message, Page, State};
use iced::{window, Element};

impl App {
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
}
