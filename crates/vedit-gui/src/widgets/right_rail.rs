use iced::widget::{button, column, container, row, scrollable, text, Column, Row};
use iced::{Element, Length};
use std::path::PathBuf;

use crate::widgets::file_explorer;

use crate::style;

#[derive(Debug, Clone)]
pub enum Message {
    TabSelected(usize),
    FileExplorer(file_explorer::Message),
}

pub struct RightRail {
    pub active_tab: usize,
    pub workspace_root: PathBuf,
}

impl RightRail {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self {
            active_tab: 0,
            workspace_root,
        }
    }

    pub fn view(&self) -> Element<Message> {
        let tab_bar = Row::new()
            .spacing(0)
            .push(button(text("Workspace").style(iced::theme::Text::Color(if self.active_tab == 0 { style::TEXT } else { style::MUTED })).size(14)).style(style::custom_button()).on_press(Message::TabSelected(0)))
            .push(button(text("Outline").style(iced::theme::Text::Color(if self.active_tab == 1 { style::TEXT } else { style::MUTED })).size(14)).style(style::custom_button()).on_press(Message::TabSelected(1)))
            .push(button(text("Search").style(iced::theme::Text::Color(if self.active_tab == 2 { style::TEXT } else { style::MUTED })).size(14)).style(style::custom_button()).on_press(Message::TabSelected(2)))
            .push(button(text("Problems").style(iced::theme::Text::Color(if self.active_tab == 3 { style::TEXT } else { style::MUTED })).size(14)).style(style::custom_button()).on_press(Message::TabSelected(3)))
            .push(button(text("Notes").style(iced::theme::Text::Color(if self.active_tab == 4 { style::TEXT } else { style::MUTED })).size(14)).style(style::custom_button()).on_press(Message::TabSelected(4)));

        let content = match self.active_tab {
            0 => self.workspace_tab(),
            1 => self.outline_tab(),
            2 => self.search_tab(),
            3 => self.problems_tab(),
            4 => self.notes_tab(),
            _ => self.workspace_tab(),
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

    fn workspace_tab(&self) -> Element<Message> {
        // This is now handled in view.rs
        text("Workspace").into()
    }

    fn outline_tab(&self) -> Element<Message> {
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

    fn search_tab(&self) -> Element<Message> {
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

    fn problems_tab(&self) -> Element<Message> {
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

    fn notes_tab(&self) -> Element<Message> {
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