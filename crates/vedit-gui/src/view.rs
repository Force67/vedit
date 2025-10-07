use crate::console::{ConsoleKind, ConsoleLineKind, ConsoleStatus};
use crate::message::{Message, RightRailTab};
use crate::state::{
    EditorState, MakefileEntry, SolutionBrowserEntry, SolutionErrorEntry, SolutionTreeNode,
    VisualStudioProjectEntry, VisualStudioSolutionEntry,
};
use crate::syntax::{format_highlight, SyntaxHighlighter};
use crate::widgets::debugger;
use crate::widgets::text_editor::TextEditor as EditorWidget;
use crate::style::{
    active_document_button, document_button, floating_panel_container, notification_container, overlay_container, panel_container,
    ribbon_container, root_container, status_container, top_bar_button, NotificationTone,
};
use crate::notifications::{Notification, NotificationKind};
use iced::alignment::{Horizontal, Vertical};

use iced::widget::{
    button, column, container, horizontal_space, mouse_area, pick_list, row, scrollable, text, text_input, Column, Row,
    vertical_slider,
};


use iced::Pixels;
use iced::widget::slider;
use iced::{theme, Alignment, Color, Element, Font, Length, Padding, Point};


use vedit_application::{SettingsCategory, SETTINGS_CATEGORIES};

pub fn view(state: &EditorState) -> Element<'_, Message> {
    let scale = state.scale_factor() as f32;
    let spacing_large = (16.0 * scale).max(8.0);
    let spacing_medium = (12.0 * scale).max(6.0);
    let spacing_small = (8.0 * scale).max(4.0);

    let title_bar = render_title_bar(state, scale, spacing_large, spacing_medium, spacing_small);

    let mut layout = column![title_bar];

    if state.debugger_menu_open() {
        layout = layout.push(debugger::menu(
            state.debugger(),
            scale,
            spacing_large,
            spacing_medium,
            spacing_small,
        ));
    }

    let mut main_element = if state.settings().is_open() {
        layout.push(render_settings(state, scale, spacing_large, spacing_medium, spacing_small))
    } else {
        layout
            .push(render_editor_content(
                state,
                scale,
                spacing_large,
                spacing_medium,
                spacing_small,
            ))
            .push(render_status_bar(state, scale, spacing_small, spacing_large))
    };

    if state.has_notifications() {
        main_element = main_element.push(render_notifications(state, scale, spacing_large, spacing_medium));
    }

    let main_container = container(
        main_element
            .spacing(spacing_large)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_items(Alignment::Start),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x()
    .center_y()
    .style(root_container());

    if state.command_palette().is_open() {
        render_command_palette(state)
    } else {
        main_container.into()
    }
}

