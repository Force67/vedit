use crate::message::Message;
use crate::state::EditorState;
use crate::style::{self, status_container};
use iced::widget::{Space, container, row, text};
use iced::{Alignment, Color, Element, Length, Padding, alignment::Vertical};
use iced_font_awesome::fa_icon_solid;

/// Separator element for status bar
fn separator(scale: f32) -> Element<'static, Message> {
    container(Space::new())
        .width(Length::Fixed(1.0))
        .height(Length::Fixed((12.0 * scale).max(8.0)))
        .style(style::status_separator())
        .into()
}

pub fn render_status_bar(
    state: &EditorState,
    scale: f32,
    spacing_small: f32,
    _spacing_large: f32,
) -> Element<'_, Message> {
    let text_size = (12.0 * scale).max(9.0);
    let icon_size = (10.0 * scale).max(8.0);
    let item_spacing = (8.0 * scale).max(4.0);
    let section_spacing = (12.0 * scale).max(6.0);

    // File info
    let file_status = state.editor().status_line();
    let file_item = row![
        fa_icon_solid("file").size(icon_size).color(style::MUTED),
        text(file_status)
            .size(text_size)
            .color(style::TEXT_SECONDARY),
    ]
    .spacing(4)
    .align_y(Alignment::Center);

    // Language
    let active_language = state
        .editor()
        .active_document()
        .map(|doc| doc.language().display_name())
        .unwrap_or("Plain Text");

    let lang_item = row![
        fa_icon_solid("code").size(icon_size).color(style::MUTED),
        text(active_language)
            .size(text_size)
            .color(style::TEXT_SECONDARY),
    ]
    .spacing(4)
    .align_y(Alignment::Center);

    // Character count
    let char_count = state
        .editor()
        .active_document()
        .map(|doc| doc.buffer.char_count())
        .unwrap_or(0);

    let chars_item = text(format!("{} chars", char_count))
        .size(text_size)
        .color(style::MUTED);

    // Workspace (shortened)
    let workspace_item = if let Some(name) = state.workspace_display_name() {
        row![
            fa_icon_solid("folder")
                .size(icon_size)
                .color(style::FOLDER_ICON),
            text(name).size(text_size).color(style::TEXT_SECONDARY),
        ]
        .spacing(4)
        .align_y(Alignment::Center)
    } else {
        row![text("No workspace").size(text_size).color(style::MUTED)].align_y(Alignment::Center)
    };

    // FPS with color coding
    let fps = state.fps_counter().fps();
    let fps_color = if fps >= 120.0 {
        style::SUCCESS
    } else if fps >= 90.0 {
        Color::from_rgb(0.5, 0.85, 0.5)
    } else if fps >= 60.0 {
        style::WARNING
    } else {
        style::ERROR
    };

    let fps_item = text(format!("{:.0} fps", fps))
        .size(text_size)
        .color(fps_color);

    // Error/Notice (if any)
    let notice_item: Element<'_, Message> = match (state.error(), state.workspace_notice()) {
        (Some(err), _) => row![
            fa_icon_solid("circle-exclamation")
                .size(icon_size)
                .color(style::ERROR),
            text(err).size(text_size).color(style::ERROR),
        ]
        .spacing(4)
        .align_y(Alignment::Center)
        .into(),
        (None, Some(notice)) => text(notice).size(text_size).color(style::PRIMARY).into(),
        _ => Space::new().width(0).into(),
    };

    // Build indicator (shown when building)
    let build_item: Element<'_, Message> = if state.is_building() {
        let build_name = state.build_target_name().unwrap_or("...");
        row![
            fa_icon_solid("gear").size(icon_size).color(style::WARNING),
            text(format!("Building: {}", build_name))
                .size(text_size)
                .color(style::WARNING),
        ]
        .spacing(4)
        .align_y(Alignment::Center)
        .into()
    } else {
        Space::new().width(0).into()
    };

    // Build status bar with separators
    let mut left_items: Vec<Element<'_, Message>> = vec![
        file_item.into(),
        separator(scale),
        lang_item.into(),
        separator(scale),
        chars_item.into(),
    ];

    // Add build indicator if building
    if state.is_building() {
        left_items.push(separator(scale));
        left_items.push(build_item);
    }

    let left_section = row(left_items)
        .spacing(section_spacing)
        .align_y(Alignment::Center);

    let right_section = row![workspace_item, separator(scale), fps_item, notice_item,]
        .spacing(section_spacing)
        .align_y(Alignment::Center);

    container(
        row![
            left_section,
            Space::new().width(Length::Fill),
            right_section,
        ]
        .spacing(item_spacing)
        .align_y(Alignment::Center),
    )
    .padding(Padding::from([spacing_small, (10.0 * scale).max(6.0)]))
    .width(Length::Fill)
    .align_y(Vertical::Center)
    .style(status_container())
    .into()
}
