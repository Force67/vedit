//! Text input style definitions

use iced::theme;
use iced::widget::text_input;
use iced::{Background, Border, Color};

use super::{BORDER_SUBTLE, PRIMARY, SURFACE, SURFACE_HOVER, TEXT, MUTED};

/// Selection highlight color with transparency
const SELECTION: Color = Color {
    r: 88.0 / 255.0,
    g: 140.0 / 255.0,
    b: 220.0 / 255.0,
    a: 0.35,
};

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
                radius: 6.0.into(),
                width: 1.0,
                color: BORDER_SUBTLE,
            },
            icon_color: MUTED,
            text_color: TEXT,
            placeholder_color: MUTED,
            selection_color: SELECTION,
        }
    }

    fn focused(&self, _style: &Self::Style) -> text_input::Appearance {
        text_input::Appearance {
            background: Background::Color(SURFACE_HOVER),
            border: Border {
                radius: 6.0.into(),
                width: 2.0, // Thicker border for focus ring
                color: PRIMARY,
            },
            icon_color: PRIMARY,
            text_color: TEXT,
            placeholder_color: MUTED,
            selection_color: SELECTION,
        }
    }

    fn hovered(&self, _style: &Self::Style) -> text_input::Appearance {
        text_input::Appearance {
            background: Background::Color(SURFACE_HOVER),
            border: Border {
                radius: 6.0.into(),
                width: 1.0,
                color: BORDER_SUBTLE,
            },
            icon_color: TEXT,
            text_color: TEXT,
            placeholder_color: MUTED,
            selection_color: SELECTION,
        }
    }

    fn disabled(&self, _style: &Self::Style) -> text_input::Appearance {
        text_input::Appearance {
            background: Background::Color(SURFACE),
            border: Border {
                radius: 6.0.into(),
                width: 1.0,
                color: BORDER_SUBTLE,
            },
            icon_color: MUTED,
            text_color: MUTED,
            placeholder_color: MUTED,
            selection_color: SELECTION,
        }
    }
}