fn render_title_bar(
    state: &EditorState,
    scale: f32,
    spacing_large: f32,
    spacing_medium: f32,
    spacing_small: f32,
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
        {
            let label = if state.console().is_visible() {
                "Terminal ▲"
            } else {
                "Terminal ▼"
            };

            button(text(label).size((14.0 * scale).max(10.0)))
                .style(top_bar_button())
                .on_press(Message::ConsoleVisibilityToggled)
        },
        button(text("▶ Run").size((14.0 * scale).max(10.0)))
            .style(top_bar_button())
            .on_press(Message::DebuggerLaunchRequested),
        button(text("■ Stop").size((14.0 * scale).max(10.0)))
            .style(top_bar_button())
            .on_press(Message::DebuggerStopRequested),
        {
            let summary = state.debugger().selection_summary();
            let arrow = if state.debugger_menu_open() { '▲' } else { '▼' };
            let label = format!("{} {}", summary, arrow);
            button(text(label).size((14.0 * scale).max(10.0)))
                .style(top_bar_button())
                .on_press(Message::DebuggerMenuToggled)
        },
        horizontal_space().width(Length::Fill),
    ]
    .spacing(spacing_large)
    .align_items(Alignment::Center);

    let message = if state.settings().is_open() {
        Message::SettingsClosed
    } else {
        Message::SettingsOpened
    };

    let settings_button = button(text("⚙").size((16.0 * scale).max(12.0)))
        .style(top_bar_button())
        .on_press(message);

    let window_buttons = row![
        button(text("—").size((14.0 * scale).max(10.0)))
            .style(top_bar_button())
            .on_press(Message::WindowMinimize),
        button(text("□").size((14.0 * scale).max(10.0)))
            .style(top_bar_button())
            .on_press(Message::WindowMaximize),
        button(text("×").size((14.0 * scale).max(10.0)))
            .style(top_bar_button())
            .on_press(Message::WindowClose),
    ]
    .spacing(spacing_small);

    row = row.push(horizontal_space().width(Length::Fill));
    row = row.push(settings_button);
    row = row.push(horizontal_space().width(Length::Fixed(20.0)));
    row = row.push(window_buttons);

    let title_bar = mouse_area(row)
        .on_press(Message::WindowDragStart);

    container(title_bar)
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

    let font_size = Pixels((14.0 * state.code_font_zoom()) as f32);
    let buffer = EditorWidget::new(state.buffer_content())
        .font(Font::MONOSPACE)
        .font_size(font_size)
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

    let tab_bar: iced::widget::Row<'_, Message, iced::Theme, iced::Renderer> = Row::with_children(vec![
        {
            let mut btn = button(text("Workspace").style(iced::theme::Text::Color(crate::style::TEXT))).style(crate::style::custom_button()).on_press(Message::RightRailTabSelected(RightRailTab::Workspace));
            if state.selected_right_rail_tab() == RightRailTab::Workspace {
                btn = btn.style(crate::style::active_document_button());
            }
            btn.into()
        },
        {
            let mut btn = button(text("Solutions").style(iced::theme::Text::Color(crate::style::MUTED))).style(crate::style::custom_button()).on_press(Message::RightRailTabSelected(RightRailTab::Solutions));
            if state.selected_right_rail_tab() == RightRailTab::Solutions {
                btn = btn.style(crate::style::active_document_button());
            }
            btn.into()
        },
        {
            let mut btn = button(text("Outline").style(iced::theme::Text::Color(crate::style::MUTED))).style(crate::style::custom_button()).on_press(Message::RightRailTabSelected(RightRailTab::Outline));
            if state.selected_right_rail_tab() == RightRailTab::Outline {
                btn = btn.style(crate::style::active_document_button());
            }
            btn.into()
        },
        {
            let mut btn = button(text("Search").style(iced::theme::Text::Color(crate::style::MUTED))).style(crate::style::custom_button()).on_press(Message::RightRailTabSelected(RightRailTab::Search));
            if state.selected_right_rail_tab() == RightRailTab::Search {
                btn = btn.style(crate::style::active_document_button());
            }
            btn.into()
        },
        {
            let mut btn = button(text("Problems").style(iced::theme::Text::Color(crate::style::MUTED))).style(crate::style::custom_button()).on_press(Message::RightRailTabSelected(RightRailTab::Problems));
            if state.selected_right_rail_tab() == RightRailTab::Problems {
                btn = btn.style(crate::style::active_document_button());
            }
            btn.into()
        },
        {
            let mut btn = button(text("Notes").style(iced::theme::Text::Color(crate::style::MUTED))).style(crate::style::custom_button()).on_press(Message::RightRailTabSelected(RightRailTab::Notes));
            if state.selected_right_rail_tab() == RightRailTab::Notes {
                btn = btn.style(crate::style::active_document_button());
            }
            btn.into()
        },
    ])
    .spacing(0);

    let workspace_content: Element<'_, Message> = match state.selected_right_rail_tab() {
        RightRailTab::Workspace => {
            if let Some(explorer) = state.file_explorer() {
                explorer.view().map(Message::FileExplorer)
            } else {
                scrollable(
                    column![text("Open a folder to browse project files").size((14.0 * scale).max(10.0))]
                        .spacing(4)
                        .padding(Padding::from([8.0, 16.0]))
                )
                .height(Length::Fill)
                .style(crate::style::custom_scrollable())
                .into()
            }
        }
        RightRailTab::Solutions => {
            if let Some(_root) = state.editor().workspace_root() {
                scrollable(render_solutions_tab(state, scale))
                    .style(crate::style::custom_scrollable())
                    .into()
            } else {
                scrollable(
                    column![text("Open a folder to view solutions").style(iced::theme::Text::Color(crate::style::TEXT))]
                        .spacing(4)
                        .padding(8)
                )
                .style(crate::style::custom_scrollable())
                .into()
            }
        }
        _ => {
            scrollable(
                column![text("Not implemented yet").style(iced::theme::Text::Color(crate::style::TEXT))]
                    .spacing(4)
                    .padding(8)
            )
            .style(crate::style::custom_scrollable())
            .into()
        }
    };

    let workspace_panel: Element<'_, Message> = container(
        column![tab_bar, workspace_content]
            .spacing(0)
    )
    .style(panel_container())
    .width(Length::Fixed(sidebar_width))
    .height(Length::Fill)
    .into();

    let content_row = row![open_panel, editor_panel, workspace_panel]
        .spacing(spacing_large)
        .width(Length::Fill)
        .height(Length::Fill);

    let mut layout = column![content_row]
        .spacing(spacing_large)
        .width(Length::Fill)
        .height(Length::Fill);

    if state.console().is_visible() {
        let header: iced::widget::Row<'_, Message, iced::Theme, iced::Renderer> = row![
            pick_list(
                vec!["Terminal".to_string(), "Debug".to_string(), "Output".to_string()],
                Some("Terminal".to_string()),
                |_| Message::DocumentSelected(0), // dummy
            ),
            button(text("▼").style(iced::theme::Text::Color(crate::style::TEXT)))
                .style(crate::style::custom_button())
                .on_press(Message::ConsoleVisibilityToggled),
        ]
        .spacing(8)
        .align_items(Alignment::Center);

        let log_view = scrollable(
            column![
                text("Application started").style(iced::theme::Text::Color(crate::style::TEXT)),
                text("Warning: deprecated function").style(iced::theme::Text::Color(crate::style::WARNING)),
                text("Error: file not found").style(iced::theme::Text::Color(crate::style::ERROR)),
            ]
            .spacing(4)
            .padding(8)
        )
        .style(crate::style::custom_scrollable());

        let content = column![header, log_view].spacing(8);

        layout = layout.push(
            container(content)
                .style(status_container())
                .width(Length::Fill)
                .height(Length::Fixed(200.0))
        );
    }

    layout.into()
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
            text(state.format_code_font_zoom()).size((14.0 * scale).max(10.0)),
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

