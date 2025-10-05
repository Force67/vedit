use crate::message::{Message, WorkspaceSnapshot};
use crate::state::EditorState;
use crate::syntax::{format_highlight, SyntaxHighlighter};
use crate::widgets::text_editor::TextEditor as EditorWidget;
use crate::style::{
    active_document_button, document_button, panel_container, ribbon_container,
    root_container, status_container, top_bar_button,
};
use iced::alignment::{Horizontal, Vertical};
use iced::widget::lazy;
use iced::widget::{
    button, column, container, horizontal_space, row, scrollable, text, text_input, Column, Rule,
    vertical_slider,
};
use iced::widget::slider;
use iced::{theme, Alignment, Color, Element, Font, Length, Padding};
use std::collections::HashSet;
use std::path::Path;
use vedit_core::{FileNode, NodeKind};
use vedit_application::{SettingsCategory, SETTINGS_CATEGORIES};

pub fn view(state: &EditorState) -> Element<'_, Message> {
    let scale = state.scale_factor() as f32;
    let spacing_large = (16.0 * scale).max(8.0);
    let spacing_medium = (12.0 * scale).max(6.0);
    let spacing_small = (8.0 * scale).max(4.0);

    let top_bar = render_top_bar(state, scale, spacing_large, spacing_medium);

    let mut layout = column![top_bar];

    if state.settings().is_open() {
        layout = layout.push(render_settings(state, scale, spacing_large, spacing_medium, spacing_small));
    } else {
        if state.command_palette().is_open() {
            layout = layout.push(render_command_palette(state));
        }
        layout = layout.push(render_editor_content(
            state,
            scale,
            spacing_large,
            spacing_medium,
            spacing_small,
        ));
        layout = layout.push(render_status_bar(state, scale, spacing_small, spacing_large));
    }

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

fn render_top_bar(
    state: &EditorState,
    scale: f32,
    spacing_large: f32,
    spacing_medium: f32,
) -> Element<'_, Message> {
    let mut row = row![
        text("vedit")
            .size((20.0 * scale).max(14.0))
            .style(Color::from_rgb8(0, 120, 215)),
        button(text("Open File…").size((14.0 * scale).max(10.0)))
            .style(top_bar_button())
            .on_press(Message::OpenFileRequested),
        button(text("Open Folder…").size((14.0 * scale).max(10.0)))
            .style(top_bar_button())
            .on_press(Message::WorkspaceOpenRequested),
        button(text("Open Solution…").size((14.0 * scale).max(10.0)))
            .style(top_bar_button())
            .on_press(Message::SolutionOpenRequested),
        {
            let label = if state.command_palette().is_open() {
                "Command Prompt ▲"
            } else {
                "Command Prompt ▼"
            };

            button(text(label).size((14.0 * scale).max(10.0)))
                .style(top_bar_button())
                .on_press(Message::CommandPromptToggled)
        },
        horizontal_space().width(Length::Fill),
    ]
    .spacing(spacing_large)
    .align_items(Alignment::Center);

    let (label, message) = if state.settings().is_open() {
        ("Close Settings", Message::SettingsClosed)
    } else {
        ("Settings", Message::SettingsOpened)
    };

    row = row.push(
        button(text(label).size((14.0 * scale).max(10.0)))
            .style(top_bar_button())
            .on_press(message),
    );

    container(row)
        .padding([spacing_medium, spacing_large])
        .width(Length::Fill)
        .style(ribbon_container())
        .into()
}

