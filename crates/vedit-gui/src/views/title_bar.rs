use crate::message::Message;
use crate::state::EditorState;
use crate::style::{ribbon_container, top_bar_button};
use iced::widget::{button, container, horizontal_space, mouse_area, row, text};
use iced::{theme, Alignment, Color, Element, Length};
use iced_font_awesome::fa_icon_solid;

pub fn render_title_bar(
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
            let icon = if state.command_palette().is_open() {
                fa_icon_solid("angle-up").color(iced::Color::WHITE)
            } else {
                fa_icon_solid("angle-down").color(iced::Color::WHITE)
            };

            button(row![
                text("Command Prompt").size((14.0 * scale).max(10.0)),
                icon.size((14.0 * scale).max(10.0))
            ].spacing(4.0))
                .style(top_bar_button())
                .on_press(Message::CommandPromptToggled)
        },
        {
            let icon = if state.console().is_visible() {
                fa_icon_solid("angle-up").color(iced::Color::WHITE)
            } else {
                fa_icon_solid("angle-down").color(iced::Color::WHITE)
            };

            button(row![
                text("Terminal").size((14.0 * scale).max(10.0)),
                icon.size((14.0 * scale).max(10.0))
            ].spacing(4.0))
                .style(top_bar_button())
                .on_press(Message::ConsoleVisibilityToggled)
        },
        button(row![
            fa_icon_solid("play").size((14.0 * scale).max(10.0)).color(iced::Color::WHITE),
            text("Run").size((14.0 * scale).max(10.0))
        ].spacing(4.0))
            .style(top_bar_button())
            .on_press(Message::DebuggerLaunchRequested),
        button(row![
            fa_icon_solid("stop").size((14.0 * scale).max(10.0)).color(iced::Color::WHITE),
            text("Stop").size((14.0 * scale).max(10.0))
        ].spacing(4.0))
            .style(top_bar_button())
            .on_press(Message::DebuggerStopRequested),
        {
            let summary = state.debugger().selection_summary();
            let icon = if state.debugger_menu_open() {
                fa_icon_solid("angle-up").color(iced::Color::WHITE)
            } else {
                fa_icon_solid("angle-down").color(iced::Color::WHITE)
            };
            button(row![
                text(summary).size((14.0 * scale).max(10.0)),
                icon.size((14.0 * scale).max(10.0))
            ].spacing(4.0))
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

    let settings_button = button(fa_icon_solid("gear").size((16.0 * scale).max(12.0)).color(iced::Color::WHITE))
        .style(top_bar_button())
        .on_press(message);

    let window_buttons = row![
        button(fa_icon_solid("window-minimize").size((14.0 * scale).max(10.0)).color(iced::Color::WHITE))
            .style(top_bar_button())
            .on_press(Message::WindowMinimize),
        button(fa_icon_solid("window-maximize").size((14.0 * scale).max(10.0)).color(iced::Color::WHITE))
            .style(top_bar_button())
            .on_press(Message::WindowMaximize),
        button(fa_icon_solid("xmark").size((14.0 * scale).max(10.0)).color(iced::Color::WHITE))
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