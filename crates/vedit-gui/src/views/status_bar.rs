use crate::message::Message;
use crate::state::EditorState;
use crate::style::status_container;
use iced::widget::{container, row, text};
use iced::{Alignment, Color, Element, Length, alignment::Vertical};

pub fn render_status_bar(
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
            text(format!("FPS: {:.0}", state.fps_counter().fps()))
                .size((18.0 * scale).max(12.0)) // Bigger font for FPS
                .color(if state.fps_counter().fps() >= 120.0 {
                    Color::from_rgb(0.0, 1.0, 0.0) // Green for 144Hz+ target
                } else if state.fps_counter().fps() >= 90.0 {
                    Color::from_rgb(0.2, 0.8, 0.2) // Light green for good FPS
                } else if state.fps_counter().fps() >= 60.0 {
                    Color::from_rgb(1.0, 1.0, 0.0) // Yellow for acceptable FPS
                } else {
                    Color::from_rgb(1.0, 0.0, 0.0) // Red for low FPS
                }),
            match (state.error(), state.workspace_notice()) {
                (Some(err), _) => text(format!("Error: {}", err)).size((14.0 * scale).max(10.0)),
                (None, Some(notice)) => text(notice)
                    .size((14.0 * scale).max(10.0))
                    .color(Color::from_rgb8(38, 139, 210)),
                _ => text("").size(14),
            },
        ]
        .spacing((24.0 * scale).max(12.0))
        .align_y(Alignment::Center),
    )
    .padding([spacing_small, spacing_large])
    .width(Length::Fill)
    .align_y(Vertical::Center)
    .style(status_container())
    .into()
}