fn render_editor_content(
    state: &EditorState,
    scale: f32,
    spacing_large: f32,
    spacing_medium: f32,
    spacing_small: f32,
) -> Element<'_, Message> {
    let editor_padding = (12.0 * scale).max(6.0);
    let scroll_metrics = state.buffer_scroll_metrics();
    let max_scroll = scroll_metrics.max_scroll() as f32;
    let scroll_value = scroll_metrics.scroll as f32;
    let scrollbar_width = (8.0 * scale).clamp(6.0, 12.0);
    let slider_position = (max_scroll - scroll_value).clamp(0.0, max_scroll);
    let scrollbar = vertical_slider::VerticalSlider::<f32, Message>::new(
        0.0..=max_scroll,
        slider_position,
        move |value| Message::BufferScrollChanged(max_scroll - value),
    )
    .step(1.0_f32)
    .width(scrollbar_width)
    .height(Length::Fill)
    .style(theme::Slider::Custom(Box::new(EditorScrollbarStyle)));

    let buffer = EditorWidget::new(state.buffer_content())
        .font(Font::MONOSPACE)
        .highlight::<SyntaxHighlighter>(state.syntax_settings(), format_highlight)
        .line_number_color(Color::from_rgb8(133, 133, 133))
        .padding(editor_padding)
        .on_action(Message::BufferAction)
        .height(Length::Fill);

    let buffer_panel: Element<'_, Message> = container(buffer)
        .padding((4.0 * scale).max(2.0))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(panel_container())
        .into();

    let scrollbar_track: Element<'_, Message> = container(scrollbar)
        .width(Length::Fixed(scrollbar_width))
        .height(Length::Fill)
        .center_x()
        .into();

    let buffer_content = row![
        buffer_panel,
        scrollbar_track,
    ]
    .spacing((6.0 * scale).max(3.0))
    .align_items(Alignment::Start)
    .width(Length::Fill)
    .height(Length::Fill);

    let editor_panel = column![
        text("Active Buffer").size((16.0 * scale).max(12.0)),
        buffer_content,
    ]
    .spacing(spacing_medium)
    .padding(spacing_large)
    .width(Length::Fill)
    .height(Length::Fill);

    let sidebar_width = (240.0 / state.scale_factor()).clamp(180.0, 320.0) as f32;

    let open_panel = render_open_files_panel(
        state,
        scale,
        spacing_large,
        spacing_medium,
        sidebar_width,
    );

    let workspace_panel = render_workspace_panel(
        state,
        scale,
        spacing_large,
        spacing_small,
        sidebar_width,
    );

    row![open_panel, editor_panel, workspace_panel]
        .spacing(spacing_large)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

struct EditorScrollbarStyle;

impl slider::StyleSheet for EditorScrollbarStyle {
    type Style = theme::Theme;

    fn active(&self, theme: &Self::Style) -> slider::Appearance {
        let palette = theme.extended_palette();
        slider::Appearance {
            rail: slider::Rail {
                colors: (
                    palette.background.base.color,
                    palette.background.base.color,
                ),
                width: 4.0,
                border_radius: 2.0.into(),
            },
            handle: slider::Handle {
                shape: slider::HandleShape::Circle { radius: 5.0 },
                color: palette.primary.weak.color,
                border_color: palette.primary.strong.color,
                border_width: 1.0,
            },
        }
    }

    fn hovered(&self, theme: &Self::Style) -> slider::Appearance {
        let mut active = self.active(theme);
        active.handle.color = theme.extended_palette().primary.base.color;
        active
    }

    fn dragging(&self, theme: &Self::Style) -> slider::Appearance {
        let mut active = self.active(theme);
        active.handle.color = theme.extended_palette().primary.strong.color;
        active
    }
}

fn render_status_bar(
    state: &EditorState,
    scale: f32,
    spacing_small: f32,
    spacing_large: f32,
) -> Element<'_, Message> {
    let workspace_root = state.editor().workspace_root().unwrap_or("(none)");
    let workspace_status = if let Some(name) = state.workspace_display_name() {
        format!("Workspace: {} ({})", name, workspace_root)
    } else {
        format!("Workspace: {}", workspace_root)
    };

    let active_language = state
        .editor()
        .active_document()
        .map(|doc| doc.language().display_name())
        .unwrap_or("Plain Text");

    container(
        row![
            text(format!("File: {}", state.editor().status_line())).size((14.0 * scale).max(10.0)),
            text(format!("Language: {}", active_language)).size((14.0 * scale).max(10.0)),
            text(workspace_status).size((14.0 * scale).max(10.0)),
            text(format!(
                "Chars: {}",
                state
                    .editor()
                    .active_document()
                    .map(|doc| doc.buffer.char_count())
                    .unwrap_or(0)
            ))
            .size((14.0 * scale).max(10.0)),
            text(state.format_scale_factor()).size((14.0 * scale).max(10.0)),
            match (state.error(), state.workspace_notice()) {
                (Some(err), _) => text(format!("Error: {}", err)).size((14.0 * scale).max(10.0)),
                (None, Some(notice)) => text(notice)
                    .size((14.0 * scale).max(10.0))
                    .style(Color::from_rgb8(38, 139, 210)),
                _ => text("").size(14),
            },
        ]
        .spacing((24.0 * scale).max(12.0))
        .align_items(Alignment::Center),
    )
    .padding([spacing_small, spacing_large])
    .width(Length::Fill)
    .align_y(Vertical::Center)
    .style(status_container())
    .into()
}

