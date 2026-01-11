use crate::message::Message;
use crate::state::EditorState;
use crate::style::{self, MUTED, TEXT, TEXT_SECONDARY, panel_container};
use iced::widget::{Column, Space, button, column, container, row, scrollable, text};
use iced::{Alignment, Element, Length, Padding};

pub fn render_open_files_panel(
    state: &EditorState,
    _scale: f32,
    _spacing_large: f32,
    _spacing_medium: f32,
    sidebar_width: f32,
) -> Element<'_, Message> {
    // Compact open files list - no spacing between items
    let mut open_list = Column::new().spacing(0);
    for (index, document) in state.editor().open_documents().iter().enumerate() {
        let is_active = index == state.editor().active_index();
        let mut title = document.display_name().to_string();
        if document.is_modified {
            title.push('*');
        }

        let file_text = text(title)
            .size(12)
            .color(if is_active { TEXT } else { TEXT_SECONDARY });

        // Compact close button that appears on hover (always visible for now)
        let close_button = button(text("×").size(12).color(MUTED))
            .style(style::close_button())
            .padding(Padding::from([0, 4]))
            .on_press(Message::CloseDocument(index));

        let item = row![file_text, Space::new().width(Length::Fill), close_button]
            .spacing(2)
            .align_y(Alignment::Center);

        let file_button = button(item)
            .style(style::open_file_button(is_active))
            .padding(Padding::from([2, 6]))
            .width(Length::Fill)
            .on_press(Message::DocumentSelected(index));

        open_list = open_list.push(file_button);
    }

    let open_scroll = scrollable(open_list)
        .style(crate::style::custom_scrollable())
        .height(Length::FillPortion(1));

    // Compact recent files list
    let mut recent_list = Column::new().spacing(0);
    for path in state.workspace_recent_files() {
        // Show only filename for cleaner look
        let display_name: String = std::path::Path::new(&path)
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| path.clone());

        let item = button(text(display_name).size(12).color(TEXT_SECONDARY))
            .style(style::open_file_button(false))
            .padding(Padding::from([2, 6]))
            .width(Length::Fill)
            .on_press(Message::WorkspaceFileActivated(path.clone()));

        recent_list = recent_list.push(item);
    }

    let recent_scroll = scrollable(recent_list)
        .style(crate::style::custom_scrollable())
        .height(Length::FillPortion(1));

    let content = column![
        text("OPEN EDITORS").size(11).color(MUTED),
        open_scroll,
        Space::new().height(Length::Fixed(8.0)),
        text("RECENT").size(11).color(MUTED),
        recent_scroll,
    ]
    .spacing(4)
    .height(Length::Fill);

    container(content)
        .padding(Padding::from([8, 6]))
        .width(Length::Fixed(sidebar_width))
        .height(Length::Fill)
        .style(panel_container())
        .into()
}
