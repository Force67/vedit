use crate::message::Message;
use crate::state::EditorState;
use crate::style::{
    separator_container, title_bar_container, top_bar_button, window_close_button,
    window_control_button, PRIMARY, TEXT,
};
use iced::widget::{Space, button, container, mouse_area, row, text};
use iced::{Alignment, Element, Length};
use iced_font_awesome::fa_icon_solid;

/// Helper to create a visual separator between button groups
fn separator(scale: f32) -> Element<'static, Message> {
    container(Space::new())
        .width(Length::Fixed(1.0))
        .height(Length::Fixed((18.0 * scale).max(12.0)))
        .style(separator_container())
        .into()
}

pub fn render_title_bar(
    state: &EditorState,
    scale: f32,
    spacing_large: f32,
    spacing_medium: f32,
    spacing_small: f32,
) -> Element<'_, Message> {
    // App branding
    let branding = text("vedit")
        .size((18.0 * scale).max(14.0))
        .color(PRIMARY);

    // File actions group
    let file_actions = row![
        button(text("Open File…").size((13.0 * scale).max(10.0)))
            .style(top_bar_button())
            .on_press(Message::OpenFileRequested),
        button(text("Open Folder…").size((13.0 * scale).max(10.0)))
            .style(top_bar_button())
            .on_press(Message::WorkspaceOpenRequested),
        button(text("Open Solution…").size((13.0 * scale).max(10.0)))
            .style(top_bar_button())
            .on_press(Message::SolutionOpenRequested),
    ]
    .spacing(spacing_small);

    // Toggle buttons group
    let toggle_buttons = row![
        {
            let icon = if state.command_palette().is_open() {
                fa_icon_solid("angle-up").color(TEXT)
            } else {
                fa_icon_solid("angle-down").color(TEXT)
            };

            button(
                row![
                    text("Command").size((13.0 * scale).max(10.0)),
                    icon.size((12.0 * scale).max(10.0))
                ]
                .spacing(4.0),
            )
            .style(top_bar_button())
            .on_press(Message::CommandPromptToggled)
        },
        {
            let icon = if state.console().is_visible() {
                fa_icon_solid("angle-up").color(TEXT)
            } else {
                fa_icon_solid("angle-down").color(TEXT)
            };

            button(
                row![
                    text("Terminal").size((13.0 * scale).max(10.0)),
                    icon.size((12.0 * scale).max(10.0))
                ]
                .spacing(4.0),
            )
            .style(top_bar_button())
            .on_press(Message::ConsoleVisibilityToggled)
        },
    ]
    .spacing(spacing_small);

    // Debug controls group
    let debug_controls = row![
        button(
            row![
                fa_icon_solid("play")
                    .size((12.0 * scale).max(10.0))
                    .color(TEXT),
                text("Run").size((13.0 * scale).max(10.0))
            ]
            .spacing(4.0)
        )
        .style(top_bar_button())
        .on_press(Message::DebuggerLaunchRequested),
        button(
            row![
                fa_icon_solid("stop")
                    .size((12.0 * scale).max(10.0))
                    .color(TEXT),
                text("Stop").size((13.0 * scale).max(10.0))
            ]
            .spacing(4.0)
        )
        .style(top_bar_button())
        .on_press(Message::DebuggerStopRequested),
        {
            let summary = state.debugger().selection_summary();
            let icon = if state.debugger_menu_open() {
                fa_icon_solid("angle-up").color(TEXT)
            } else {
                fa_icon_solid("angle-down").color(TEXT)
            };
            button(
                row![
                    text(summary).size((13.0 * scale).max(10.0)),
                    icon.size((12.0 * scale).max(10.0))
                ]
                .spacing(4.0),
            )
            .style(top_bar_button())
            .on_press(Message::DebuggerMenuToggled)
        },
    ]
    .spacing(spacing_small);

    // Main row with separators between groups
    let main_row = row![
        branding,
        Space::new().width(Length::Fixed(spacing_large)),
        separator(scale),
        Space::new().width(Length::Fixed(spacing_medium)),
        file_actions,
        Space::new().width(Length::Fixed(spacing_medium)),
        separator(scale),
        Space::new().width(Length::Fixed(spacing_medium)),
        toggle_buttons,
        Space::new().width(Length::Fixed(spacing_medium)),
        separator(scale),
        Space::new().width(Length::Fixed(spacing_medium)),
        debug_controls,
        Space::new().width(Length::Fill),
    ]
    .align_y(Alignment::Center);

    // Settings button
    let message = if state.settings().is_open() {
        Message::SettingsClosed
    } else {
        Message::SettingsOpened
    };

    let settings_button = button(
        fa_icon_solid("gear")
            .size((14.0 * scale).max(12.0))
            .color(TEXT),
    )
    .style(top_bar_button())
    .on_press(message);

    // Window controls with specialized styles
    let window_buttons = row![
        button(
            fa_icon_solid("window-minimize")
                .size((12.0 * scale).max(10.0))
                .color(TEXT)
        )
        .style(window_control_button())
        .on_press(Message::WindowMinimize),
        button(
            fa_icon_solid("window-maximize")
                .size((12.0 * scale).max(10.0))
                .color(TEXT)
        )
        .style(window_control_button())
        .on_press(Message::WindowMaximize),
        button(
            fa_icon_solid("xmark")
                .size((12.0 * scale).max(10.0))
                .color(TEXT)
        )
        .style(window_close_button())
        .on_press(Message::WindowClose),
    ]
    .spacing(2.0);

    // Combine everything
    let full_row = row![
        main_row,
        settings_button,
        Space::new().width(Length::Fixed(spacing_medium)),
        separator(scale),
        Space::new().width(Length::Fixed(spacing_medium)),
        window_buttons,
    ]
    .align_y(Alignment::Center);

    let title_bar = mouse_area(full_row).on_press(Message::WindowDragStart);

    container(title_bar)
        .padding([spacing_medium, spacing_large])
        .width(Length::Fill)
        .style(title_bar_container())
        .into()
}
