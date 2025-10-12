//! Button style definitions

use iced::theme;
use iced::widget::button;
use iced::{Background, Border, Color};

use super::{BG, BORDER, MUTED, PRIMARY, PRIMARY_HOVER, SUCCESS, ERROR, SURFACE, SURFACE2};

pub fn primary() -> theme::Button {
    theme::Button::Custom(Box::new(PrimaryButton))
}

pub fn secondary() -> theme::Button {
    theme::Button::Custom(Box::new(SecondaryButton))
}

pub fn active() -> theme::Button {
    theme::Button::Custom(Box::new(ActiveButton))
}

pub fn text() -> theme::Button {
    theme::Button::Custom(Box::new(ButtonText))
}

pub fn destructive() -> theme::Button {
    theme::Button::Custom(Box::new(DestructiveButton))
}

pub fn active_tab() -> theme::Button {
    theme::Button::Custom(Box::new(ActiveTabButton))
}

pub fn inactive_tab() -> theme::Button {
    theme::Button::Custom(Box::new(InactiveTabButton))
}

struct PrimaryButton;

impl button::StyleSheet for PrimaryButton {
    type Style = theme::Theme;

    fn active(&self, _style: &Self::Style) -> button::Appearance {
        button::Appearance {
            background: Some(Background::Color(PRIMARY)),
            text_color: Color::WHITE,
            border: Border {
                radius: 4.0.into(),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
            shadow: Default::default(),
        }
    }

    fn hovered(&self, _style: &Self::Style) -> button::Appearance {
        button::Appearance {
            background: Some(Background::Color(PRIMARY_HOVER)),
            text_color: Color::WHITE,
            border: Border {
                radius: 4.0.into(),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
            shadow: Default::default(),
        }
    }

    fn pressed(&self, _style: &Self::Style) -> button::Appearance {
        button::Appearance {
            background: Some(Background::Color(PRIMARY)),
            text_color: Color::WHITE,
            border: Border {
                radius: 4.0.into(),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
            shadow: Default::default(),
        }
    }
}

struct SecondaryButton;

impl button::StyleSheet for SecondaryButton {
    type Style = theme::Theme;

    fn active(&self, _style: &Self::Style) -> button::Appearance {
        button::Appearance {
            background: Some(Background::Color(SURFACE2)),
            text_color: TEXT,
            border: Border {
                radius: 4.0.into(),
                width: 1.0,
                color: BORDER,
            },
            shadow: Default::default(),
        }
    }

    fn hovered(&self, _style: &Self::Style) -> button::Appearance {
        button::Appearance {
            background: Some(Background::Color(BORDER)),
            text_color: TEXT,
            border: Border {
                radius: 4.0.into(),
                width: 1.0,
                color: BORDER,
            },
            shadow: Default::default(),
        }
    }

    fn pressed(&self, _style: &Self::Style) -> button::Appearance {
        button::Appearance {
            background: Some(Background::Color(SURFACE)),
            text_color: TEXT,
            border: Border {
                radius: 4.0.into(),
                width: 1.0,
                color: BORDER,
            },
            shadow: Default::default(),
        }
    }
}

struct ActiveButton;

impl button::StyleSheet for ActiveButton {
    type Style = theme::Theme;

    fn active(&self, _style: &Self::Style) -> button::Appearance {
        button::Appearance {
            background: Some(Background::Color(SUCCESS)),
            text_color: Color::BLACK,
            border: Border {
                radius: 4.0.into(),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
            shadow: Default::default(),
        }
    }

    fn hovered(&self, _style: &Self::Style) -> button::Appearance {
        button::Appearance {
            background: Some(Background::Color(SUCCESS)),
            text_color: Color::BLACK,
            border: Border {
                radius: 4.0.into(),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
            shadow: Default::default(),
        }
    }

    fn pressed(&self, _style: &Self::Style) -> button::Appearance {
        button::Appearance {
            background: Some(Background::Color(SUCCESS)),
            text_color: Color::BLACK,
            border: Border {
                radius: 4.0.into(),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
            shadow: Default::default(),
        }
    }
}

struct ButtonText;

impl button::StyleSheet for ButtonText {
    type Style = theme::Theme;

    fn active(&self, _style: &Self::Style) -> button::Appearance {
        button::Appearance {
            background: None,
            text_color: MUTED,
            border: Border {
                radius: 2.0.into(),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
            shadow: Default::default(),
        }
    }

    fn hovered(&self, _style: &Self::Style) -> button::Appearance {
        button::Appearance {
            background: Some(Background::Color(SURFACE2)),
            text_color: TEXT,
            border: Border {
                radius: 2.0.into(),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
            shadow: Default::default(),
        }
    }

    fn pressed(&self, _style: &Self::Style) -> button::Appearance {
        button::Appearance {
            background: Some(Background::Color(SURFACE)),
            text_color: TEXT,
            border: Border {
                radius: 2.0.into(),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
            shadow: Default::default(),
        }
    }
}

struct DestructiveButton;

impl button::StyleSheet for DestructiveButton {
    type Style = theme::Theme;

    fn active(&self, _style: &Self::Style) -> button::Appearance {
        button::Appearance {
            background: Some(Background::Color(ERROR)),
            text_color: Color::WHITE,
            border: Border {
                radius: 4.0.into(),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
            shadow: Default::default(),
        }
    }

    fn hovered(&self, _style: &Self::Style) -> button::Appearance {
        button::Appearance {
            background: Some(Background::Color(Color::from_rgb(200, 90, 100))),
            text_color: Color::WHITE,
            border: Border {
                radius: 4.0.into(),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
            shadow: Default::default(),
        }
    }

    fn pressed(&self, _style: &Self::Style) -> button::Appearance {
        button::Appearance {
            background: Some(Background::Color(Color::from_rgb(150, 70, 80))),
            text_color: Color::WHITE,
            border: Border {
                radius: 4.0.into(),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
            shadow: Default::default(),
        }
    }
}

struct ActiveTabButton;

impl button::StyleSheet for ActiveTabButton {
    type Style = theme::Theme;

    fn active(&self, _style: &Self::Style) -> button::Appearance {
        button::Appearance {
            background: Some(Background::Color(SURFACE)),
            text_color: TEXT,
            border: Border {
                radius: 0.0,
                width: 1.0,
                color: BORDER,
            },
            shadow: Default::default(),
        }
    }

    fn hovered(&self, _style: &Self::Style) -> button::Appearance {
        button::Appearance {
            background: Some(Background::Color(SURFACE2)),
            text_color: TEXT,
            border: Border {
                radius: 0.0,
                width: 1.0,
                color: BORDER,
            },
            shadow: Default::default(),
        }
    }

    fn pressed(&self, _style: &Self::Style) -> button::Appearance {
        button::Appearance {
            background: Some(Background::Color(SURFACE)),
            text_color: TEXT,
            border: Border {
                radius: 0.0,
                width: 1.0,
                color: BORDER,
            },
            shadow: Default::default(),
        }
    }
}

struct InactiveTabButton;

impl button::StyleSheet for InactiveTabButton {
    type Style = theme::Theme;

    fn active(&self, _style: &Self::Style) -> button::Appearance {
        button::Appearance {
            background: None,
            text_color: MUTED,
            border: Border {
                radius: 0.0,
                width: 1.0,
                color: BORDER,
            },
            shadow: Default::default(),
        }
    }

    fn hovered(&self, _style: &Self::Style) -> button::Appearance {
        button::Appearance {
            background: Some(Background::Color(SURFACE)),
            text_color: TEXT,
            border: Border {
                radius: 0.0,
                width: 1.0,
                color: BORDER,
            },
            shadow: Default::default(),
        }
    }

    fn pressed(&self, _style: &Self::Style) -> button::Appearance {
        button::Appearance {
            background: Some(Background::Color(SURFACE2)),
            text_color: TEXT,
            border: Border {
                radius: 0.0,
                width: 1.0,
                color: BORDER,
            },
            shadow: Default::default(),
        }
    }
}

// Import TEXT from parent module
use super::TEXT;