fn render_notifications(
    state: &EditorState,
    scale: f32,
    spacing_large: f32,
    spacing_medium: f32,
) -> Element<'_, Message> {
    let bubble_spacing = (spacing_medium * 0.6).max(6.0);
    let mut stack = column![]
        .spacing(bubble_spacing)
        .align_items(Alignment::End);

    for notification in state.notifications() {
        stack = stack.push(render_notification_card(notification, scale));
    }

    let overlay = row![horizontal_space().width(Length::Fill), stack]
        .spacing(bubble_spacing)
        .align_items(Alignment::End);

    container(overlay)
        .width(Length::Fill)
        .height(Length::Shrink)
        .padding([0.0, spacing_large, spacing_large, spacing_large])
        .align_y(Vertical::Bottom)
        .into()
}

fn render_notification_card(notification: &Notification, scale: f32) -> Element<'_, Message> {
    let tone = notification_tone(notification.kind);
    let accent = notification_accent(notification.kind);
    let padding = (12.0 * scale).max(8.0);
    let spacing = (10.0 * scale).max(6.0);
    let icon_size = (14.0 * scale).max(10.0);

    let icon = container(text("●").size(icon_size).style(accent))
    .width(Length::Fixed((icon_size + 4.0).max(12.0)))
    .center_x()
    .center_y();

    let mut body = column![
        text(&notification.title)
            .size((15.0 * scale).max(11.0))
            .style(Color::from_rgb8(240, 240, 240)),
    ]
    .spacing((4.0 * scale).max(2.0));

    if let Some(details) = notification.body() {
        body = body.push(
            text(details)
                .size((13.0 * scale).max(9.5))
                .style(Color::from_rgb8(190, 190, 190)),
        );
    }

    let close_button = button(text("✕").size((14.0 * scale).max(10.0)))
        .style(theme::Button::Text)
        .on_press(Message::NotificationDismissed(notification.id));

    let content = row![icon, body.width(Length::Fill), close_button]
        .spacing(spacing)
        .align_items(Alignment::Center);

    container(content)
        .padding(Padding::new(padding))
        .max_width((320.0 * scale).max(220.0))
        .style(notification_container(tone))
        .into()
}

