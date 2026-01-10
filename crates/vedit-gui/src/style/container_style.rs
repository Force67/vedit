//! Container style definitions

use iced::theme;
use iced::widget::container;
use iced::{Background, Border};

use super::{elevation, BG, BORDER_SUBTLE, PRIMARY, SURFACE, SURFACE2, OVERLAY, TEXT};

pub fn card() -> theme::Container {
    theme::Container::Custom(Box::new(CardContainer))
}

pub fn selected() -> theme::Container {
    theme::Container::Custom(Box::new(SelectedContainer))
}

pub fn modal() -> theme::Container {
    theme::Container::Custom(Box::new(ModalContainer))
}

pub fn root_container() -> theme::Container {
    theme::Container::Custom(Box::new(RootContainer))
}

struct CardContainer;

impl container::StyleSheet for CardContainer {
    type Style = theme::Theme;

    fn appearance(&self, _style: &Self::Style) -> container::Appearance {
        container::Appearance {
            background: Some(Background::Color(SURFACE)),
            border: Border {
                radius: 8.0.into(),
                width: 1.0,
                color: BORDER_SUBTLE,
            },
            text_color: Some(TEXT),
            shadow: elevation::level_1(),
        }
    }
}

struct SelectedContainer;

impl container::StyleSheet for SelectedContainer {
    type Style = theme::Theme;

    fn appearance(&self, _style: &Self::Style) -> container::Appearance {
        container::Appearance {
            background: Some(Background::Color(SURFACE2)),
            border: Border {
                radius: 8.0.into(),
                width: 2.0,
                color: PRIMARY,
            },
            text_color: Some(TEXT),
            shadow: elevation::level_2(),
        }
    }
}

struct ModalContainer;

impl container::StyleSheet for ModalContainer {
    type Style = theme::Theme;

    fn appearance(&self, _style: &Self::Style) -> container::Appearance {
        container::Appearance {
            background: Some(Background::Color(OVERLAY)),
            border: Border {
                radius: 12.0.into(),
                width: 1.0,
                color: BORDER_SUBTLE,
            },
            text_color: Some(TEXT),
            shadow: elevation::level_3(),
        }
    }
}

struct RootContainer;

impl container::StyleSheet for RootContainer {
    type Style = theme::Theme;

    fn appearance(&self, _style: &Self::Style) -> container::Appearance {
        container::Appearance {
            background: Some(Background::Color(BG)),
            text_color: Some(TEXT),
            ..Default::default()
        }
    }
}
