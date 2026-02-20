use iced::widget::{column, container, mouse_area, text, Column};
use iced::{Element, Fill, Padding};

use crate::app::Message;
use crate::source::SourceItem;
use crate::ui::theme;

/// Maximum number of items visible at once
const VISIBLE_COUNT: usize = 8;

/// Build the result list widget.
/// Shows a window of VISIBLE_COUNT items around the selected index.
pub fn view<'a>(results: &'a [SourceItem], selected_index: usize) -> Element<'a, Message> {
    if results.is_empty() {
        return column![].into();
    }

    // Calculate visible window: keep selected item in view
    let start = if selected_index + 1 >= VISIBLE_COUNT {
        selected_index + 1 - VISIBLE_COUNT
    } else {
        0
    };
    let end = (start + VISIBLE_COUNT).min(results.len());

    let mut rows = Column::new().spacing(2);
    for i in start..end {
        let item = &results[i];
        let is_selected = i == selected_index;
        let style = if is_selected {
            theme::result_row_selected as fn(&iced::Theme) -> container::Style
        } else {
            theme::result_row
        };

        let name = text(&item.title).size(16).color(theme::TEXT_PRIMARY);

        let row_content: Element<'a, Message> = if let Some(subtitle) = &item.subtitle {
            column![name, text(subtitle).size(12).color(theme::TEXT_SECONDARY)]
                .spacing(2)
                .into()
        } else {
            name.into()
        };

        let row = container(row_content)
            .padding(Padding::from([8, 12]))
            .width(Fill)
            .style(style);

        let clickable = mouse_area(row)
            .on_press(Message::SelectAndExecute(i));

        rows = rows.push(clickable);
    }

    rows.into()
}
