use crate::console::{ConsoleKind, ConsoleLineKind, ConsoleStatus};
use crate::message::Message;
use crate::state::EditorState;
use crate::style::{self, panel_container};
use iced::widget::{Space, button, column, container, row, scrollable, text, text_input};
use iced::{Alignment, Color, Element, Font, Length, Padding};
use iced_font_awesome::fa_icon_solid;

pub fn render_console_panel(
    state: &EditorState,
    scale: f32,
    spacing_large: f32,
    spacing_medium: f32,
    spacing_small: f32,
) -> Element<'_, Message> {
    let console = state.console();
    let text_size = (12.0 * scale).max(10.0);
    let icon_size = (11.0 * scale).max(9.0);

    // Horizontal tab bar instead of vertical
    let mut tab_row = row![].spacing(0).align_y(Alignment::Center);

    for tab in console.tabs() {
        let tab_id = tab.id();
        let is_active = Some(tab_id) == console.active_tab_id();

        // Icon based on console type
        let icon = match tab.kind() {
            ConsoleKind::Shell => "terminal",
            ConsoleKind::Debug => "bug",
            ConsoleKind::EditorLog => "file-lines",
            ConsoleKind::Build => "hammer",
        };

        let tab_content = row![
            fa_icon_solid(icon).size(icon_size).color(if is_active {
                style::TEXT
            } else {
                style::MUTED
            }),
            text(tab.title()).size(text_size).color(if is_active {
                style::TEXT
            } else {
                style::TEXT_SECONDARY
            }),
        ]
        .spacing(6)
        .align_y(Alignment::Center);

        let tab_button = button(tab_content)
            .style(style::console_tab(is_active))
            .padding(Padding::from([6, 12]))
            .on_press(Message::ConsoleTabSelected(tab_id));

        tab_row = tab_row.push(tab_button);
    }

    // New tab button
    let new_tab_btn = button(
        row![
            fa_icon_solid("plus").size(icon_size).color(style::MUTED),
            text("New").size(text_size).color(style::TEXT_SECONDARY),
        ]
        .spacing(4)
        .align_y(Alignment::Center),
    )
    .style(style::chevron_button())
    .padding(Padding::from([6, 10]))
    .on_press(Message::ConsoleNewRequested);

    tab_row = tab_row.push(new_tab_btn);
    tab_row = tab_row.push(Space::new().width(Length::Fill));

    let tab_bar = container(tab_row)
        .padding(Padding::from([0, spacing_small as u16]))
        .width(Length::Fill)
        .style(style::tab_bar_container());

    let content_panel: Element<'_, Message> = if let Some(active) = console.active_tab() {
        let tab_id = active.id();

        let mut lines = column![]
            .spacing((2.0 * scale).max(1.0))
            .width(Length::Fill);

        for entry in active.lines() {
            let color = match entry.kind {
                ConsoleLineKind::Output => style::TEXT_SECONDARY,
                ConsoleLineKind::Error => style::ERROR,
                ConsoleLineKind::Info => style::SUCCESS,
                ConsoleLineKind::Command => style::PRIMARY,
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
                    .color(color),
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
                Space::new().width(Length::Fill).width(Length::Fill),
                text(status_text)
                    .size((13.0 * scale).max(9.0))
                    .color(Color::from_rgb8(180, 180, 180)),
            ]
            .align_y(Alignment::Center),
            scroll,
        ]
        .spacing(spacing_medium)
        .width(Length::Fill);

        match active.kind() {
            ConsoleKind::Shell => {
                let input_field = text_input("Run command", active.input())
                    .on_input(move |value| Message::ConsoleInputChanged(tab_id, value))
                    .on_submit(Message::ConsoleInputSubmitted(tab_id))
                    .padding((6.0 * scale).max(4.0))
                    .size((14.0 * scale).max(10.0))
                    .width(Length::Fill);

                let send_button = button(text("Send").size((14.0 * scale).max(10.0)))
                    .padding((6.0 * scale).max(4.0))
                    .style(iced::widget::button::primary)
                    .on_press(Message::ConsoleInputSubmitted(tab_id));

                content = content.push(
                    row![input_field, send_button]
                        .spacing(spacing_small)
                        .align_y(Alignment::Center),
                );
            }
            ConsoleKind::Debug => {
                content = content.push(
                    text("Debug output (read-only)")
                        .size((13.0 * scale).max(9.0))
                        .color(Color::from_rgb8(180, 180, 180)),
                );
            }
            ConsoleKind::EditorLog => {
                content = content.push(
                    text("Editor internal debug log (read-only)")
                        .size((13.0 * scale).max(9.0))
                        .color(Color::from_rgb8(180, 180, 180)),
                );
            }
            ConsoleKind::Build => {
                content = content.push(
                    text("Build output (read-only)")
                        .size((13.0 * scale).max(9.0))
                        .color(Color::from_rgb8(180, 180, 180)),
                );
            }
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
                    .color(Color::from_rgb8(200, 200, 200)),
                button(text("Create terminal").size((14.0 * scale).max(10.0)))
                    .style(iced::widget::button::primary)
                    .on_press(Message::ConsoleNewRequested),
            ]
            .spacing(spacing_medium)
            .width(Length::Fill)
            .align_x(Alignment::Start),
        )
        .padding(spacing_large)
        .width(Length::Fill)
        .style(panel_container())
        .into()
    };

    // Vertical layout: tab bar on top, content below
    column![tab_bar, content_panel]
        .spacing(0)
        .width(Length::Fill)
        .into()
}
