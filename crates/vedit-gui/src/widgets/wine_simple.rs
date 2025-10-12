//! Simplified Wine integration widget for vedit

use crate::message::Message;
use iced::widget::{button, column, container, row, text, Scrollable};
use iced::{Alignment, Element, Length};

/// Simple Wine panel widget
pub fn render_wine_panel() -> Element<'static, Message> {
    let content = column![
        // Header
        row![
            text("🍷 Wine Integration").size(20),
            iced::widget::Space::with_width(Length::Fill),
            button(text("+ Create Environment"))
                .on_press(Message::WineCreateEnvironmentDialog)
        ]
        .align_items(Alignment::Center),

        // Environments section
        row![
            text("Environments").size(16),
            iced::widget::Space::with_width(Length::Fill),
            button(text("Create"))
                .on_press(Message::WineCreateEnvironmentDialog)
        ]
        .spacing(10),

        // Placeholder content
        text("No Wine environments created yet")
            .style(iced::Color::from_rgb(0.7, 0.7, 0.7)),

        iced::widget::Space::with_height(Length::Fixed(20.0)),

        // Processes section
        row![
            text("Processes").size(16),
            iced::widget::Space::with_width(Length::Fill),
        ]
        .spacing(10),

        text("No processes running")
            .style(iced::Color::from_rgb(0.7, 0.7, 0.7)),

        iced::widget::Space::with_height(Length::Fixed(20.0)),

        // Settings section
        row![
            text("Settings").size(16),
            iced::widget::Space::with_width(Length::Fill),
        ]
        .spacing(10),

        column![
            text("Wine Status: Available"),
            text("System: Linux"),
            text("Architecture: Ready"),
        ]
        .spacing(5)
    ]
    .spacing(15)
    .padding(20);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(iced::theme::Container::Box)
        .into()
}