fn notification_tone(kind: NotificationKind) -> NotificationTone {
    match kind {
        NotificationKind::Info => NotificationTone::Info,
        NotificationKind::Success => NotificationTone::Success,
        NotificationKind::Error => NotificationTone::Error,
    }
}

fn notification_accent(kind: NotificationKind) -> Color {
    match kind {
        NotificationKind::Info => Color::from_rgb8(52, 152, 219),
        NotificationKind::Success => Color::from_rgb8(39, 174, 96),
        NotificationKind::Error => Color::from_rgb8(231, 76, 60),
    }
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

fn render_solutions_tab(state: &EditorState, scale: f32) -> Column<'_, Message> {
    let mut content = column![
        text("Solutions")
            .size((16.0 * scale).max(12.0))
            .style(iced::theme::Text::Color(crate::style::TEXT))
    ]
    .spacing((6.0 * scale).max(3.0))
    .padding(Padding::from([8.0, 16.0]));

    let entries = state.workspace_solutions();
    if entries.is_empty() {
        content = content.push(
            text("No solutions or Makefiles found")
                .style(iced::theme::Text::Color(crate::style::MUTED))
                .size((13.0 * scale).max(9.0)),
        );
        return content;
    }

    for entry in entries {
        content = content.push(render_solution_entry(entry, scale));
    }

    content
}

fn render_solution_entry(entry: &SolutionBrowserEntry, scale: f32) -> Element<'_, Message> {
    match entry {
        SolutionBrowserEntry::VisualStudio(solution) => {
            render_visual_studio_solution(solution, scale)
        }
        SolutionBrowserEntry::Makefile(makefile) => render_makefile_entry(makefile, scale),
        SolutionBrowserEntry::Error(error) => render_solution_error(error, scale),
    }
}

fn render_visual_studio_solution(
    solution: &VisualStudioSolutionEntry,
    scale: f32,
) -> Element<'_, Message> {
    let spacing = (4.0 * scale).max(2.0);
    let mut content = column![
        button(
            text(format!("🟦 {}", solution.name))
                .style(iced::theme::Text::Color(crate::style::TEXT))
                .size((14.0 * scale).max(10.0)),
        )
        .style(document_button())
        .on_press(Message::SolutionSelected(solution.path.clone()))
    ]
    .spacing(spacing);

    for warning in &solution.warnings {
        content = content.push(
            row![
                horizontal_space().width(Length::Fixed(16.0)),
                text(warning)
                    .style(iced::theme::Text::Color(crate::style::WARNING))
                    .size((12.0 * scale).max(9.0)),
            ]
            .spacing(spacing)
            .align_items(Alignment::Center),
        );
    }

    for project in &solution.projects {
        content = content.push(render_visual_studio_project(project, scale));
    }

    content.into()
}

