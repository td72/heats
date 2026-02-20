use iced::widget::{column, container, mouse_area, scrollable, text, Column};
use iced::{Element, Fill, Padding};

use crate::app::Message;
use crate::source::SourceItem;
use crate::ui::theme;

/// Maximum number of visible results
const MAX_VISIBLE: usize = 8;

/// Build the result list widget
pub fn view<'a>(results: &'a [SourceItem], selected_index: usize) -> Element<'a, Message> {
    if results.is_empty() {
        return column![].into();
    }

    let visible = results.iter().take(MAX_VISIBLE).enumerate();
    let rows: Vec<Element<'a, Message>> = visible
        .map(|(i, item)| {
            let is_selected = i == selected_index;
            let style = if is_selected {
                theme::result_row_selected as fn(&iced::Theme) -> container::Style
            } else {
                theme::result_row
            };

            let name = text(&item.title).size(16).style(theme::result_name);

            let row_content: Element<'a, Message> = if let Some(subtitle) = &item.subtitle {
                column![name, text(subtitle).size(12).style(theme::result_subtitle)]
                    .spacing(2)
                    .into()
            } else {
                name.into()
            };

            let row = container(row_content)
                .padding(Padding::from([8, 12]))
                .width(Fill)
                .style(style);

            mouse_area(row)
                .on_press(Message::SelectAndExecute(i))
                .into()
        })
        .collect();

    let list = Column::from_vec(rows).spacing(2);

    scrollable(list).height(Fill).into()
}
