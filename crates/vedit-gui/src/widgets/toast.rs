use iced::widget::{Column, container, text};
use iced::{Element, Length};

use crate::style;

#[derive(Debug, Clone)]
pub enum Message {
    Dismiss,
}

pub struct Toast {
    pub message: String,
    pub level: ToastLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastLevel {
    Info,
    Success,
    Warning,
    Error,
}

impl ToastLevel {
    pub fn color(&self) -> iced::Color {
        match self {
            ToastLevel::Info => style::PRIMARY,
            ToastLevel::Success => style::SUCCESS,
            ToastLevel::Warning => style::WARNING,
            ToastLevel::Error => style::ERROR,
        }
    }
}

impl Toast {
    pub fn new(message: String, level: ToastLevel) -> Self {
        Self { message, level }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let content = Column::new()
            .spacing(8)
            .push(text(&self.message).color(style::TEXT))
            .push(text("Dismiss").color(self.level.color()));

        container(content)
            .style(style::notification_container(self.level.into()))
            .width(Length::Fixed(300.0))
            .padding(16)
            .into()
    }
}

impl From<ToastLevel> for crate::style::NotificationTone {
    fn from(level: ToastLevel) -> Self {
        match level {
            ToastLevel::Info => crate::style::NotificationTone::Info,
            ToastLevel::Success => crate::style::NotificationTone::Success,
            ToastLevel::Warning => crate::style::NotificationTone::Info, // or add Warning
            ToastLevel::Error => crate::style::NotificationTone::Error,
        }
    }
}