fn render_visual_studio_project(
    project: &VisualStudioProjectEntry,
    scale: f32,
) -> Element<'_, Message> {
    let spacing = (3.0 * scale).max(2.0);
    let mut column = Column::new().spacing(spacing);

    let header = row![
        horizontal_space().width(Length::Fixed(16.0)),
        text("🛠").size((13.0 * scale).max(9.0)),
        text(&project.name)
            .style(iced::theme::Text::Color(crate::style::TEXT))
            .size((13.0 * scale).max(9.0)),
    ]
    .spacing(spacing)
    .align_items(Alignment::Center);

    column = column.push(
        button(header)
            .style(document_button())
            .on_press(Message::WorkspaceFileActivated(project.path.clone())),
    );

    if let Some(error) = &project.load_error {
        column = column.push(
            row![
                horizontal_space().width(Length::Fixed(32.0)),
                text(error)
                    .style(iced::theme::Text::Color(crate::style::ERROR))
                    .size((12.0 * scale).max(9.0)),
            ]
            .spacing(spacing)
            .align_items(Alignment::Center),
        );
    } else if !project.files.is_empty() {
        column = column.push(render_solution_node_column(&project.files, 32.0, scale));
    }

    column.into()
}

fn render_makefile_entry(makefile: &MakefileEntry, scale: f32) -> Element<'_, Message> {
    let spacing = (4.0 * scale).max(2.0);
    let mut column = column![
        button(
            text(format!("⚙ {}", makefile.name))
                .style(iced::theme::Text::Color(crate::style::TEXT))
                .size((14.0 * scale).max(10.0)),
        )
        .style(document_button())
        .on_press(Message::WorkspaceFileActivated(makefile.path.clone()))
    ]
    .spacing(spacing);

    if makefile.files.is_empty() {
        column = column.push(
            row![
                horizontal_space().width(Length::Fixed(16.0)),
                text("No referenced files detected")
                    .style(iced::theme::Text::Color(crate::style::MUTED))
                    .size((12.0 * scale).max(9.0)),
            ]
            .spacing(spacing)
            .align_items(Alignment::Center),
        );
    } else {
        column = column.push(render_solution_node_column(&makefile.files, 16.0, scale));
    }

    column.into()
}

fn render_solution_error(error: &SolutionErrorEntry, scale: f32) -> Element<'_, Message> {
    column![
        text(format!("{}: {}", error.path, error.message))
            .style(iced::theme::Text::Color(crate::style::ERROR))
            .size((12.0 * scale).max(9.0)),
    ]
    .spacing((2.0 * scale).max(1.0))
    .padding(Padding::from([4.0, 16.0]))
    .into()
}

fn render_solution_node_column<'a>(
    nodes: &'a [SolutionTreeNode],
    indent: f32,
    scale: f32,
) -> Column<'a, Message> {
    let spacing = (3.0 * scale).max(1.0);
    let mut column = Column::new().spacing(spacing);

    for node in nodes {
        let icon = if node.is_directory { "📁" } else { "📄" };
        let row_content = row![
            horizontal_space().width(Length::Fixed(indent)),
            text(icon).size((12.0 * scale).max(9.0)),
            text(&node.name)
                .style(iced::theme::Text::Color(crate::style::TEXT))
                .size((13.0 * scale).max(9.0)),
        ]
        .spacing(spacing)
        .align_items(Alignment::Center);

        let element: Element<'_, Message> = if let Some(path) = &node.path {
            button(row_content)
                .style(document_button())
                .on_press(Message::WorkspaceFileActivated(path.clone()))
                .into()
        } else {
            row_content.into()
        };

        column = column.push(element);

        if !node.children.is_empty() {
            column = column.push(render_solution_node_column(&node.children, indent + 16.0, scale));
        }
    }

    column
}

