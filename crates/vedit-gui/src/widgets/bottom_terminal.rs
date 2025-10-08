use iced::widget::{button, column, container, pick_list, row, scrollable, text, Column, Row};
use iced::{Alignment, Element, Length};
use iced_font_awesome::fa_icon_solid;

use crate::style;

#[derive(Debug, Clone)]
pub enum Message {
    ToggleCollapse,
    SelectTerminal(String),
    LogReceived(String, LogLevel),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Warning,
    Error,
}

impl LogLevel {
    pub fn color(&self) -> iced::Color {
        match self {
            LogLevel::Info => style::TEXT,
            LogLevel::Warning => style::WARNING,
            LogLevel::Error => style::ERROR,
        }
    }
}

pub struct BottomTerminal {
    pub collapsed: bool,
    pub selected_terminal: String,
    pub logs: Vec<(String, LogLevel)>,
}

impl BottomTerminal {
    pub fn new() -> Self {
        Self {
            collapsed: false,
            selected_terminal: "Terminal".to_string(),
            logs: vec![
                ("Application started".to_string(), LogLevel::Info),
                ("Warning: deprecated function".to_string(), LogLevel::Warning),
                ("Error: file not found".to_string(), LogLevel::Error),
            ],
        }
    }

    pub fn view(&self) -> Element<Message> {
        if self.collapsed {
            let header = button(row![
                fa_icon_solid("angle-right").color(iced::Color::WHITE),
                text("Terminal").style(iced::theme::Text::Color(style::TEXT))
            ].spacing(4.0))
                .style(style::custom_button())
                .on_press(Message::ToggleCollapse);

            container(header)
                .style(style::status_container())
                .width(Length::Fill)
                .height(32)
                .into()
        } else {
            let header = Row::new()
                .spacing(8)
                .push(
                    pick_list(
                        vec!["Terminal".to_string(), "Debug".to_string(), "Output".to_string()],
                        Some(self.selected_terminal.clone()),
                        Message::SelectTerminal,
                    )
                )
                .push(
                    button(fa_icon_solid("angle-down").color(iced::Color::WHITE))
                        .style(style::custom_button())
                        .on_press(Message::ToggleCollapse)
                )
                .align_items(Alignment::Center);

            let log_view = scrollable(
                Column::new()
                    .spacing(4)
                    .extend(self.logs.iter().map(|(msg, level)| {
                        text(msg).style(iced::theme::Text::Color(level.color())).into()
                    }))
                    .padding(8)
            )
            .style(style::custom_scrollable());

            let content = Column::new()
                .push(header)
                .push(log_view);

            container(content)
                .style(style::status_container())
                .width(Length::Fill)
                .height(200)
                .into()
        }
    }
}