fn render_settings(
    state: &EditorState,
    scale: f32,
    spacing_large: f32,
    spacing_medium: f32,
    spacing_small: f32,
) -> Element<'_, Message> {
    let mut categories_list = column![text("Categories").size((16.0 * scale).max(12.0))]
        .spacing(spacing_small);

    for category in SETTINGS_CATEGORIES.iter().copied() {
        let label = category.label();
        let mut entry = button(text(label).size((14.0 * scale).max(10.0)))
            .style(document_button())
            .width(Length::Fill)
            .on_press(Message::SettingsCategorySelected(category));

        if category == state.settings().selected_category() {
            entry = entry.style(active_document_button());
        }

        categories_list = categories_list.push(entry);
    }

    let categories_panel = container(categories_list)
        .padding(spacing_large)
        .width(Length::Fixed((220.0 * scale).max(160.0)))
        .style(panel_container());

    let detail: Element<'_, Message> = match state.settings().selected_category() {
        SettingsCategory::Keybindings =>
            render_keybindings_settings(state, scale, spacing_large, spacing_medium, spacing_small),
    };

    row![categories_panel, detail]
        .spacing(spacing_large)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn render_keybindings_settings(
    state: &EditorState,
    scale: f32,
    spacing_large: f32,
    spacing_medium: f32,
    spacing_small: f32,
) -> Element<'_, Message> {
    let mut content = column![
        row![
            text("Quick Command Shortcuts").size((16.0 * scale).max(12.0)),
            horizontal_space().width(Length::Fill),
            {
                let button_label = text("Save Keybindings").size((14.0 * scale).max(10.0));
                let base = button(button_label);
                if state.settings_dirty() {
                    base.on_press(Message::SettingsBindingsSaveRequested)
                } else {
                    base
                }
            },
        ]
        .spacing(spacing_small)
        .align_items(Alignment::Center),
        text("Assign keyboard shortcuts to launch quick actions directly.")
            .size((14.0 * scale).max(10.0)),
    ]
    .spacing(spacing_small);

    let keymap_path = state
        .keymap_path_display()
        .unwrap_or_else(|| "(default: ./keybindings.toml)".to_string());

    content = content.push(
        row![
            text(format!("Keymap file: {}", keymap_path)).size((13.0 * scale).max(9.0)),
            horizontal_space().width(Length::Fill),
            button(text("Change File…").size((13.0 * scale).max(9.0)))
                .on_press(Message::SettingsKeymapPathRequested),
        ]
        .spacing(spacing_small)
        .align_items(Alignment::Center),
    );

    if let Some(notice) = state.settings_notice() {
        content = content.push(
            text(notice)
                .size((13.0 * scale).max(9.0))
                .style(Color::from_rgb8(38, 139, 210)),
        );
    }

    if let Some(err) = state.settings_error() {
        content = content.push(
            text(err)
                .size((13.0 * scale).max(9.0))
                .style(Color::from_rgb8(220, 50, 47)),
        );
    }

    for command in state
        .quick_commands()
        .iter()
        .filter(|cmd| cmd.action.is_some())
    {
        let id = command.id;
        let binding_value = state.settings().binding_input(id);
        let field = text_input("e.g. Ctrl+Alt+K", binding_value)
            .padding(Padding::new((4.0 * scale).max(2.0)))
            .on_input(move |value| Message::SettingsBindingChanged(id, value))
            .on_submit(Message::SettingsBindingApplied(id))
            .width(Length::FillPortion(2));

        let apply_button = button(text("Assign").size((14.0 * scale).max(10.0)))
            .on_press(Message::SettingsBindingApplied(id));

        let mut entry = column![
            text(command.title).size((14.0 * scale).max(10.0)),
            text(command.description)
                .size((12.0 * scale).max(9.0))
                .style(Color::from_rgb8(170, 170, 170)),
            row![field, apply_button]
                .spacing(spacing_small)
                .align_items(Alignment::Center),
        ]
        .spacing(spacing_small)
        .padding([spacing_small, 0.0, spacing_small, 0.0]);

        if let Some(err) = state.settings().binding_error(id) {
            entry = entry.push(
                text(err)
                    .size((12.0 * scale).max(9.0))
                    .style(Color::from_rgb8(220, 50, 47)),
            );
        }

        content = content.push(entry);
    }

    container(content.spacing(spacing_medium))
        .padding(spacing_large)
        .width(Length::Fill)
        .style(panel_container())
        .into()
}

