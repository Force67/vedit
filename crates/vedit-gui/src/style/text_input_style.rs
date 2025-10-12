//! Text input style definitions

use iced::theme;
use iced::widget::text_input;
use iced::{Background, Border, Color};

use super::{BORDER, SURFACE, SURFACE2, TEXT, MUTED};

pub fn default() -> theme::TextInput {
    theme::TextInput::Custom(Box::new(DefaultInput))
}

struct DefaultInput;

impl text_input::StyleSheet for DefaultInput {
    type Style = theme::Theme;

    fn active(&self, _style: &Self::Style) -> text_input::Appearance {
        text_input::Appearance {
            background: Background::Color(SURFACE),
            border: Border {
                radius: 4.0.into(),
                width: 1.0,
                color: BORDER,
            },
            icon_color: MUTED,
            text_color: TEXT,
            placeholder_color: MUTED,
            selection_color: Color::from_rgb(122, 162, 247),
        }
    }

    fn focused(&self, _style: &Self::Style) -> text_input::Appearance {
        text_input::Appearance {
            background: Background::Color(SURFACE2),
            border: Border {
                radius: 4.0.into(),
                width: 1.0,
                color: Color::from_rgb(122, 162, 247),
            },
            icon_color: TEXT,
            text_color: TEXT,
            placeholder_color: MUTED,
            selection_color: Color::from_rgb(122, 162, 247),
        }
    }

    fn hovered(&self, _style: &Self::Style) -> text_input::Appearance {
        text_input::Appearance {
            background: Background::Color(SURFACE2),
            border: Border {
                radius: 4.0.into(),
                width: 1.0,
                color: BORDER,
            },
            icon_color: TEXT,
            text_color: TEXT,
            placeholder_color: MUTED,
            selection_color: Color::from_rgb(122, 162, 247),
        }
    }

    fn disabled(&self, _style: &Self::Style) -> text_input::Appearance {
        text_input::Appearance {
            background: Background::Color(SURFACE),
            border: Border {
                radius: 4.0.into(),
                width: 1.0,
                color: BORDER,
            },
            icon_color: MUTED,
            text_color: MUTED,
            placeholder_color: MUTED,
            selection_color: Color::from_rgb(122, 162, 247),
        }
    }
}