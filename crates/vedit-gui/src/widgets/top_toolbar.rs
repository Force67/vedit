use iced::widget::{button, container, row, text, tooltip, Row};
use iced::{Alignment, Element, Length};

use crate::style;

#[derive(Debug, Clone)]
pub enum Message {
    NewFile,
    OpenFile,
    SaveFile,
    Run,
    Debug,
    Settings,
}

pub struct TopToolbar;

impl TopToolbar {
    pub fn new() -> Self {
        Self
    }

    pub fn view(&self) -> Element<Message> {
        let file_group = Row::new()
            .spacing(8)
            .push(
                tooltip(
                    button(text("New").color(style::TEXT))
                        .style(style::custom_button())
                        .on_press(Message::NewFile),
                    "New File (Ctrl+N)",
                    tooltip::Position::Bottom,
                )
            )
            .push(
                tooltip(
                    button(text("Open").color(style::TEXT))
                        .style(style::custom_button())
                        .on_press(Message::OpenFile),
                    "Open File (Ctrl+O)",
                    tooltip::Position::Bottom,
                )
            )
            .push(
                tooltip(
                    button(text("Save").color(style::TEXT))
                        .style(style::custom_button())
                        .on_press(Message::SaveFile),
                    "Save File (Ctrl+S)",
                    tooltip::Position::Bottom,
                )
            );

        let run_group = Row::new()
            .spacing(8)
            .push(
                tooltip(
                    button(text("Run").color(style::TEXT))
                        .style(style::custom_button())
                        .on_press(Message::Run),
                    "Run (F5)",
                    tooltip::Position::Bottom,
                )
            )
            .push(
                tooltip(
                    button(text("Debug").color(style::TEXT))
                        .style(style::custom_button())
                        .on_press(Message::Debug),
                    "Debug (F10)",
                    tooltip::Position::Bottom,
                )
            );

        let settings_group = Row::new()
            .spacing(8)
            .push(
                tooltip(
                    button(text("Settings").color(style::TEXT))
                        .style(style::custom_button())
                        .on_press(Message::Settings),
                    "Settings (Ctrl+,)",
                    tooltip::Position::Bottom,
                )
            );

        let toolbar = Row::new()
            .spacing(16)
            .push(file_group)
            .push(run_group)
            .push(settings_group)
            .align_y(Alignment::Center);

        container(toolbar)
            .style(style::ribbon_container())
            .padding(8)
            .width(Length::Fill)
            .into()
    }
}