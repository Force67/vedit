use iced::widget::{Row, button, column, container, row, scrollable, text, text_input};
use iced::{Alignment, Element, Length, Padding, theme};
use iced_font_awesome::fa_icon_solid;

use crate::message::{Message, RightRailTab};
use crate::state::EditorState;
use crate::style;

pub fn render_right_rail(
    state: &EditorState,
    scale: f32,
    sidebar_width: f32,
) -> Element<'_, Message> {
    // Create the tab bar with Font Awesome icons
    let tab_bar: Row<Message, theme::Theme, iced::Renderer> = Row::with_children(vec![
        {
            let mut btn = button(fa_icon_solid("folder").size(14.0).color(iced::Color::WHITE))
                .style(crate::style::custom_button())
                .on_press(Message::RightRailTabSelected(RightRailTab::Workspace));
            if state.selected_right_rail_tab() == RightRailTab::Workspace {
                btn = btn.style(crate::style::active_document_button());
            }
            btn.into()
        },
        {
            let mut btn = button(
                fa_icon_solid("lightbulb")
                    .size(14.0)
                    .color(crate::style::MUTED),
            )
            .style(crate::style::custom_button())
            .on_press(Message::RightRailTabSelected(RightRailTab::Solutions));
            if state.selected_right_rail_tab() == RightRailTab::Solutions {
                btn = btn.style(crate::style::active_document_button());
            }
            btn.into()
        },
        {
            let mut btn = button(fa_icon_solid("list").size(14.0).color(crate::style::MUTED))
                .style(crate::style::custom_button())
                .on_press(Message::RightRailTabSelected(RightRailTab::Outline));
            if state.selected_right_rail_tab() == RightRailTab::Outline {
                btn = btn.style(crate::style::active_document_button());
            }
            btn.into()
        },
        {
            let mut btn = button(
                fa_icon_solid("magnifying-glass")
                    .size(14.0)
                    .color(crate::style::MUTED),
            )
            .style(crate::style::custom_button())
            .on_press(Message::RightRailTabSelected(RightRailTab::Search));
            if state.selected_right_rail_tab() == RightRailTab::Search {
                btn = btn.style(crate::style::active_document_button());
            }
            btn.into()
        },
        {
            let mut btn = button(
                fa_icon_solid("triangle-exclamation")
                    .size(14.0)
                    .color(crate::style::MUTED),
            )
            .style(crate::style::custom_button())
            .on_press(Message::RightRailTabSelected(RightRailTab::Problems));
            if state.selected_right_rail_tab() == RightRailTab::Problems {
                btn = btn.style(crate::style::active_document_button());
            }
            btn.into()
        },
        {
            let mut btn = button(
                fa_icon_solid("note-sticky")
                    .size(14.0)
                    .color(crate::style::MUTED),
            )
            .style(crate::style::custom_button())
            .on_press(Message::RightRailTabSelected(RightRailTab::Notes));
            if state.selected_right_rail_tab() == RightRailTab::Notes {
                btn = btn.style(crate::style::active_document_button());
            }
            btn.into()
        },
        {
            let mut btn = button(
                fa_icon_solid("wine-glass")
                    .size(14.0)
                    .color(crate::style::MUTED),
            )
            .style(crate::style::custom_button())
            .on_press(Message::RightRailTabSelected(RightRailTab::Wine));
            if state.selected_right_rail_tab() == RightRailTab::Wine {
                btn = btn.style(crate::style::active_document_button());
            }
            btn.into()
        },
    ])
    .spacing(0);

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
        text("Notes").size((14.0 * scale).max(10.0)).color(style::TEXT),
        iced::widget::Space::new().width(Length::Fill),
        button(
            fa_icon_solid("plus")
                .size(12.0)
                .color(iced::Color::WHITE)
        )
        .style(style::custom_button())
        .on_press(Message::StickyNoteCreateRequested)
        .padding(4)
    ]
    .align_y(Alignment::Center)
    .padding(Padding::from([8.0, 8.0]));

    if notes.is_empty() {
        let empty_state = column![
            text("No notes yet").color(style::MUTED).size((12.0 * scale).max(10.0)),
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

            let delete_btn = button(
                fa_icon_solid("trash")
                    .size(10.0)
                    .color(style::ERROR)
            )
            .style(style::document_button())
            .on_press(Message::StickyNoteDeleted(note_id))
            .padding(2);

            let note_header = row![line_info, iced::widget::Space::new().width(Length::Fill), delete_btn]
                .align_y(Alignment::Center);

            let content_input = text_input("Add note content...", &note.content)
                .on_input(move |value| Message::StickyNoteContentChanged(note_id, value))
                .size((12.0 * scale).max(10.0))
                .padding(4);

            container(
                column![note_header, content_input].spacing(4)
            )
            .style(style::ribbon_container())
            .padding(8)
            .into()
        })
        .collect();

    let notes_list = scrollable(
        column(note_items).spacing(8).padding(Padding::from([0.0, 8.0]))
    )
    .style(style::custom_scrollable())
    .height(Length::Fill);

    column![header, notes_list].into()
}
