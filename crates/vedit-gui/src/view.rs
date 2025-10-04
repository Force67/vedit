use crate::message::{Message, WorkspaceSnapshot};
use crate::state::EditorState;
use crate::syntax::{format_highlight, SyntaxHighlighter};
use crate::widgets::text_editor::TextEditor as EditorWidget;
use crate::style::{
    active_document_button, document_button, panel_container, ribbon_container,
    root_container, status_container, top_bar_button,
};
use iced::alignment::Vertical;
use iced::widget::lazy;
use iced::widget::{button, column, container, row, scrollable, text, text_input, Column};
use iced::{theme, Alignment, Color, Element, Length, Padding};
use vedit_core::FileNode;

pub fn view(state: &EditorState) -> Element<'_, Message> {
    let scale = state.scale_factor() as f32;
    let spacing_large = (16.0 * scale).max(8.0);
    let spacing_medium = (12.0 * scale).max(6.0);
    let spacing_small = (8.0 * scale).max(4.0);

    let buffer = EditorWidget::new(state.buffer_content())
        .highlight::<SyntaxHighlighter>(state.syntax_settings(), format_highlight)
        .line_number_color(Color::from_rgb8(133, 133, 133))
        .on_action(Message::BufferAction)
        .height(Length::Fill)
        .padding((12.0 * scale).max(6.0));

    let editor_panel = column![
        text("Active Buffer").size((16.0 * scale).max(12.0)),
        container(buffer)
            .padding((4.0 * scale).max(2.0))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(panel_container()),
    ]
    .spacing(spacing_medium)
    .padding(spacing_large)
    .width(Length::FillPortion(4))
    .height(Length::Fill);

    let mut documents_column = column![text("Open Files").size((16.0 * scale).max(12.0))]
        .spacing(spacing_small)
        .padding([0.0, 0.0, spacing_small, 0.0]);

    for (index, document) in state.editor().open_documents().iter().enumerate() {
        let is_active = index == state.editor().active_index();
        let mut label = document.display_name().to_string();
        if document.is_modified {
            label.push('*');
        }

        let mut entry = button(text(label).size((14.0 * scale).max(10.0)))
            .width(Length::Fill)
            .style(document_button())
            .on_press(Message::DocumentSelected(index));

        if is_active {
            entry = entry.style(active_document_button());
        }

        documents_column = documents_column.push(entry);
    }

    let open_files_section = container(documents_column)
        .padding(spacing_large)
        .width(Length::Fill)
        .style(panel_container());

    let workspace_title = if let Some(root) = state.editor().workspace_root() {
        format!("Workspace: {}", root)
    } else {
        "Workspace".to_string()
    };

    let workspace_contents: Element<'_, Message> = if let Some((version, tree)) =
        state.editor().workspace_snapshot()
    {
        let snapshot = WorkspaceSnapshot::new(version, tree);
        let scale_key = (scale * 100.0).round() as u32;
        lazy((snapshot, scale_key), |(snapshot, scale_key)| -> Element<'static, Message> {
            let scale = *scale_key as f32 / 100.0;
            scrollable(render_workspace_nodes(snapshot.tree.as_slice(), 0, scale))
                .height(Length::Fill)
                .into()
        })
        .into()
    } else {
        column![text("Open a folder to browse project files").size((14.0 * scale).max(10.0))]
            .width(Length::Fill)
            .height(Length::Shrink)
            .into()
    };

    let workspace_section = container(
        column![text(workspace_title).size((16.0 * scale).max(12.0)), workspace_contents]
            .spacing(spacing_small)
            .height(Length::Fill),
    )
    .padding(spacing_large)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(panel_container());

    let sidebar_width = (240.0 / state.scale_factor()).clamp(180.0, 320.0) as f32;

    let side_panel = column![open_files_section, workspace_section]
        .spacing(spacing_large)
        .width(Length::Fixed(sidebar_width))
        .height(Length::Fill);

    let content_row = row![editor_panel, side_panel]
        .spacing(spacing_large)
        .width(Length::Fill)
        .height(Length::Fill);

    let top_bar = container(
        row![
            text("vedit")
                .size((20.0 * scale).max(14.0))
                .style(Color::from_rgb8(0, 120, 215)),
            button(text("Open File…").size((14.0 * scale).max(10.0)))
                .style(top_bar_button())
                .on_press(Message::OpenFileRequested),
            button(text("Open Folder…").size((14.0 * scale).max(10.0)))
                .style(top_bar_button())
                .on_press(Message::WorkspaceOpenRequested),
        ]
        .spacing(spacing_large)
        .align_items(Alignment::Center),
    )
    .padding([spacing_medium, spacing_large])
    .width(Length::Fill)
    .style(ribbon_container());

    let workspace_status = format!(
        "Workspace: {}",
        state.editor().workspace_root().unwrap_or("(none)")
    );

    let active_language = state
        .editor()
        .active_document()
        .map(|doc| doc.language().display_name())
        .unwrap_or("Plain Text");

    let status_bar = container(
        row![
            text(format!("File: {}", state.editor().status_line())).size((14.0 * scale).max(10.0)),
            text(format!("Language: {}", active_language)).size((14.0 * scale).max(10.0)),
            text(workspace_status).size((14.0 * scale).max(10.0)),
            text(format!(
                "Chars: {}",
                state
                    .editor()
                    .active_document()
                    .map(|doc| doc.buffer.chars().count())
                    .unwrap_or(0)
            ))
            .size((14.0 * scale).max(10.0)),
            if let Some(err) = state.error() {
                text(format!("Error: {}", err)).size((14.0 * scale).max(10.0))
            } else {
                text("").size(14)
            },
        ]
        .spacing((24.0 * scale).max(12.0))
        .align_items(Alignment::Center),
    )
    .padding([spacing_small, spacing_large])
    .width(Length::Fill)
    .align_y(Vertical::Center)
    .style(status_container());

    let mut layout = column![top_bar];

    if state.command_palette().is_open() {
        layout = layout.push(render_command_palette(state));
    }

    layout = layout
        .push(content_row)
        .push(status_bar);

    container(
        layout
            .spacing(spacing_large)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_items(Alignment::Start),
    )
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x()
        .center_y()
        .style(root_container())
        .into()
}

