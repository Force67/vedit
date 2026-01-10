use iced::widget::{Row, button, column, container, scrollable, text};
use iced::{Element, Length, Padding, theme};
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