fn render_open_files_panel(
    state: &EditorState,
    scale: f32,
    spacing_large: f32,
    spacing_medium: f32,
    sidebar_width: f32,
) -> Element<'_, Message> {
    let list_spacing = ((6.0 * scale).max(3.0)).round() as u16;

    let mut open_list = Column::new().spacing(list_spacing);
    for (index, document) in state.editor().open_documents().iter().enumerate() {
        let is_active = index == state.editor().active_index();
        let mut title = document.display_name().to_string();
        if document.is_modified {
            title.push('*');
        }

        let mut label = column![text(&title).size((14.0 * scale).max(10.0))]
        .spacing((4.0 * scale).max(2.0));

        if let Some(path) = document.path.as_deref() {
            if !path.is_empty() {
                label = label.push(
                    text(path)
                        .size((12.0 * scale).max(9.0))
                        .style(Color::from_rgb8(150, 150, 150)),
                );
            }
        }

        let mut entry = button(label)
            .padding((6.0 * scale).max(3.0))
            .width(Length::Fill)
            .style(document_button())
            .on_press(Message::DocumentSelected(index));

        if is_active {
            entry = entry.style(active_document_button());
        }

        open_list = open_list.push(entry);
    }

    let open_scroll = scrollable(open_list).height(Length::Fill);

    let mut content = column![
        text("Open Files").size((16.0 * scale).max(12.0)),
        open_scroll,
    ]
    .spacing(spacing_medium)
    .height(Length::Fill);

    let recent_files = state.workspace_recent_files();
    if !recent_files.is_empty() {
        let mut recent_column = Column::new().spacing(list_spacing);
        for path in recent_files {
            let display = Path::new(&path)
                .file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.to_string())
                .unwrap_or(path.clone());

            let label = column![
                text(display).size((14.0 * scale).max(10.0)),
                text(path.clone())
                    .size((12.0 * scale).max(9.0))
                    .style(Color::from_rgb8(150, 150, 150)),
            ]
            .spacing((4.0 * scale).max(2.0));

            recent_column = recent_column.push(
                button(label)
                    .style(theme::Button::Text)
                    .width(Length::Fill)
                    .on_press(Message::WorkspaceFileActivated(path.clone())),
            );
        }

        content = content
            .push(Rule::horizontal(1))
            .push(text("Recent Files").size((14.0 * scale).max(10.0)))
            .push(recent_column);
    }

    container(content)
        .padding(spacing_large)
        .width(Length::Fixed(sidebar_width))
        .height(Length::Fill)
        .style(panel_container())
        .into()
}

fn render_workspace_panel(
    state: &EditorState,
    scale: f32,
    spacing_large: f32,
    spacing_small: f32,
    sidebar_width: f32,
) -> Element<'_, Message> {
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
        let collapsed_paths = state.workspace_collapsed_paths();
        let collapsed_version = state.workspace_collapsed_version();
        lazy(
            (snapshot, scale_key, collapsed_version, collapsed_paths),
            |(snapshot, scale_key, _version, collapsed_paths)| -> Element<'static, Message> {
                let scale = *scale_key as f32 / 100.0;
                let collapsed_set: HashSet<String> = collapsed_paths.iter().cloned().collect();
                scrollable(render_workspace_nodes(
                    snapshot.tree.as_slice(),
                    0,
                    scale,
                    &collapsed_set,
                ))
                .height(Length::Fill)
                .into()
            },
        )
        .into()
    } else {
        column![text("Open a folder to browse project files").size((14.0 * scale).max(10.0))]
            .width(Length::Fill)
            .height(Length::Shrink)
            .into()
    };

    let mut sticky_list = column![]
        .spacing(spacing_small)
        .width(Length::Fill);
    let sticky_notes = state.active_sticky_notes();
    if sticky_notes.is_empty() {
        sticky_list = sticky_list.push(
            text("No sticky notes for this file").size((12.0 * scale).max(9.0)),
        );
    } else {
        for note in sticky_notes {
            let note_id = note.id;
            let header = text(format!("Line {}, Column {}", note.line, note.column))
                .size((12.0 * scale).max(9.0));
            let input = text_input("Add a note…", &note.content)
                .on_input(move |value| Message::StickyNoteContentChanged(note_id, value))
                .padding(spacing_small / 2.0)
                .size((14.0 * scale).max(10.0))
                .width(Length::Fill);
            let remove = button(text("Remove").size((12.0 * scale).max(9.0)))
                .style(theme::Button::Text)
                .on_press(Message::StickyNoteDeleted(note_id));
            let entry = column![
                header,
                input,
                row![horizontal_space().width(Length::Fill), remove]
                    .align_items(Alignment::Center),
            ]
            .spacing(spacing_small / 2.0)
            .width(Length::Fill);
            sticky_list = sticky_list.push(
                container(entry)
                    .padding(spacing_small)
                    .width(Length::Fill)
                    .style(panel_container()),
            );
        }
    }

    let add_button = button(text("Add Sticky Note").size((14.0 * scale).max(10.0)))
        .style(theme::Button::Primary)
        .on_press(Message::StickyNoteCreateRequested);

    let sticky_section = column![
        text("Sticky Notes").size((16.0 * scale).max(12.0)),
        sticky_list,
        add_button,
    ]
    .spacing(spacing_small)
    .width(Length::Fill);

    container(
        column![
            text(workspace_title).size((16.0 * scale).max(12.0)),
            workspace_contents,
            Rule::horizontal(1),
            sticky_section,
        ]
        .spacing(spacing_small)
        .height(Length::Fill),
    )
    .padding(spacing_large)
    .width(Length::Fixed(sidebar_width))
    .height(Length::Fill)
    .style(panel_container())
    .into()
}

