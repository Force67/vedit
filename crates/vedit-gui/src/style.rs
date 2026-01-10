use iced::widget::{button, container, rule, scrollable, text_input};
use iced::{Background, Border, Color, Shadow, Theme, Vector};

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

// Container styles
pub fn root_container() -> impl Fn(&Theme) -> container::Style {
    |_theme| container::Style {
        background: Some(Background::Color(BG)),
        text_color: Some(TEXT),
        ..Default::default()
    }
}

pub fn panel_container() -> impl Fn(&Theme) -> container::Style {
    |_theme| container::Style {
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

pub fn ribbon_container() -> impl Fn(&Theme) -> container::Style {
    |_theme| container::Style {
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

pub fn status_container() -> impl Fn(&Theme) -> container::Style {
    |_theme| container::Style {
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

pub fn custom_container() -> impl Fn(&Theme) -> container::Style {
    |_theme| container::Style {
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

pub fn floating_panel_container() -> impl Fn(&Theme) -> container::Style {
    |_theme| container::Style {
        background: Some(Background::Color(SURFACE)),
        text_color: Some(TEXT),
        border: Border {
            radius: 12.0.into(),
            width: 1.0,
            color: BORDER,
        },
        ..Default::default()
    }
}

pub fn overlay_container() -> impl Fn(&Theme) -> container::Style {
    |_theme| container::Style {
        background: None,
        ..Default::default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationTone {
    Info,
    Success,
    Error,
}

pub fn notification_container(tone: NotificationTone) -> impl Fn(&Theme) -> container::Style {
    move |_theme| {
        let accent = match tone {
            NotificationTone::Info => PRIMARY,
            NotificationTone::Success => SUCCESS,
            NotificationTone::Error => ERROR,
        };

        container::Style {
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
            snap: true,
        }
    }
}

// Button styles
pub fn top_bar_button() -> impl Fn(&Theme, button::Status) -> button::Style {
    |_theme, status| {
        let base = button::Style {
            background: Some(Background::Color(SURFACE2)),
            text_color: TEXT,
            border: Border {
                radius: 3.0.into(),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
            shadow: Default::default(),
            snap: true,
        };

        match status {
            button::Status::Active => base,
            button::Status::Hovered => button::Style {
                background: Some(Background::Color(PRIMARY_HOVER)),
                ..base
            },
            button::Status::Pressed => button::Style {
                background: Some(Background::Color(PRIMARY)),
                ..base
            },
            button::Status::Disabled => button::Style {
                background: Some(Background::Color(SURFACE)),
                text_color: MUTED,
                ..base
            },
        }
    }
}

pub fn document_button() -> impl Fn(&Theme, button::Status) -> button::Style {
    |_theme, status| {
        let base = button::Style {
            background: None,
            text_color: MUTED,
            border: Border::default(),
            shadow: Default::default(),
            snap: true,
        };

        match status {
            button::Status::Active | button::Status::Disabled => base,
            button::Status::Hovered | button::Status::Pressed => button::Style {
                background: Some(Background::Color(PRIMARY_HOVER)),
                text_color: TEXT,
                ..base
            },
        }
    }
}

pub fn active_document_button() -> impl Fn(&Theme, button::Status) -> button::Style {
    |_theme, _status| button::Style {
        background: Some(Background::Color(PRIMARY)),
        text_color: TEXT,
        border: Border::default(),
        shadow: Default::default(),
        snap: true,
    }
}

pub fn custom_button() -> impl Fn(&Theme, button::Status) -> button::Style {
    |_theme, status| {
        let base = button::Style {
            background: Some(Background::Color(SURFACE2)),
            text_color: TEXT,
            border: Border {
                radius: 4.0.into(),
                width: 1.0,
                color: BORDER,
            },
            shadow: Default::default(),
            snap: true,
        };

        match status {
            button::Status::Active => base,
            button::Status::Hovered => button::Style {
                background: Some(Background::Color(PRIMARY_HOVER)),
                border: Border {
                    radius: 4.0.into(),
                    width: 1.0,
                    color: PRIMARY,
                },
                ..base
            },
            button::Status::Pressed => button::Style {
                background: Some(Background::Color(PRIMARY)),
                border: Border {
                    radius: 4.0.into(),
                    width: 1.0,
                    color: PRIMARY,
                },
                ..base
            },
            button::Status::Disabled => button::Style {
                background: Some(Background::Color(SURFACE)),
                text_color: MUTED,
                ..base
            },
        }
    }
}

pub fn selected_button() -> impl Fn(&Theme, button::Status) -> button::Style {
    |_theme, status| {
        let base = button::Style {
            background: Some(Background::Color(PRIMARY)),
            text_color: TEXT,
            border: Border {
                radius: 4.0.into(),
                width: 1.0,
                color: PRIMARY,
            },
            shadow: Default::default(),
            snap: true,
        };

        match status {
            button::Status::Active => base,
            button::Status::Hovered => button::Style {
                background: Some(Background::Color(PRIMARY_HOVER)),
                border: Border {
                    radius: 4.0.into(),
                    width: 1.0,
                    color: PRIMARY_HOVER,
                },
                ..base
            },
            button::Status::Pressed => button::Style {
                background: Some(Background::Color(SURFACE2)),
                border: Border {
                    radius: 4.0.into(),
                    width: 1.0,
                    color: PRIMARY,
                },
                ..base
            },
            button::Status::Disabled => button::Style {
                background: Some(Background::Color(SURFACE)),
                text_color: MUTED,
                border: Border {
                    radius: 4.0.into(),
                    width: 1.0,
                    color: BORDER,
                },
                ..base
            },
        }
    }
}

// Scrollable styles
pub fn custom_scrollable() -> impl Fn(&Theme, scrollable::Status) -> scrollable::Style {
    |_theme, status| {
        let scroller_color = match status {
            scrollable::Status::Active { .. } => MUTED,
            scrollable::Status::Hovered { .. } | scrollable::Status::Dragged { .. } => PRIMARY,
        };

        let rail = scrollable::Rail {
            background: Some(Background::Color(SURFACE2)),
            border: Border {
                radius: 2.0.into(),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
            scroller: scrollable::Scroller {
                background: Background::Color(scroller_color),
                border: Border {
                    radius: 2.0.into(),
                    width: 0.0,
                    color: Color::TRANSPARENT,
                },
            },
        };

        scrollable::Style {
            container: container::Style {
                background: Some(Background::Color(SURFACE)),
                text_color: Some(TEXT),
                ..Default::default()
            },
            vertical_rail: rail,
            horizontal_rail: rail,
            gap: None,
            auto_scroll: scrollable::AutoScroll {
                background: Background::Color(SURFACE2),
                border: Border {
                    radius: 4.0.into(),
                    width: 1.0,
                    color: MUTED,
                },
                shadow: Shadow::default(),
                icon: TEXT,
            },
        }
    }
}

// Text input styles
pub fn custom_text_input() -> impl Fn(&Theme, text_input::Status) -> text_input::Style {
    |_theme, status| {
        let base = text_input::Style {
            background: Background::Color(SURFACE),
            border: Border {
                radius: 4.0.into(),
                width: 1.0,
                color: BORDER,
            },
            icon: MUTED,
            placeholder: MUTED,
            value: TEXT,
            selection: PRIMARY,
        };

        match status {
            text_input::Status::Active => base,
            text_input::Status::Hovered => text_input::Style {
                border: Border {
                    color: PRIMARY,
                    ..base.border
                },
                ..base
            },
            text_input::Status::Focused { .. } => text_input::Style {
                border: Border {
                    color: FOCUS_RING,
                    ..base.border
                },
                ..base
            },
            text_input::Status::Disabled => base,
        }
    }
}

// Rule styles
pub fn custom_rule() -> impl Fn(&Theme) -> rule::Style {
    |_theme| rule::Style {
        color: BORDER,
        radius: 0.0.into(),
        fill_mode: rule::FillMode::Full,
        snap: true,
    }
}
