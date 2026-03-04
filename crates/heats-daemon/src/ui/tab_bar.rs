use iced::widget::{container, row, text};
use iced::{Element, Padding};

use crate::app::Message;
use crate::ui::theme;
use heats_core::config::ModeConfig;

/// Build the tab bar showing mode names. Current mode is highlighted.
pub fn view<'a>(modes: &'a [ModeConfig], current_index: Option<usize>) -> Element<'a, Message> {
    let mut tabs = row![].spacing(4);

    for (i, mode) in modes.iter().enumerate() {
        let is_current = current_index == Some(i);
        let label = text(&mode.name).size(12);
        let label = if is_current {
            label.color(theme::TEXT_PRIMARY)
        } else {
            label.color(theme::TEXT_SECONDARY)
        };
        let style = if is_current {
            theme::tab_active as fn(&iced::Theme) -> container::Style
        } else {
            theme::tab_inactive
        };
        let tab = container(label).padding(Padding::from([3, 8])).style(style);
        tabs = tabs.push(tab);
    }

    tabs.into()
}