fn render_workspace_nodes(
    nodes: &[FileNode],
    indent: u16,
    scale: f32,
    collapsed: &HashSet<String>,
) -> Column<'static, Message> {
    let spacing = ((4.0 * scale).max(2.0)).round() as u16;
    nodes.iter().fold(Column::new().spacing(spacing), |column, node| {
        column.push(render_workspace_node(node, indent, scale, collapsed))
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
    let drop_width = (360.0 * scale).clamp(260.0, 520.0);

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

    let header = row![
        text("Command Prompt").size((18.0 * scale).max(14.0)),
        horizontal_space().width(Length::Fill),
        button(text("×").size((16.0 * scale).max(12.0)))
            .style(theme::Button::Text)
            .on_press(Message::CommandPaletteClosed),
    ]
    .spacing(spacing_small)
    .align_items(Alignment::Center);

    let palette_column = column![
        header,
        input,
        scrollable(command_list).height(Length::Fixed(240.0 * scale)),
    ]
    .spacing(spacing_medium)
    .width(Length::Fill);

    let dropdown = container(palette_column)
        .padding(spacing_large)
        .width(Length::Fixed(drop_width))
        .style(panel_container());

    container(dropdown)
        .width(Length::Fill)
        .padding([0.0, spacing_large, 0.0, spacing_large])
        .align_x(Horizontal::Left)
        .into()
}

fn render_workspace_node(
    node: &FileNode,
    indent: u16,
    scale: f32,
    collapsed: &HashSet<String>,
) -> Element<'static, Message> {
    let label_size = (14.0 * scale).max(10.0);
    let is_collapsed = node.is_directory && collapsed.contains(&node.path);
    let can_expand = node.is_directory && (node.has_children || !node.is_fully_scanned);
    let tag = match node.kind {
        NodeKind::Solution => "[SLN] ",
        NodeKind::Project | NodeKind::ProjectStub => "[PRJ] ",
        _ => "",
    };
    let base_label = if node.is_directory {
        format!("{}{}{}", tag, node.name, if matches!(node.kind, NodeKind::Solution | NodeKind::Project) { "" } else { "/" })
    } else {
        format!("{}{}", tag, node.name)
    };
    let entry: Element<'static, Message> = if node.is_directory {
        let indicator = if can_expand {
            if is_collapsed { "▸" } else { "▾" }
        } else if matches!(node.kind, NodeKind::Solution | NodeKind::Project | NodeKind::ProjectStub) {
            "◇"
        } else {
            "•"
        };
        let row_content = row![
            text(indicator).size(label_size),
            text(base_label).size(label_size),
        ]
        .spacing((6.0 * scale).max(3.0))
        .align_items(Alignment::Center);

        button(row_content)
            .style(theme::Button::Text)
            .width(Length::Fill)
            .on_press(Message::WorkspaceDirectoryToggled(node.path.clone()))
            .into()
    } else {
        button(text(base_label).size(label_size))
            .style(theme::Button::Text)
            .width(Length::Fill)
            .on_press(Message::WorkspaceFileActivated(node.path.clone()))
            .into()
    };

    let indent_padding = ((indent as f32 * 12.0 * scale).round() as u16).min(u16::MAX);

    let mut column = Column::new();
    column = column.push(container(entry).padding(Padding::from([0, 0, 0, indent_padding])));

    if node.is_directory && !is_collapsed && !node.children.is_empty() {
        column = column.push(render_workspace_nodes(
            &node.children,
            indent + 1,
            scale,
            collapsed,
        ));
    }

    column.into()
}
