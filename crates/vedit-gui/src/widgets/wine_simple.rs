//! Simplified Wine integration widget for vedit

use crate::message::Message;
use iced::widget::{button, column, container, row, text};
use iced::{Alignment, Element, Length};
use iced_font_awesome::fa_icon_solid;

/// Simple Wine panel widget
pub fn render_wine_panel() -> Element<'static, Message> {
    let content = column![
        // Header
        row![
            row![
                fa_icon_solid("wine-glass").size(20.0).color(iced::Color::WHITE),
                text(" Wine Integration").size(20)
            ],
            iced::widget::Space::new().width(Length::Fill),
            button(row![
                fa_icon_solid("plus").size(14.0).color(iced::Color::WHITE),
                text(" Create Environment")
            ])
                .on_press(Message::WineCreateEnvironmentDialog)
        ]
        .align_y(Alignment::Center),

        // Environments section
        row![
            fa_icon_solid("box").size(16.0).color(iced::Color::WHITE),
            text(" Environments").size(16),
            iced::widget::Space::new().width(Length::Fill),
            button(row![
                fa_icon_solid("plus").size(12.0).color(iced::Color::WHITE),
                text(" Create")
            ])
                .on_press(Message::WineCreateEnvironmentDialog)
        ]
        .spacing(10),

        // Placeholder content
        text("No Wine environments created yet")
            .color(iced::Color::from_rgb(0.7, 0.7, 0.7)),

        iced::widget::Space::new().height(Length::Fixed(20.0)),

        // Processes section
        row![
            fa_icon_solid("gear").size(16.0).color(iced::Color::WHITE),
            text(" Processes").size(16),
            iced::widget::Space::new().width(Length::Fill),
        ]
        .spacing(10),

        text("No processes running")
            .color(iced::Color::from_rgb(0.7, 0.7, 0.7)),

        iced::widget::Space::new().height(Length::Fixed(20.0)),

        // Settings section
        row![
            fa_icon_solid("sliders").size(16.0).color(iced::Color::WHITE),
            text(" Settings").size(16),
            iced::widget::Space::new().width(Length::Fill),
        ]
        .spacing(10),

        column![
            row![fa_icon_solid("check-circle").size(14.0).color(iced::Color::from_rgb(0.2, 0.8, 0.2)), text(" Wine Status: Available")],
            row![fa_icon_solid("computer").size(14.0).color(iced::Color::WHITE), text(" System: Linux")],
            row![fa_icon_solid("microchip").size(14.0).color(iced::Color::WHITE), text(" Architecture: Ready")],
        ]
        .spacing(5)
    ]
    .spacing(15)
    .padding(20);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(crate::style::panel_container())
        .into()
}