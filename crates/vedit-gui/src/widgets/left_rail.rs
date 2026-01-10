use iced::widget::{button, container, row, scrollable, text, Column};
use iced::{Alignment, Element, Padding};
use iced_font_awesome::fa_icon_solid;

use crate::style;

#[derive(Debug, Clone)]
pub enum Message {
    OpenFile(String),
    CloseFile(String),
    ToggleRecentFiles,
}

pub struct OpenFilesList {
    pub files: Vec<String>,
    pub dirty_files: std::collections::HashSet<String>,
}

impl OpenFilesList {
    pub fn new() -> Self {
        Self {
            files: vec!["main.rs".to_string(), "lib.rs".to_string()],
            dirty_files: std::collections::HashSet::new(),
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let mut col = Column::new().spacing(4).padding(Padding::from(8));

        for file in &self.files {
            let is_dirty = self.dirty_files.contains(file);
            let dirty_dot = if is_dirty {
                fa_icon_solid("circle").color(iced::Color::WHITE)
            } else {
                fa_icon_solid("circle").color(iced::Color::TRANSPARENT)
            };

            let file_text = text(file).color(crate::style::TEXT);

            let close_button = button(fa_icon_solid("xmark").color(iced::Color::WHITE))
                .style(crate::style::custom_button())
                .on_press(Message::CloseFile(file.clone()));

            let item = row![dirty_dot, file_text, close_button]
                .spacing(4)
                .align_y(Alignment::Center);

            let button = button(item)
                .style(style::document_button())
                .on_press(Message::OpenFile(file.clone()));

            col = col.push(button);
        }

        scrollable(col).style(style::custom_scrollable()).into()
    }
}

pub struct RecentFiles {
    pub files: Vec<String>,
    pub collapsed: bool,
}

impl RecentFiles {
    pub fn new() -> Self {
        Self {
            files: vec!["old.rs".to_string(), "temp.rs".to_string()],
            collapsed: false,
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let icon = if self.collapsed {
            fa_icon_solid("angle-right").color(iced::Color::WHITE)
        } else {
            fa_icon_solid("angle-down").color(iced::Color::WHITE)
        };
        let header = button(row![
            icon,
            text("Recent Files").color(crate::style::TEXT)
        ].spacing(4.0))
            .style(crate::style::custom_button())
            .on_press(Message::ToggleRecentFiles);

        let mut col = Column::new().spacing(4).padding(Padding::from(8));
        col = col.push(header);

        if !self.collapsed {
            for file in &self.files {
                let item = button(text(file).color(crate::style::MUTED))
                    .style(crate::style::document_button())
                    .on_press(Message::OpenFile(file.clone()));

                col = col.push(item);
            }
        }

        container(col).style(style::panel_container()).into()
    }
}