fn render_open_files_panel(
    state: &EditorState,
    scale: f32,
    spacing_large: f32,
    spacing_medium: f32,
    sidebar_width: f32,
) -> Element<'_, Message> {
    let mut open_list = Column::new().spacing(4);
    for (index, document) in state.editor().open_documents().iter().enumerate() {
        let is_active = index == state.editor().active_index();
        let mut title = document.display_name().to_string();
        if document.is_modified {
            title.push('*');
        }

        let file_text = text(&title).style(iced::theme::Text::Color(crate::style::TEXT));

        let close_button = button(text("×").style(iced::theme::Text::Color(crate::style::MUTED)))
            .style(crate::style::custom_button())
            .on_press(Message::DocumentSelected(0)); // dummy

        let item = row![file_text, close_button]
            .spacing(4)
            .align_items(Alignment::Center);

        let button = button(item)
            .style(crate::style::document_button())
            .on_press(Message::DocumentSelected(index));

        open_list = open_list.push(button);
    }

    let open_scroll = scrollable(open_list).style(crate::style::custom_scrollable());

    let header = button(text("Recent Files").style(iced::theme::Text::Color(crate::style::TEXT)))
        .style(crate::style::custom_button())
        .on_press(Message::DocumentSelected(0)); // dummy

    let mut recent_list = Column::new().spacing(4);
    for path in state.workspace_recent_files() {
        let item = button(text(&path).style(iced::theme::Text::Color(crate::style::MUTED)))
            .style(crate::style::document_button())
            .on_press(Message::WorkspaceFileActivated(path.clone()));

        recent_list = recent_list.push(item);
    }

    let recent_scroll = scrollable(recent_list).style(crate::style::custom_scrollable());

    let content = column![
        text("Open Files").style(iced::theme::Text::Color(crate::style::TEXT)),
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





fn render_console_panel(
    state: &EditorState,
    scale: f32,
    spacing_large: f32,
    spacing_medium: f32,
    spacing_small: f32,
) -> Element<'_, Message> {
    let console = state.console();
    let tab_width = (160.0 * scale).max(120.0);

    let mut tab_column = column![text("Terminals").size((16.0 * scale).max(12.0))]
        .spacing(spacing_small)
        .width(Length::Fill);

    for tab in console.tabs() {
        let label = text(tab.title()).size((14.0 * scale).max(10.0));
        let tab_id = tab.id();
        let mut button = button(label)
            .width(Length::Fill)
            .padding((6.0 * scale).max(4.0));
        if Some(tab_id) == console.active_tab_id() {
            button = button.style(theme::Button::Primary);
        } else {
            button = button.style(theme::Button::Text);
        }
        tab_column = tab_column.push(button.on_press(Message::ConsoleTabSelected(tab_id)));
    }

    tab_column = tab_column.push(
        button(text("+ New").size((14.0 * scale).max(10.0)))
            .padding((6.0 * scale).max(4.0))
            .width(Length::Fill)
            .style(theme::Button::Secondary)
            .on_press(Message::ConsoleNewRequested),
    );

    let tabs_panel = container(tab_column)
        .padding(spacing_medium)
        .width(Length::Fixed(tab_width))
        .style(panel_container());

    let content_panel: Element<'_, Message> = if let Some(active) = console.active_tab() {
        let tab_id = active.id();

        let mut lines = column![]
            .spacing((2.0 * scale).max(1.0))
            .width(Length::Fill);

        for entry in active.lines() {
            let color = match entry.kind {
                ConsoleLineKind::Output => Color::from_rgb8(210, 210, 210),
                ConsoleLineKind::Error => Color::from_rgb8(255, 160, 160),
                ConsoleLineKind::Info => Color::from_rgb8(180, 220, 180),
                ConsoleLineKind::Command => Color::from_rgb8(156, 220, 254),
            };

            let text_value = if entry.text.is_empty() {
                " ".to_string()
            } else {
                entry.text.clone()
            };

            lines = lines.push(
                text(text_value)
                    .font(Font::MONOSPACE)
                    .size((13.0 * scale).max(9.0))
                    .style(color),
            );
        }

        let scroll_height = (220.0 * scale).max(160.0);
        let scroll = scrollable(lines)
            .height(Length::Fixed(scroll_height))
            .width(Length::Fill);

        let status_text = match active.status() {
            ConsoleStatus::Running => "Running".to_string(),
            ConsoleStatus::Exited(code) => format!("Exited ({})", code),
        };

        let mut content = column![
            row![
                text(active.title()).size((16.0 * scale).max(12.0)),
                horizontal_space().width(Length::Fill),
                text(status_text)
                    .size((13.0 * scale).max(9.0))
                    .style(Color::from_rgb8(180, 180, 180)),
            ]
            .align_items(Alignment::Center),
            scroll,
        ]
        .spacing(spacing_medium)
        .width(Length::Fill);

        if active.kind() == ConsoleKind::Shell {
            let input_field = text_input("Run command", active.input())
                .on_input(move |value| Message::ConsoleInputChanged(tab_id, value))
                .on_submit(Message::ConsoleInputSubmitted(tab_id))
                .padding((6.0 * scale).max(4.0))
                .size((14.0 * scale).max(10.0))
                .width(Length::Fill);

            let send_button = button(text("Send").size((14.0 * scale).max(10.0)))
                .padding((6.0 * scale).max(4.0))
                .style(theme::Button::Primary)
                .on_press(Message::ConsoleInputSubmitted(tab_id));

            content = content.push(
                row![input_field, send_button]
                    .spacing(spacing_small)
                    .align_items(Alignment::Center),
            );
        } else {
            content = content.push(
                text("Debug output (read-only)")
                    .size((13.0 * scale).max(9.0))
                    .style(Color::from_rgb8(180, 180, 180)),
            );
        }

        container(content)
            .padding(spacing_large)
            .width(Length::Fill)
            .style(panel_container())
            .into()
    } else {
        container(
            column![
                text("No console available")
                    .size((14.0 * scale).max(10.0))
                    .style(Color::from_rgb8(200, 200, 200)),
                button(text("Create terminal").size((14.0 * scale).max(10.0)))
                    .style(theme::Button::Primary)
                    .on_press(Message::ConsoleNewRequested),
            ]
            .spacing(spacing_medium)
            .width(Length::Fill)
            .align_items(Alignment::Start),
        )
        .padding(spacing_large)
        .width(Length::Fill)
        .style(panel_container())
        .into()
    };

    row![tabs_panel, content_panel]
        .spacing(spacing_large)
        .width(Length::Fill)
        .into()
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
    let drop_width = (600.0 * scale).clamp(400.0, 800.0);

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
        // Show a window of 6 items centered on the selection
        let window_size = 6;
        let half_window = window_size / 2;
        let start = selection.saturating_sub(half_window);
        let end = (start + window_size).min(filtered.len());
        let adjusted_start = if end - start < window_size && start > 0 {
            start.saturating_sub(window_size - (end - start))
        } else {
            start
        };

        for i in adjusted_start..end {
            if let Some(index) = filtered.get(i) {
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

                    if i == selection {
                        entry = entry.style(theme::Button::Primary);
                    } else {
                        entry = entry.style(theme::Button::Text);
                    }

                    command_list = command_list.push(entry);
                }
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
        scrollable(command_list)
            .height(Length::Fixed(240.0 * scale))
            .style(crate::style::custom_scrollable()),
    ]
    .spacing(spacing_medium)
    .width(Length::Fill);

    let dropdown = container(palette_column)
        .padding(spacing_large)
        .width(Length::Fixed(drop_width))
        .style(floating_panel_container());

    let overlay_background = container(dropdown)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x()
        .center_y()
        .style(overlay_container());

    overlay_background.into()
}
