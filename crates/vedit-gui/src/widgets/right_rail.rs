use iced::widget::{Row, button, column, container, row, scrollable, text, text_input};
use iced::{Alignment, Element, Length, Padding, theme};
use iced_font_awesome::fa_icon_solid;

use crate::message::{Message, RightRailTab};
use crate::state::EditorState;
use crate::style::{self, TEXT, TEXT_SECONDARY};

/// Helper to create a tab button with proper styling
fn tab_button(
    icon: &'static str,
    tab: RightRailTab,
    current_tab: RightRailTab,
) -> Element<'static, Message> {
    let is_active = current_tab == tab;
    let icon_color = if is_active { TEXT } else { TEXT_SECONDARY };
    let icon_widget = fa_icon_solid(icon).size(14.0).color(icon_color);

    if is_active {
        button(icon_widget)
            .style(style::active_document_button())
            .on_press(Message::RightRailTabSelected(tab))
            .padding(6)
            .into()
    } else {
        button(icon_widget)
            .style(style::document_button())
            .on_press(Message::RightRailTabSelected(tab))
            .padding(6)
            .into()
    }
}

pub fn render_right_rail(
    state: &EditorState,
    scale: f32,
    sidebar_width: f32,
) -> Element<'_, Message> {
    let current_tab = state.selected_right_rail_tab();

    // Create the tab bar with Font Awesome icons
    let tab_bar: Row<Message, theme::Theme, iced::Renderer> = Row::with_children(vec![
        tab_button("folder", RightRailTab::Workspace, current_tab),
        tab_button("lightbulb", RightRailTab::Solutions, current_tab),
        tab_button("list", RightRailTab::Outline, current_tab),
        tab_button("magnifying-glass", RightRailTab::Search, current_tab),
        tab_button("triangle-exclamation", RightRailTab::Problems, current_tab),
        tab_button("note-sticky", RightRailTab::Notes, current_tab),
        tab_button("wine-glass", RightRailTab::Wine, current_tab),
    ])
    .spacing(2)
    .padding(4);

    // Render the content for the selected tab
    let content: Element<Message> = match state.selected_right_rail_tab() {
        RightRailTab::Workspace => {
            if let Some(explorer) = state.file_explorer() {
                explorer.view().map(Message::FileExplorer)
            } else {
                scrollable(
                    column![
                        text("Open a folder to browse project files")
                            .size((14.0 * scale).max(10.0))
                    ]
                    .spacing(4)
                    .padding(Padding::from([8.0, 16.0])),
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
                    column![text("Open a folder to view solutions").color(crate::style::TEXT)]
                        .spacing(4)
                        .padding(8),
                )
                .style(crate::style::custom_scrollable())
                .into()
            }
        }
        RightRailTab::Wine => crate::widgets::wine_simple::render_wine_panel(),
        RightRailTab::Notes => render_notes_tab(state, scale),
        _ => scrollable(
            column![text("Not implemented yet").color(crate::style::TEXT)]
                .spacing(4)
                .padding(8),
        )
        .style(crate::style::custom_scrollable())
        .into(),
    };

    // Combine tab bar and content into the right rail panel
    container(column![tab_bar, content].spacing(0))
        .style(style::panel_container())
        .width(Length::Fixed(sidebar_width))
        .height(Length::Fill)
        .into()
}

fn render_notes_tab(state: &EditorState, scale: f32) -> Element<'static, Message> {
    let notes = state.active_sticky_notes();
    let has_workspace = state.editor().workspace_root().is_some();

    if !has_workspace {
        return scrollable(
            column![text("Open a workspace to use sticky notes").color(style::MUTED)]
                .spacing(4)
                .padding(8),
        )
        .style(style::custom_scrollable())
        .into();
    }

    let header = row![
        text("Notes")
            .size((14.0 * scale).max(10.0))
            .color(style::TEXT),
        iced::widget::Space::new().width(Length::Fill),
        button(fa_icon_solid("plus").size(12.0).color(iced::Color::WHITE))
            .style(style::custom_button())
            .on_press(Message::StickyNoteCreateRequested)
            .padding(4)
    ]
    .align_y(Alignment::Center)
    .padding(Padding::from([8.0, 8.0]));

    if notes.is_empty() {
        let empty_state = column![
            text("No notes yet")
                .color(style::MUTED)
                .size((12.0 * scale).max(10.0)),
            text("Click + to add a note at cursor")
                .color(style::MUTED)
                .size((11.0 * scale).max(9.0)),
        ]
        .spacing(4)
        .padding(Padding::from([16.0, 8.0]));

        return column![header, empty_state].into();
    }

    let note_items: Vec<Element<'static, Message>> = notes
        .into_iter()
        .map(|note| {
            let note_id = note.id;
            let line_info = text(format!("Line {}", note.line))
                .size((11.0 * scale).max(9.0))
                .color(style::MUTED);

            let delete_btn = button(fa_icon_solid("trash").size(10.0).color(style::ERROR))
                .style(style::document_button())
                .on_press(Message::StickyNoteDeleted(note_id))
                .padding(2);

            let note_header = row![
                line_info,
                iced::widget::Space::new().width(Length::Fill),
                delete_btn
            ]
            .align_y(Alignment::Center);

            let content_input = text_input("Add note content...", &note.content)
                .on_input(move |value| Message::StickyNoteContentChanged(note_id, value))
                .size((12.0 * scale).max(10.0))
                .padding(4);

            container(column![note_header, content_input].spacing(4))
                .style(style::ribbon_container())
                .padding(8)
                .into()
        })
        .collect();

    let notes_list = scrollable(
        column(note_items)
            .spacing(8)
            .padding(Padding::from([0.0, 8.0])),
    )
    .style(style::custom_scrollable())
    .height(Length::Fill);

    column![header, notes_list].into()
}
