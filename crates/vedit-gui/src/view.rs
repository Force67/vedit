use crate::message::{Message, WorkspaceSnapshot};
use crate::state::EditorState;
use iced::alignment::{Horizontal, Vertical};
use iced::widget::lazy;
use iced::widget::{button, column, container, row, scrollable, text, text_editor, Column};
use iced::{theme, Alignment, Element, Length, Padding};
use vedit_core::{startup_banner, FileNode};

pub fn view(state: &EditorState) -> Element<'_, Message> {
    let banner = text(startup_banner())
        .size(28)
        .horizontal_alignment(Horizontal::Center);

    let buffer = text_editor::TextEditor::new(state.buffer_content())
        .on_action(Message::BufferAction)
        .height(Length::Fill)
        .padding(12);

    let editor_panel = column![
        text("Active Buffer").size(16),
        container(buffer)
            .padding(4)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(theme::Container::Box),
    ]
    .spacing(12)
    .padding(16)
    .width(Length::FillPortion(3))
    .height(Length::Fill);

    let mut documents_column = column![text("Open Files").size(16)]
        .spacing(8)
        .padding([0, 0, 8, 0]);

    for (index, document) in state.editor().open_documents().iter().enumerate() {
        let is_active = index == state.editor().active_index();
        let mut label = document.display_name().to_string();
        if document.is_modified {
            label.push('*');
        }
        if is_active {
            label = format!("• {}", label);
        }

        let mut entry = button(text(label).size(14))
            .width(Length::Fill)
            .on_press(Message::DocumentSelected(index));

        if is_active {
            entry = entry.style(theme::Button::Primary);
        }

        documents_column = documents_column.push(entry);
    }

    let open_files_section = container(documents_column)
        .padding(16)
        .width(Length::Fill)
        .style(theme::Container::Box);

    let workspace_title = if let Some(root) = state.editor().workspace_root() {
        format!("Workspace: {}", root)
    } else {
        "Workspace".to_string()
    };

    let workspace_contents: Element<'_, Message> = if let Some((version, tree)) =
        state.editor().workspace_snapshot()
    {
        let snapshot = WorkspaceSnapshot::new(version, tree);
        lazy(snapshot, |snapshot| -> Element<'static, Message> {
            scrollable(render_workspace_nodes(snapshot.tree.as_slice(), 0))
                .height(Length::Fill)
                .into()
        })
        .into()
    } else {
        column![text("Open a folder to browse project files").size(14)]
            .width(Length::Fill)
            .height(Length::Shrink)
            .into()
    };

    let workspace_section = container(
        column![text(workspace_title).size(16), workspace_contents]
            .spacing(8)
            .height(Length::Fill),
    )
    .padding(16)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(theme::Container::Box);

    let side_panel = column![open_files_section, workspace_section]
        .spacing(16)
        .width(Length::FillPortion(2))
        .height(Length::Fill);

    let content_row = row![editor_panel, side_panel]
        .spacing(16)
        .width(Length::Fill)
        .height(Length::Fill);

    let top_bar = container(
        row![
            text("vedit").size(20),
            button(text("Open File…")).on_press(Message::OpenFileRequested),
            button(text("Open Folder…")).on_press(Message::WorkspaceOpenRequested),
        ]
        .spacing(16)
        .align_items(Alignment::Center),
    )
    .padding([12, 16])
    .width(Length::Fill)
    .style(theme::Container::Box);

    let workspace_status = format!(
        "Workspace: {}",
        state.editor().workspace_root().unwrap_or("(none)")
    );

    let status_bar = container(
        row![
            text(format!("File: {}", state.editor().status_line())).size(14),
            text(workspace_status).size(14),
            text(format!(
                "Chars: {}",
                state
                    .editor()
                    .active_document()
                    .map(|doc| doc.buffer.chars().count())
                    .unwrap_or(0)
            ))
            .size(14),
            if let Some(err) = state.error() {
                text(format!("Error: {}", err)).size(14)
            } else {
                text("").size(14)
            },
        ]
        .spacing(24)
        .align_items(Alignment::Center),
    )
    .padding([10, 16])
    .width(Length::Fill)
    .align_y(Vertical::Center);

    container(
        column![top_bar, banner, content_row, status_bar]
            .spacing(16)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_items(Alignment::Start),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x()
    .center_y()
    .into()
}

fn render_workspace_nodes(nodes: &[FileNode], indent: u16) -> Column<'static, Message> {
    nodes.iter().fold(Column::new().spacing(4), |column, node| {
        column.push(render_workspace_node(node, indent))
    })
}

fn render_workspace_node(node: &FileNode, indent: u16) -> Element<'static, Message> {
    let label = if node.is_directory {
        format!("{}/", node.name)
    } else {
        node.name.clone()
    };

    let entry: Element<'static, Message> = if node.is_directory {
        text(label).size(14).into()
    } else {
        button(text(label).size(14))
            .style(theme::Button::Text)
            .width(Length::Fill)
            .on_press(Message::WorkspaceFileActivated(node.path.clone()))
            .into()
    };

    let indent_padding = indent.saturating_mul(16);

    let mut column = Column::new();
    column = column.push(container(entry).padding(Padding::from([0, 0, 0, indent_padding])));

    if node.is_directory && !node.children.is_empty() {
        column = column.push(render_workspace_nodes(&node.children, indent + 1));
    }

    column.into()
}
