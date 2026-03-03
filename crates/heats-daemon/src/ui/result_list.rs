use iced::widget::{column, container, image, mouse_area, row, text, Column};
use iced::{Element, Fill, Padding};

use crate::app::Message;
use crate::ui::theme;
use heats_core::source::{IconData, SourceItem};

/// Estimated row height in pixels (padding + title + subtitle + spacing)
const ROW_HEIGHT_ESTIMATE: f32 = 54.0;
/// Fixed overhead: outer padding (12*2) + search input (~44) + spacing (8)
const LAYOUT_OVERHEAD: f32 = 76.0;

/// Calculate how many items fit in the available window height.
fn visible_count(window_height: f32) -> usize {
    let available = (window_height - LAYOUT_OVERHEAD).max(0.0);
    let count = (available / ROW_HEIGHT_ESTIMATE) as usize;
    count.max(1)
}

/// Build the result list widget.
/// Shows a window of items around the selected index, sized to fit the window.
pub fn view<'a>(
    results: &[&'a SourceItem],
    selected_index: usize,
    window_height: f32,
) -> Element<'a, Message> {
    if results.is_empty() {
        return column![].into();
    }

    let max_visible = visible_count(window_height);

    // Calculate visible window: keep selected item in view
    let start = (selected_index + 1).saturating_sub(max_visible);
    let end = (start + max_visible).min(results.len());

    let mut rows = Column::new().spacing(2);
    for (i, item) in results.iter().enumerate().take(end).skip(start) {
        let is_selected = i == selected_index;
        let style = if is_selected {
            theme::result_row_selected as fn(&iced::Theme) -> container::Style
        } else {
            theme::result_row
        };

        let name = text(&item.title).size(16).color(theme::TEXT_PRIMARY);

        let text_column: Element<'a, Message> = if let Some(subtitle) = &item.subtitle {
            column![name, text(subtitle).size(12).color(theme::TEXT_SECONDARY)]
                .spacing(2)
                .into()
        } else {
            name.into()
        };

        let row_content: Element<'a, Message> = match &item.icon {
            Some(IconData::Rgba {
                width,
                height,
                pixels,
            }) => {
                let handle =
                    image::Handle::from_rgba(*width, *height, pixels.as_ref().clone());
                let icon = image(handle).width(24).height(24);
                row![icon, text_column]
                    .spacing(8)
                    .align_y(iced::Alignment::Center)
                    .into()
            }
            Some(IconData::Text(s)) => row![text(s).size(20), text_column]
                .spacing(8)
                .align_y(iced::Alignment::Center)
                .into(),
            None => text_column,
        };

        let row = container(row_content)
            .padding(Padding::from([6, 12]))
            .width(Fill)
            .style(style);

        let clickable = mouse_area(row).on_press(Message::SelectAndExecute(i));

        rows = rows.push(clickable);
    }

    rows.into()
}
