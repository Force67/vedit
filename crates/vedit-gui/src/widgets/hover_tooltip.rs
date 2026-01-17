//! Hover tooltip for type definitions
//!
//! This widget displays a floating tooltip showing a preview of a type definition
//! when hovering over a type identifier in the editor.

use crate::message::Message;
use crate::style;
use iced::widget::{Space, button, column, container, row, text};
use iced::{Background, Border, Color, Element, Length, Padding, Shadow, Theme, Vector};
use vedit_symbols::DefinitionLocation;

/// Render a hover tooltip showing type definition preview
pub fn render_hover_tooltip<'a>(
    location: &'a DefinitionLocation,
    symbol_name: &'a str,
    x: f32,
    y: f32,
    scale: f32,
    window_size: iced::Size,
) -> Element<'a, Message> {
    let tooltip_width = 450.0 * scale;
    let tooltip_max_height = 250.0 * scale;
    let padding = (10.0 * scale) as u16;
    let text_size = 13.0 * scale;
    let header_size = 14.0 * scale;
    let code_size = 12.0 * scale;

    // Header with symbol name and kind
    let kind_text = location.kind.as_str();
    let header = row![
        text(symbol_name).size(header_size).color(style::PRIMARY),
        Space::new().width(8),
        text(kind_text).size(text_size * 0.9).color(style::MUTED),
    ]
    .spacing(4)
    .align_y(iced::Alignment::Center);

    // File path and line number
    let file_name = location
        .file_path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let line_text = format!(":{}", location.line);

    let file_info = row![
        text(file_name)
            .size(text_size * 0.85)
            .color(style::TEXT_SECONDARY),
        text(line_text).size(text_size * 0.85).color(style::MUTED),
    ]
    .spacing(0);

    // Separator line
    let separator = container(Space::new().width(Length::Fill).height(Length::Fixed(1.0)))
        .style(|_theme: &Theme| iced::widget::container::Style {
            background: Some(Background::Color(style::BORDER_SUBTLE)),
            ..Default::default()
        })
        .padding(Padding::from([4, 0]));

    // Code preview with syntax-like coloring (simplified)
    let preview_lines: Vec<String> = location
        .preview
        .lines()
        .take(8)
        .map(|s| s.to_string())
        .collect();
    let code_preview = column(
        preview_lines
            .into_iter()
            .map(|line| {
                text(line)
                    .size(code_size)
                    .font(iced::Font::MONOSPACE)
                    .color(style::TEXT)
                    .into()
            })
            .collect::<Vec<_>>(),
    )
    .spacing(2);

    // Code container with background
    let code_container = container(code_preview)
        .padding(8)
        .width(Length::Fill)
        .style(|_theme: &Theme| iced::widget::container::Style {
            background: Some(Background::Color(style::BG)),
            border: Border {
                radius: 4.0.into(),
                width: 1.0,
                color: style::BORDER_SUBTLE,
            },
            ..Default::default()
        });

    // "Go to Definition" button
    let file_path = location.file_path.clone();
    let line = location.line;
    let col = location.column;

    let goto_button = button(
        text("Go to Definition (F12)")
            .size(text_size * 0.9)
            .color(style::PRIMARY),
    )
    .padding(Padding::from([4, 8]))
    .style(|_theme: &Theme, status| {
        let mut base = iced::widget::button::Style::default();
        base.background = Some(Background::Color(Color::TRANSPARENT));
        base.text_color = style::PRIMARY;
        base.border = Border::default();
        base.shadow = Shadow::default();

        match status {
            iced::widget::button::Status::Hovered => {
                base.background = Some(Background::Color(style::SURFACE_HOVER));
                base
            }
            iced::widget::button::Status::Pressed => {
                base.background = Some(Background::Color(style::SURFACE2));
                base
            }
            _ => base,
        }
    })
    .on_press(Message::HoverGotoDefinition(file_path, line, col));

    // Main content layout
    let content = column![header, file_info, separator, code_container, goto_button,]
        .spacing(6)
        .width(Length::Fixed(tooltip_width));

    // Tooltip container with shadow
    let tooltip_container = container(content).padding(padding).style(|_theme: &Theme| {
        iced::widget::container::Style {
            background: Some(Background::Color(style::SURFACE)),
            border: Border {
                radius: 6.0.into(),
                width: 1.0,
                color: style::BORDER,
            },
            shadow: Shadow {
                offset: Vector::new(0.0, 4.0),
                blur_radius: 12.0,
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.3),
            },
            ..Default::default()
        }
    });

    // Position tooltip avoiding screen edges
    // Position below and to the right of cursor by default
    // Y coordinate already points to bottom of hovered line, so minimal offset needed
    let cursor_offset_y = 2.0; // Just a tiny gap
    let cursor_offset_x = 0.0;

    let mut tooltip_x = x + cursor_offset_x;
    let mut tooltip_y = y + cursor_offset_y;

    // Clamp to window bounds
    if tooltip_x + tooltip_width > window_size.width - 10.0 {
        tooltip_x = (window_size.width - tooltip_width - 10.0).max(10.0);
    }
    if tooltip_y + tooltip_max_height > window_size.height - 10.0 {
        // Show above cursor instead
        tooltip_y = (y - tooltip_max_height - 10.0).max(10.0);
    }
    tooltip_x = tooltip_x.max(10.0);
    tooltip_y = tooltip_y.max(10.0);

    // Wrap in a full-size container with absolute positioning via padding
    container(tooltip_container)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(Padding {
            top: tooltip_y,
            left: tooltip_x,
            right: 0.0,
            bottom: 0.0,
        })
        .into()
}

/// Render a simple "loading" tooltip while fetching definition
pub fn render_loading_tooltip<'a>(
    symbol_name: &'a str,
    x: f32,
    y: f32,
    scale: f32,
    window_size: iced::Size,
) -> Element<'a, Message> {
    let tooltip_width = 200.0 * scale;
    let padding = (10.0 * scale) as u16;
    let text_size = 13.0 * scale;

    let content = column![
        text(symbol_name).size(text_size).color(style::PRIMARY),
        text("Looking up definition...")
            .size(text_size * 0.85)
            .color(style::MUTED),
    ]
    .spacing(4)
    .width(Length::Fixed(tooltip_width));

    let tooltip_container = container(content).padding(padding).style(|_theme: &Theme| {
        iced::widget::container::Style {
            background: Some(Background::Color(style::SURFACE)),
            border: Border {
                radius: 6.0.into(),
                width: 1.0,
                color: style::BORDER,
            },
            shadow: Shadow {
                offset: Vector::new(0.0, 4.0),
                blur_radius: 12.0,
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.3),
            },
            ..Default::default()
        }
    });

    // Position calculation
    let cursor_offset_y = 20.0 * scale;
    let mut tooltip_x = x;
    let mut tooltip_y = y + cursor_offset_y;

    if tooltip_x + tooltip_width > window_size.width - 10.0 {
        tooltip_x = (window_size.width - tooltip_width - 10.0).max(10.0);
    }
    tooltip_x = tooltip_x.max(10.0);
    tooltip_y = tooltip_y.max(10.0);

    container(tooltip_container)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(Padding {
            top: tooltip_y,
            left: tooltip_x,
            right: 0.0,
            bottom: 0.0,
        })
        .into()
}
