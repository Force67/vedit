use iced::theme;
use iced::widget::{button, container, scrollable, text_input, pick_list, rule};
use iced::{Background, Border, Color, Shadow, Vector};

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::from_rgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
}

pub const BG: Color = rgb(14, 15, 18);
pub const SURFACE: Color = rgb(21, 23, 27);
pub const SURFACE2: Color = rgb(27, 30, 36);
pub const OVERLAY: Color = rgb(11, 12, 16);
pub const BORDER: Color = rgb(42, 46, 54);
pub const TEXT: Color = rgb(230, 234, 242);
pub const MUTED: Color = rgb(154, 166, 178);
pub const PRIMARY: Color = rgb(122, 162, 247);
pub const PRIMARY_HOVER: Color = rgb(143, 176, 250);
pub const SUCCESS: Color = rgb(128, 211, 155);
pub const WARNING: Color = rgb(230, 180, 80);
pub const ERROR: Color = rgb(228, 104, 118);
pub const FOCUS_RING: Color = Color::from_rgba(122.0 / 255.0, 162.0 / 255.0, 247.0 / 255.0, 0.55);

#[derive(Debug, Clone, Copy, Default)]
pub struct RootContainer;

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

#[derive(Debug, Clone, Copy, Default)]
pub struct PanelContainer;

impl container::StyleSheet for PanelContainer {
    type Style = theme::Theme;

