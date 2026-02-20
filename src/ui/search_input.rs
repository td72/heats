use iced::widget::text_input;
use iced::{Element, Fill};

use crate::app::Message;
use crate::ui::theme;

/// The search input ID for focus management
pub const SEARCH_INPUT_ID: &str = "heats-search-input";

/// Build the search input widget
pub fn view(query: &str) -> Element<'_, Message> {
    text_input("Type to search...", query)
        .on_input(Message::QueryChanged)
        .on_submit(Message::Execute)
        .id(SEARCH_INPUT_ID)
        .padding(12)
        .size(18)
        .width(Fill)
        .style(theme::search_input)
        .into()
}
