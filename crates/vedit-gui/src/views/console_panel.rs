use crate::console::{ConsoleKind, ConsoleLineKind, ConsoleStatus};
use crate::message::Message;
use crate::state::EditorState;
use crate::style::panel_container;
use iced::widget::{button, column, container, horizontal_space, row, scrollable, text, text_input};
use iced::{theme, Alignment, Color, Element, Font, Length};

pub fn render_console_panel(
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
                    .style(theme::Button::Primary)
                    .on_press(Message::ConsoleInputSubmitted(tab_id));

                content = content.push(
                    row![input_field, send_button]
                        .spacing(spacing_small)
                        .align_items(Alignment::Center),
                );
            }
            ConsoleKind::Debug => {
                content = content.push(
                    text("Debug output (read-only)")
                        .size((13.0 * scale).max(9.0))
                        .style(Color::from_rgb8(180, 180, 180)),
                );
            }
            ConsoleKind::EditorLog => {
                content = content.push(
                    text("Editor internal debug log (read-only)")
                        .size((13.0 * scale).max(9.0))
                        .style(Color::from_rgb8(180, 180, 180)),
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