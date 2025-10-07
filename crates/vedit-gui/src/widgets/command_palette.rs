use iced::widget::{column, container, scrollable, text, text_input, Column};
use iced::{Element, Length};

use crate::style;

#[derive(Debug, Clone)]
pub enum Message {
    InputChanged(String),
    ExecuteCommand(String),
    Close,
}

pub struct CommandPalette {
    pub query: String,
    pub suggestions: Vec<String>,
}

impl CommandPalette {
    pub fn new() -> Self {
        Self {
            query: "".to_string(),
            suggestions: vec![
                "Open File".to_string(),
                "Save File".to_string(),
                "Run".to_string(),
                "Debug".to_string(),
            ],
        }
    }

    pub fn view(&self) -> Element<Message> {
        let input = text_input("Type a command...", &self.query)
            .on_input(Message::InputChanged)
            .style(style::custom_text_input());

        let filtered_suggestions: Vec<_> = self
            .suggestions
            .iter()
            .filter(|s| s.to_lowercase().contains(&self.query.to_lowercase()))
            .collect();

        let suggestions = scrollable(
            Column::new()
                .spacing(4)
                .extend(filtered_suggestions.iter().map(|s| {
                    text(*s).style(iced::theme::Text::Color(style::TEXT)).into()
                }))
                .padding(8)
        )
        .style(style::custom_scrollable());

        let content = Column::new()
            .spacing(8)
            .push(input)
            .push(suggestions);

        container(content)
            .style(style::panel_container())
            .width(Length::Fixed(400.0))
            .height(Length::Fixed(300.0))
            .padding(16)
            .into()
    }
}