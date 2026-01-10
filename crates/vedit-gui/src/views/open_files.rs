use crate::message::Message;
use crate::state::EditorState;
use crate::style::{panel_container, TEXT, MUTED};
use iced::widget::{button, column, container, row, scrollable, text, Column};
use iced::{Alignment, Element, Length};

pub fn render_open_files_panel(
    state: &EditorState,
    _scale: f32,
    spacing_large: f32,
    spacing_medium: f32,
    sidebar_width: f32,
) -> Element<'_, Message> {
    let mut open_list = Column::new().spacing(4);
    for (index, document) in state.editor().open_documents().iter().enumerate() {
        let _is_active = index == state.editor().active_index();
        let mut title = document.display_name().to_string();
        if document.is_modified {
            title.push('*');
        }

        let file_text = text(title.clone()).color(TEXT);

        let close_button = button(text("×").color(MUTED))
            .style(crate::style::custom_button())
            .on_press(Message::DocumentSelected(0)); // dummy

        let item = row![file_text, close_button]
            .spacing(4)
            .align_y(Alignment::Center);

        let button = button(item)
            .style(crate::style::document_button())
            .on_press(Message::DocumentSelected(index));

        open_list = open_list.push(button);
    }

    let open_scroll = scrollable(open_list).style(crate::style::custom_scrollable());

    let header = button(text("Recent Files").color(TEXT))
        .style(crate::style::custom_button())
        .on_press(Message::DocumentSelected(0)); // dummy

    let mut recent_list = Column::new().spacing(4);
    for path in state.workspace_recent_files() {
        let item = button(text(path.clone()).color(MUTED))
            .style(crate::style::document_button())
            .on_press(Message::WorkspaceFileActivated(path.clone()));

        recent_list = recent_list.push(item);
    }

    let recent_scroll = scrollable(recent_list).style(crate::style::custom_scrollable());

    let content = column![
        text("Open Files").color(TEXT),
        open_scroll,
        header,
        recent_scroll,
    ]
    .spacing(spacing_medium)
    .height(Length::Fill);

    container(content)
        .padding(spacing_large)
        .width(Length::Fixed(sidebar_width))
        .height(Length::Fill)
        .style(panel_container())
        .into()
}