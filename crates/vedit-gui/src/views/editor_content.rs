use crate::message::Message;
use crate::state::EditorState;
use crate::style::panel_container;
use crate::syntax::{format_highlight, SyntaxHighlighter};
use crate::widgets::text_editor::TextEditor as EditorWidget;
use crate::views::scrollbar_style::editor_scrollbar_style;
use crate::views::console_panel;
use iced::widget::{column, container, row, text, vertical_slider};
use iced::{Alignment, Color, Element, Font, Length, Pixels};

pub fn render_editor_content(
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
    .step(0.5_f32)  // Smaller steps for smoother scrolling
    .width(scrollbar_width)
    .height(Length::Fill)
    .style(editor_scrollbar_style());

    let font_size = Pixels((14.0 * state.code_font_zoom()) as f32);
    let buffer = EditorWidget::new(state.buffer_content())
        .font(Font::MONOSPACE)
        .font_size(font_size)
        .highlight::<SyntaxHighlighter>(state.syntax_settings(), format_highlight)
        .line_number_color(Color::from_rgb8(133, 133, 133))
        .search_highlight_line(state.get_search_highlight_line())
        .debug_dots(state.get_debug_dots().to_vec())
        .on_gutter_click(|line_number| Message::GutterClicked(line_number))
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
        .center_x(Length::Fill)
        .into();

    let buffer_content = row![
        buffer_panel,
        scrollbar_track,
    ]
    .spacing((6.0 * scale).max(3.0))
    .align_y(Alignment::Start)
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

    let open_panel = crate::views::open_files::render_open_files_panel(
        state,
        scale,
        spacing_large,
        spacing_medium,
        sidebar_width,
    );

    let workspace_panel = crate::widgets::right_rail::render_right_rail(
        state,
        scale,
        sidebar_width,
    );

    let content_row = row![open_panel, editor_panel, workspace_panel]
        .spacing(spacing_large)
        .width(Length::Fill)
        .height(Length::Fill);

    let mut layout = column![content_row]
        .spacing(spacing_large)
        .width(Length::Fill)
        .height(Length::Fill);

    if state.console().is_visible() {
        layout = layout.push(
            container(console_panel::render_console_panel(
                state,
                scale,
                spacing_large,
                spacing_medium,
                spacing_small,
            ))
            .width(Length::Fill)
            .height(Length::Fixed(300.0))
        );
    }

    layout.into()
}