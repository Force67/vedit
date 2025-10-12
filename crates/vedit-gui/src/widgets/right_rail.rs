use iced::widget::{button, column, container, row, scrollable, text, Column, Row};
use iced::{Element, Length};
use std::path::PathBuf;

use crate::widgets::file_explorer;

use crate::style;
use crate::message::{Message, RightRailTab};

pub struct RightRail {
    pub workspace_root: PathBuf,
}

impl RightRail {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self {
            workspace_root,
        }
    }

    pub fn view(&self, active_tab: RightRailTab, scale: f32) -> Element<Message> {
        let tab_bar = Row::new()
            .spacing(0)
            .push(button(text("📁 Workspace").size(14))
                .style(if active_tab == RightRailTab::Workspace { style::active() } else { style::secondary() })
                .on_press(Message::RightRailTabSelected(RightRailTab::Workspace)))
            .push(button(text("📝 Solutions").size(14))
                .style(if active_tab == RightRailTab::Solutions { style::active() } else { style::secondary() })
                .on_press(Message::RightRailTabSelected(RightRailTab::Solutions)))
            .push(button(text("📄 Outline").size(14))
                .style(if active_tab == RightRailTab::Outline { style::active() } else { style::secondary() })
                .on_press(Message::RightRailTabSelected(RightRailTab::Outline)))
            .push(button(text("🔍 Search").size(14))
                .style(if active_tab == RightRailTab::Search { style::active() } else { style::secondary() })
                .on_press(Message::RightRailTabSelected(RightRailTab::Search)))
            .push(button(text("⚠️ Problems").size(14))
                .style(if active_tab == RightRailTab::Problems { style::active() } else { style::secondary() })
                .on_press(Message::RightRailTabSelected(RightRailTab::Problems)))
            .push(button(text("📝 Notes").size(14))
                .style(if active_tab == RightRailTab::Notes { style::active() } else { style::secondary() })
                .on_press(Message::RightRailTabSelected(RightRailTab::Notes)))
            .push(button(text("🍷 Wine").size(14))
                .style(if active_tab == RightRailTab::Wine { style::active() } else { style::secondary() })
                .on_press(Message::RightRailTabSelected(RightRailTab::Wine)));

        let content = match active_tab {
            RightRailTab::Workspace => self.workspace_tab(scale),
            RightRailTab::Solutions => self.solutions_tab(scale),
            RightRailTab::Outline => self.outline_tab(scale),
            RightRailTab::Search => self.search_tab(scale),
            RightRailTab::Problems => self.problems_tab(scale),
            RightRailTab::Notes => self.notes_tab(scale),
            RightRailTab::Wine => crate::widgets::wine_simple::render_wine_panel(),
        };

        let rail = Column::new()
            .push(tab_bar)
            .push(content);

        container(rail)
            .style(style::panel_container())
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn workspace_tab(&self, _scale: f32) -> Element<Message> {
        // This is now handled in view.rs
        text("Workspace").into()
    }

    fn solutions_tab(&self, _scale: f32) -> Element<Message> {
        text("Solutions placeholder").into()
    }

    fn outline_tab(&self, _scale: f32) -> Element<Message> {
        scrollable(
            column![
                text("Outline").style(iced::theme::Text::Color(style::TEXT)),
                text("fn main()").style(iced::theme::Text::Color(style::MUTED)),
                text("fn helper()").style(iced::theme::Text::Color(style::MUTED)),
            ]
            .spacing(4)
            .padding(8)
        )
        .style(style::custom_scrollable())
        .into()
    }

    fn search_tab(&self, _scale: f32) -> Element<Message> {
        scrollable(
            column![
                text("Search Results").style(iced::theme::Text::Color(style::TEXT)),
                text("No results").style(iced::theme::Text::Color(style::MUTED)),
            ]
            .spacing(4)
            .padding(8)
        )
        .style(style::custom_scrollable())
        .into()
    }

    fn problems_tab(&self, _scale: f32) -> Element<Message> {
        scrollable(
            column![
                text("Problems").style(iced::theme::Text::Color(style::TEXT)),
                text("No problems").style(iced::theme::Text::Color(style::MUTED)),
            ]
            .spacing(4)
            .padding(8)
        )
        .style(style::custom_scrollable())
        .into()
    }

    fn notes_tab(&self, _scale: f32) -> Element<Message> {
        scrollable(
            column![
                text("Notes").style(iced::theme::Text::Color(style::TEXT)),
                text("No notes").style(iced::theme::Text::Color(style::MUTED)),
            ]
            .spacing(4)
            .padding(8)
        )
        .style(style::custom_scrollable())
        .into()
    }
}