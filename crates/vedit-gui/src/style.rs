use iced::theme;
use iced::widget::{button, container};
use iced::{Background, Border, Color, Shadow, Vector};

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::from_rgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
}

const BG_PRIMARY: Color = rgb(30, 30, 30);
const BG_PANEL: Color = rgb(45, 45, 48);
const BG_BUTTON_HOVER: Color = rgb(62, 62, 66);
const ACCENT: Color = rgb(0, 120, 215);
const BG_STATUS: Color = rgb(40, 40, 43);
const BG_RIBBON: Color = rgb(37, 37, 38);
const TEXT_PRIMARY: Color = rgb(231, 231, 231);
const TEXT_MUTED: Color = rgb(180, 180, 180);
const NOTIFY_SUCCESS: Color = rgb(39, 174, 96);
const NOTIFY_INFO: Color = rgb(52, 152, 219);
const NOTIFY_ERROR: Color = rgb(231, 76, 60);
const NOTIFY_SURFACE: Color = rgb(36, 36, 39);

#[derive(Debug, Clone, Copy, Default)]
pub struct RootContainer;

impl container::StyleSheet for RootContainer {
    type Style = theme::Theme;

    fn appearance(&self, _style: &Self::Style) -> container::Appearance {
        container::Appearance {
            background: Some(Background::Color(BG_PRIMARY)),
            text_color: Some(TEXT_PRIMARY),
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PanelContainer;

impl container::StyleSheet for PanelContainer {
    type Style = theme::Theme;

    fn appearance(&self, _style: &Self::Style) -> container::Appearance {
        container::Appearance {
            background: Some(Background::Color(BG_PANEL)),
            border: Border {
                radius: 4.0.into(),
                width: 1.0,
                color: rgb(60, 60, 63),
            },
            text_color: Some(TEXT_PRIMARY),
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RibbonContainer;

impl container::StyleSheet for RibbonContainer {
    type Style = theme::Theme;

    fn appearance(&self, _style: &Self::Style) -> container::Appearance {
        container::Appearance {
            background: Some(Background::Color(BG_RIBBON)),
            border: Border {
                radius: 0.0.into(),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
            text_color: Some(TEXT_PRIMARY),
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct StatusContainer;

impl container::StyleSheet for StatusContainer {
    type Style = theme::Theme;

    fn appearance(&self, _style: &Self::Style) -> container::Appearance {
        container::Appearance {
            background: Some(Background::Color(BG_STATUS)),
            border: Border {
                radius: 0.0.into(),
                width: 1.0,
                color: rgb(63, 63, 70),
            },
            text_color: Some(TEXT_MUTED),
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TopBarButton;

impl button::StyleSheet for TopBarButton {
    type Style = theme::Theme;

    fn active(&self, _style: &Self::Style) -> button::Appearance {
        button::Appearance {
            shadow_offset: Vector::default(),
            background: Some(Background::Color(BG_RIBBON)),
            text_color: TEXT_PRIMARY,
            border: Border {
                radius: 3.0.into(),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
            shadow: Default::default(),
        }
    }

    fn hovered(&self, _style: &Self::Style) -> button::Appearance {
        button::Appearance {
            shadow_offset: Vector::default(),
            background: Some(Background::Color(BG_BUTTON_HOVER)),
            text_color: TEXT_PRIMARY,
            border: Border {
                radius: 3.0.into(),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
            shadow: Default::default(),
        }
    }

    fn pressed(&self, _style: &Self::Style) -> button::Appearance {
        button::Appearance {
            shadow_offset: Vector::default(),
            background: Some(Background::Color(ACCENT)),
            text_color: TEXT_PRIMARY,
            border: Border {
                radius: 3.0.into(),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
            shadow: Default::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DocumentButton;

impl button::StyleSheet for DocumentButton {
    type Style = theme::Theme;

    fn active(&self, _style: &Self::Style) -> button::Appearance {
        button::Appearance {
            shadow_offset: Vector::default(),
            background: None,
            text_color: TEXT_MUTED,
            border: Border::default(),
            shadow: Default::default(),
        }
    }

    fn hovered(&self, _style: &Self::Style) -> button::Appearance {
        button::Appearance {
            shadow_offset: Vector::default(),
            background: Some(Background::Color(BG_BUTTON_HOVER)),
            text_color: TEXT_PRIMARY,
            border: Border::default(),
            shadow: Default::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ActiveDocumentButton;

impl button::StyleSheet for ActiveDocumentButton {
    type Style = theme::Theme;

    fn active(&self, _style: &Self::Style) -> button::Appearance {
        button::Appearance {
            shadow_offset: Vector::default(),
            background: Some(Background::Color(ACCENT)),
            text_color: TEXT_PRIMARY,
            border: Border::default(),
            shadow: Default::default(),
        }
    }

    fn hovered(&self, style: &Self::Style) -> button::Appearance {
        self.active(style)
    }
}

pub fn root_container() -> theme::Container {
    theme::Container::Custom(Box::new(RootContainer))
}

pub fn panel_container() -> theme::Container {
    theme::Container::Custom(Box::new(PanelContainer))
}

pub fn ribbon_container() -> theme::Container {
    theme::Container::Custom(Box::new(RibbonContainer))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationTone {
    Info,
    Success,
    Error,
}

#[derive(Debug, Clone, Copy)]
pub struct NotificationContainer {
    tone: NotificationTone,
}

impl NotificationContainer {
    pub fn new(tone: NotificationTone) -> Self {
        Self { tone }
    }
}

impl container::StyleSheet for NotificationContainer {
    type Style = theme::Theme;

    fn appearance(&self, _style: &Self::Style) -> container::Appearance {
        let accent = match self.tone {
            NotificationTone::Info => NOTIFY_INFO,
            NotificationTone::Success => NOTIFY_SUCCESS,
            NotificationTone::Error => NOTIFY_ERROR,
        };

        container::Appearance {
            background: Some(Background::Color(NOTIFY_SURFACE)),
            border: Border {
                radius: 12.0.into(),
                width: 1.0,
                color: Color::from_rgba(accent.r, accent.g, accent.b, 0.75),
            },
            text_color: Some(TEXT_PRIMARY),
            shadow: Shadow {
                offset: Vector::new(0.0, 6.0),
                blur_radius: 18.0,
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.45),
            },
        }
    }
}

pub fn notification_container(tone: NotificationTone) -> theme::Container {
    theme::Container::Custom(Box::new(NotificationContainer::new(tone)))
}

pub fn status_container() -> theme::Container {
    theme::Container::Custom(Box::new(StatusContainer))
}

pub fn top_bar_button() -> theme::Button {
    theme::Button::Custom(Box::new(TopBarButton))
}

pub fn document_button() -> theme::Button {
    theme::Button::Custom(Box::new(DocumentButton))
}

pub fn active_document_button() -> theme::Button {
    theme::Button::Custom(Box::new(ActiveDocumentButton))
}