fn render_workspace_nodes(nodes: &[FileNode], indent: u16, scale: f32) -> Column<'static, Message> {
    let spacing = ((4.0 * scale).max(2.0)).round() as u16;
    nodes.iter().fold(Column::new().spacing(spacing), |column, node| {
        column.push(render_workspace_node(node, indent, scale))
    })
}

fn render_command_palette(state: &EditorState) -> Element<'_, Message> {
    let palette = state.command_palette();
    let commands = state.quick_commands();
    let filtered = palette.filtered_indices(commands);
    let selection = palette.selection_index();
    let scale = state.scale_factor() as f32;
    let spacing_large = (16.0 * scale).max(8.0);
    let spacing_medium = (12.0 * scale).max(6.0);
    let spacing_small = (8.0 * scale).max(4.0);

    let submit_message = state
        .selected_quick_command()
        .map(Message::CommandPaletteCommandInvoked)
        .unwrap_or(Message::CommandPaletteClosed);

    let input = text_input("Type a command…", palette.query())
        .on_input(Message::CommandPaletteInputChanged)
        .on_submit(submit_message)
        .padding(spacing_small)
        .size((16.0 * scale).max(12.0))
        .width(Length::Fill);

    let mut command_list = column![]
        .spacing(spacing_small)
        .width(Length::Fill);

    if filtered.is_empty() {
        command_list = command_list.push(
            container(text("No commands match your search").size((14.0 * scale).max(10.0)))
                .padding(spacing_small)
                .width(Length::Fill)
                .style(panel_container()),
        );
    } else {
        for (position, index) in filtered.iter().enumerate() {
            if let Some(command) = commands.get(*index) {
                let label = column![
                    text(command.title).size((16.0 * scale).max(12.0)),
                    text(command.description).size((12.0 * scale).max(9.0)),
                ]
                .spacing(spacing_small / 2.0)
                .width(Length::Fill);

                let mut entry = button(label)
                    .padding(spacing_small)
                    .width(Length::Fill)
                    .on_press(Message::CommandPaletteCommandInvoked(command.id));

                if position == selection {
                    entry = entry.style(theme::Button::Primary);
                } else {
                    entry = entry.style(theme::Button::Text);
                }

                command_list = command_list.push(entry);
            }
        }
    }

    let palette_column = column![
        text("Quick Command Menu").size((18.0 * scale).max(14.0)),
        input,
        scrollable(command_list).height(Length::Fixed(240.0 * scale)),
    ]
    .spacing(spacing_medium)
    .width(Length::Fill);

    container(palette_column)
        .padding(spacing_large)
        .width(Length::Fill)
        .style(panel_container())
        .into()
}

fn render_workspace_node(node: &FileNode, indent: u16, scale: f32) -> Element<'static, Message> {
    let label_size = (14.0 * scale).max(10.0);
    let label = if node.is_directory {
        format!("{}/", node.name)
    } else {
        node.name.clone()
    };

    let entry: Element<'static, Message> = if node.is_directory {
        text(label).size(label_size).into()
    } else {
        button(text(label).size(label_size))
            .style(theme::Button::Text)
            .width(Length::Fill)
            .on_press(Message::WorkspaceFileActivated(node.path.clone()))
            .into()
    };

    let indent_padding = ((indent as f32 * 12.0 * scale).round() as u16).min(u16::MAX);

    let mut column = Column::new();
    column = column.push(container(entry).padding(Padding::from([0, 0, 0, indent_padding])));

    if node.is_directory && !node.children.is_empty() {
        column = column.push(render_workspace_nodes(&node.children, indent + 1, scale));
    }

    column.into()
}