    fn appearance(&self, _style: &Self::Style) -> container::Appearance {
        container::Appearance {
            background: Some(Background::Color(SURFACE)),
            border: Border {
                radius: 4.0.into(),
                width: 1.0,
                color: BORDER,
            },
            text_color: Some(TEXT),
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
            background: Some(Background::Color(SURFACE2)),
            border: Border {
                radius: 0.0.into(),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
            text_color: Some(TEXT),
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
            background: Some(Background::Color(OVERLAY)),
            border: Border {
                radius: 0.0.into(),
                width: 1.0,
                color: BORDER,
            },
            text_color: Some(MUTED),
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
            background: Some(Background::Color(SURFACE2)),
            text_color: TEXT,
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
            background: Some(Background::Color(PRIMARY_HOVER)),
            text_color: TEXT,
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
            background: Some(Background::Color(PRIMARY)),
            text_color: TEXT,
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
            text_color: MUTED,
            border: Border::default(),
            shadow: Default::default(),
        }
    }

    fn hovered(&self, _style: &Self::Style) -> button::Appearance {
        button::Appearance {
            shadow_offset: Vector::default(),
            background: Some(Background::Color(PRIMARY_HOVER)),
            text_color: TEXT,
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
            background: Some(Background::Color(PRIMARY)),
            text_color: TEXT,
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
            NotificationTone::Info => PRIMARY,
            NotificationTone::Success => SUCCESS,
            NotificationTone::Error => ERROR,
        };

        container::Appearance {
            background: Some(Background::Color(OVERLAY)),
            border: Border {
                radius: 12.0.into(),
                width: 1.0,
                color: Color::from_rgba(accent.r, accent.g, accent.b, 0.75),
            },
            text_color: Some(TEXT),
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

pub fn custom_container() -> theme::Container {
    theme::Container::Custom(Box::new(CustomContainer))
}

pub fn custom_button() -> theme::Button {
    theme::Button::Custom(Box::new(CustomButton))
}



pub fn custom_scrollable() -> theme::Scrollable {
    theme::Scrollable::Custom(Box::new(CustomScrollable))
}

pub fn custom_text_input() -> theme::TextInput {
    theme::TextInput::Custom(Box::new(CustomTextInput))
}



pub fn custom_rule() -> theme::Rule {
    theme::Rule::Custom(Box::new(CustomRule))
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CustomContainer;

impl container::StyleSheet for CustomContainer {
    type Style = theme::Theme;

    fn appearance(&self, _style: &Self::Style) -> container::Appearance {
        container::Appearance {
            background: Some(Background::Color(SURFACE)),
            text_color: Some(TEXT),
            border: Border {
                radius: 4.0.into(),
                width: 1.0,
                color: BORDER,
            },
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CustomButton;

impl button::StyleSheet for CustomButton {
    type Style = theme::Theme;

    fn active(&self, _style: &Self::Style) -> button::Appearance {
        button::Appearance {
            shadow_offset: Vector::default(),
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
            shadow_offset: Vector::default(),
            background: Some(Background::Color(PRIMARY_HOVER)),
            text_color: TEXT,
            border: Border {
                radius: 4.0.into(),
                width: 1.0,
                color: PRIMARY,
            },
            shadow: Default::default(),
        }
    }

    fn pressed(&self, _style: &Self::Style) -> button::Appearance {
        button::Appearance {
            shadow_offset: Vector::default(),
            background: Some(Background::Color(PRIMARY)),
            text_color: TEXT,
            border: Border {
                radius: 4.0.into(),
                width: 1.0,
                color: PRIMARY,
            },
            shadow: Default::default(),
        }
    }

    fn disabled(&self, _style: &Self::Style) -> button::Appearance {
        button::Appearance {
            shadow_offset: Vector::default(),
            background: Some(Background::Color(SURFACE)),
            text_color: MUTED,
            border: Border {
                radius: 4.0.into(),
                width: 1.0,
                color: BORDER,
            },
            shadow: Default::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SelectedButton;

impl button::StyleSheet for SelectedButton {
    type Style = theme::Theme;

    fn active(&self, _style: &Self::Style) -> button::Appearance {
        button::Appearance {
            shadow_offset: Vector::default(),
            background: Some(Background::Color(PRIMARY)),
            text_color: TEXT,
            border: Border {
                radius: 4.0.into(),
                width: 1.0,
                color: PRIMARY,
            },
            shadow: Default::default(),
        }
    }

    fn hovered(&self, _style: &Self::Style) -> button::Appearance {
        button::Appearance {
            shadow_offset: Vector::default(),
            background: Some(Background::Color(PRIMARY_HOVER)),
            text_color: TEXT,
            border: Border {
                radius: 4.0.into(),
                width: 1.0,
                color: PRIMARY_HOVER,
            },
            shadow: Default::default(),
        }
    }

    fn pressed(&self, _style: &Self::Style) -> button::Appearance {
        button::Appearance {
            shadow_offset: Vector::default(),
            background: Some(Background::Color(SURFACE2)),
            text_color: TEXT,
            border: Border {
                radius: 4.0.into(),
                width: 1.0,
                color: PRIMARY,
            },
            shadow: Default::default(),
        }
    }

    fn disabled(&self, _style: &Self::Style) -> button::Appearance {
        button::Appearance {
            shadow_offset: Vector::default(),
            background: Some(Background::Color(SURFACE)),
            text_color: MUTED,
            border: Border {
                radius: 4.0.into(),
                width: 1.0,
                color: BORDER,
            },
            shadow: Default::default(),
        }
    }
}

pub fn selected_button() -> theme::Button {
    theme::Button::Custom(Box::new(SelectedButton))
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CustomScrollable;

impl scrollable::StyleSheet for CustomScrollable {
    type Style = theme::Theme;

    fn active(&self, _style: &Self::Style) -> scrollable::Appearance {
        scrollable::Appearance {
            container: container::Appearance {
                background: Some(Background::Color(SURFACE)),
                text_color: Some(TEXT),
                ..Default::default()
            },
            scrollbar: scrollable::Scrollbar {
                background: Some(Background::Color(SURFACE2)),
                border: Border {
                    radius: 2.0.into(),
                    width: 0.0,
                    color: Color::TRANSPARENT,
                },
                scroller: scrollable::Scroller {
                    color: MUTED,
                    border: Border {
                        radius: 2.0.into(),
                        width: 0.0,
                        color: Color::TRANSPARENT,
                    },
                },
            },
            gap: None,
        }
    }

    fn hovered(&self, style: &Self::Style, _is_active: bool) -> scrollable::Appearance {
        let mut appearance = self.active(style);
        appearance.scrollbar.scroller.color = PRIMARY;
        appearance
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CustomTextInput;

impl text_input::StyleSheet for CustomTextInput {
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
        }
    }

    fn hovered(&self, style: &Self::Style) -> text_input::Appearance {
        let mut appearance = self.active(style);
        appearance.border.color = PRIMARY;
        appearance
    }

    fn focused(&self, style: &Self::Style) -> text_input::Appearance {
        let mut appearance = self.hovered(style);
        appearance.border.color = FOCUS_RING;
        appearance
    }

    fn placeholder_color(&self, _style: &Self::Style) -> Color {
        MUTED
    }

    fn value_color(&self, _style: &Self::Style) -> Color {
        TEXT
    }

    fn selection_color(&self, _style: &Self::Style) -> Color {
        PRIMARY
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
        }
    }

    fn disabled_color(&self, _style: &Self::Style) -> Color {
        MUTED
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CustomPickList;

impl pick_list::StyleSheet for CustomPickList {
    type Style = theme::Theme;

    fn active(&self, _style: &Self::Style) -> pick_list::Appearance {
        pick_list::Appearance {
            text_color: TEXT,
            placeholder_color: MUTED,
            handle_color: TEXT,
            background: Background::Color(SURFACE),
            border: Border {
                radius: 4.0.into(),
                width: 1.0,
                color: BORDER,
            },
        }
    }

    fn hovered(&self, style: &Self::Style) -> pick_list::Appearance {
        let mut appearance = self.active(style);
        appearance.border.color = PRIMARY;
        appearance
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CustomRule;

impl rule::StyleSheet for CustomRule {
    type Style = theme::Theme;

    fn appearance(&self, _style: &Self::Style) -> rule::Appearance {
        rule::Appearance {
            color: BORDER,
            width: 1,
            radius: 0.0.into(),
            fill_mode: rule::FillMode::Full,
        }
    }
}
