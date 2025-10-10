use crate::message::{Message, RightRailTab};
use crate::state::EditorState;
use crate::style::{panel_container, active_document_button, document_button};
use crate::syntax::{format_highlight, SyntaxHighlighter};
use crate::widgets::text_editor::TextEditor as EditorWidget;
use crate::views::scrollbar_style::EditorScrollbarStyle;
use iced::widget::{button, column, container, horizontal_space, pick_list, row, scrollable, text, text_input, vertical_slider, Row};
use iced::{theme, Alignment, Color, Element, Font, Length, Padding, Pixels};
use iced::widget::slider;

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

    let open_panel = crate::views::open_files::render_open_files_panel(
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
                scrollable(crate::views::solutions::render_solutions_tab(state, scale))
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
                .style(crate::style::status_container())
                .width(Length::Fill)
                .height(Length::Fixed(200.0))
        );
    }

    layout.into()
}