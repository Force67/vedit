use crate::message::Message;
use crate::state::EditorState;
use crate::style::{self, panel_container};
use crate::syntax::{SyntaxHighlighter, format_highlight};
use crate::views::console_panel;
use crate::views::document_tabs::render_document_tabs;
use crate::views::scrollbar_style::editor_scrollbar_style;
use crate::widgets::text_editor::TextEditor as EditorWidget;
use iced::widget::{column, container, row, vertical_slider};
use iced::{Element, Font, Length, Pixels};

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
    let scrollbar_width = (6.0 * scale).clamp(4.0, 8.0); // Thinner scrollbar
    let slider_position = (max_scroll - scroll_value).clamp(0.0, max_scroll);
    let scrollbar = vertical_slider::VerticalSlider::<f32, Message>::new(
        0.0..=max_scroll,
        slider_position,
        move |value| Message::BufferScrollChanged(max_scroll - value),
    )
    .step(0.5_f32)
    .width(scrollbar_width)
    .height(Length::Fill)
    .style(editor_scrollbar_style());

    let font_size = Pixels((14.0 * state.code_font_zoom()) as f32);
    let buffer = EditorWidget::new(state.buffer_content())
        .font(Font::MONOSPACE)
        .font_size(font_size)
        .highlight::<SyntaxHighlighter>(state.syntax_settings(), format_highlight)
        .line_number_color(style::GUTTER_LINE_NUMBER)
        .search_highlight_line(state.get_search_highlight_line())
        .debug_dots(state.get_debug_dots().to_vec())
        .on_gutter_click(|line_number| Message::GutterClicked(line_number))
        .padding(editor_padding)
        .on_action(Message::BufferAction)
        .height(Length::Fill);

    // Put buffer and scrollbar together in a row, then wrap in styled container
    let buffer_with_scrollbar = row![buffer, scrollbar]
        .spacing(2.0)
        .width(Length::Fill)
        .height(Length::Fill);

    let editor_panel = container(buffer_with_scrollbar)
        .padding((4.0 * scale).max(2.0))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(panel_container());

    let sidebar_width = (200.0 / state.scale_factor()).clamp(140.0, 240.0) as f32;

    // Conditionally render open files panel based on tab location setting
    let show_sidebar_tabs = !state.tabs_at_top();

    let workspace_panel =
        crate::widgets::right_rail::render_right_rail(state, scale, sidebar_width);

    // Build main content area
    let main_content = if show_sidebar_tabs {
        // Sidebar mode: show open files panel on left
        let open_panel = crate::views::open_files::render_open_files_panel(
            state,
            scale,
            spacing_large,
            spacing_medium,
            sidebar_width,
        );

        row![open_panel, editor_panel, workspace_panel]
            .spacing(spacing_small)
            .width(Length::Fill)
            .height(Length::Fill)
    } else {
        // Top tabs mode: no open files panel, just editor and workspace
        row![editor_panel, workspace_panel]
            .spacing(spacing_small)
            .width(Length::Fill)
            .height(Length::Fill)
    };

    // Build layout with optional tab bar at top
    let mut layout = if state.tabs_at_top() {
        // Tab bar at top
        let tab_bar = render_document_tabs(state, scale);
        column![tab_bar, main_content]
            .spacing(0)
            .width(Length::Fill)
            .height(Length::Fill)
    } else {
        column![main_content]
            .spacing(spacing_large)
            .width(Length::Fill)
            .height(Length::Fill)
    };

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
            .height(Length::Fixed(300.0)),
        );
    }

    layout.into()
}
