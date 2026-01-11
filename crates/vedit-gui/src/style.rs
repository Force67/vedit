use iced::widget::{button, container, scrollable};
use iced::{Background, Border, Color, Shadow, Theme};

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::from_rgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
}

// Base colors - nuanced dark theme with good contrast
pub const BG: Color = rgb(17, 18, 23); // Editor background
pub const BG_ELEVATED: Color = rgb(24, 26, 32); // Title bar, elevated surfaces
pub const SURFACE: Color = rgb(30, 33, 40); // Panel backgrounds
pub const SURFACE_HOVER: Color = rgb(38, 42, 50); // Hover states
pub const SURFACE2: Color = rgb(45, 49, 58); // Active/selected surfaces
pub const OVERLAY: Color = rgb(14, 15, 19); // Modal overlays
pub const BORDER: Color = rgb(50, 56, 68); // Standard borders
pub const BORDER_SUBTLE: Color = rgb(40, 45, 54); // Subtle separators

// Text hierarchy
pub const TEXT: Color = rgb(235, 238, 245); // Primary text
pub const TEXT_SECONDARY: Color = rgb(170, 180, 195); // Secondary text
pub const MUTED: Color = rgb(110, 120, 135); // Disabled/placeholder

// Accent colors with variations
pub const PRIMARY: Color = rgb(88, 140, 220); // Primary blue
pub const PRIMARY_HOVER: Color = rgb(108, 160, 240);
pub const PRIMARY_PRESSED: Color = rgb(70, 120, 200);

// Semantic colors
pub const SUCCESS: Color = rgb(72, 195, 130);
pub const WARNING: Color = rgb(235, 190, 80);
pub const ERROR: Color = rgb(225, 95, 105);

// Focus indicators
pub const FOCUS_RING: Color = Color {
    r: 88.0 / 255.0,
    g: 140.0 / 255.0,
    b: 220.0 / 255.0,
    a: 0.6,
};

pub mod elevation {
    use iced::{Color, Shadow, Vector};

    /// No shadow - flat elements
    pub fn level_0() -> Shadow {
        Shadow::default()
    }

    /// Subtle shadow - buttons, cards
    pub fn level_1() -> Shadow {
        Shadow {
            offset: Vector::new(0.0, 1.0),
            blur_radius: 3.0,
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.15),
        }
    }

    /// Medium shadow - hovered buttons, panels
    pub fn level_2() -> Shadow {
        Shadow {
            offset: Vector::new(0.0, 3.0),
            blur_radius: 8.0,
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.20),
        }
    }

    /// Prominent shadow - floating panels, dialogs
    pub fn level_3() -> Shadow {
        Shadow {
            offset: Vector::new(0.0, 6.0),
            blur_radius: 16.0,
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.25),
        }
    }

    /// Maximum shadow - notifications, modals
    pub fn level_4() -> Shadow {
        Shadow {
            offset: Vector::new(0.0, 10.0),
            blur_radius: 24.0,
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.30),
        }
    }
}

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
            radius: 6.0.into(),
            width: 1.0,
            color: BORDER_SUBTLE,
        },
        text_color: Some(TEXT),
        shadow: elevation::level_1(),
        snap: true,
    }
}

/// Title bar container with subtle elevation
pub fn title_bar_container() -> impl Fn(&Theme) -> container::Style {
    |_theme| container::Style {
        background: Some(Background::Color(BG_ELEVATED)),
        border: Border {
            radius: 0.0.into(),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        text_color: Some(TEXT),
        shadow: elevation::level_1(),
        snap: true,
    }
}

pub fn ribbon_container() -> impl Fn(&Theme) -> container::Style {
    |_theme| container::Style {
        background: Some(Background::Color(BG_ELEVATED)),
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
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        text_color: Some(TEXT_SECONDARY),
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
            color: BORDER_SUBTLE,
        },
        shadow: elevation::level_3(),
        snap: true,
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
            background: Some(Background::Color(SURFACE)),
            border: Border {
                radius: 12.0.into(),
                width: 1.0,
                color: Color::from_rgba(accent.r, accent.g, accent.b, 0.6),
            },
            text_color: Some(TEXT),
            shadow: elevation::level_4(),
            snap: true,
        }
    }
}

pub fn top_bar_button() -> impl Fn(&Theme, button::Status) -> button::Style {
    |_theme, status| {
        let base = button::Style {
            background: Some(Background::Color(SURFACE_HOVER)),
            text_color: TEXT,
            border: Border {
                radius: 5.0.into(),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
            shadow: Shadow::default(),
            snap: true,
        };

        match status {
            button::Status::Active => button::Style {
                background: None,
                ..base
            },
            button::Status::Hovered => button::Style {
                background: Some(Background::Color(SURFACE_HOVER)),
                shadow: elevation::level_1(),
                ..base
            },
            button::Status::Pressed => button::Style {
                background: Some(Background::Color(SURFACE2)),
                shadow: Shadow::default(),
                ..base
            },
            button::Status::Disabled => button::Style {
                background: None,
                text_color: MUTED,
                ..base
            },
        }
    }
}

/// Window control button style (minimize, maximize)
pub fn window_control_button() -> impl Fn(&Theme, button::Status) -> button::Style {
    |_theme, status| {
        let base = button::Style {
            background: None,
            text_color: TEXT_SECONDARY,
            border: Border {
                radius: 4.0.into(),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
            shadow: Shadow::default(),
            snap: true,
        };

        match status {
            button::Status::Active => base,
            button::Status::Hovered => button::Style {
                background: Some(Background::Color(SURFACE_HOVER)),
                text_color: TEXT,
                ..base
            },
            button::Status::Pressed => button::Style {
                background: Some(Background::Color(SURFACE2)),
                text_color: TEXT,
                ..base
            },
            button::Status::Disabled => button::Style {
                text_color: MUTED,
                ..base
            },
        }
    }
}

/// Window close button - red on hover
pub fn window_close_button() -> impl Fn(&Theme, button::Status) -> button::Style {
    |_theme, status| {
        let base = button::Style {
            background: None,
            text_color: TEXT_SECONDARY,
            border: Border {
                radius: 4.0.into(),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
            shadow: Shadow::default(),
            snap: true,
        };

        match status {
            button::Status::Active => base,
            button::Status::Hovered => button::Style {
                background: Some(Background::Color(ERROR)),
                text_color: TEXT,
                ..base
            },
            button::Status::Pressed => button::Style {
                background: Some(Background::Color(rgb(180, 70, 80))),
                text_color: TEXT,
                ..base
            },
            button::Status::Disabled => button::Style {
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
            text_color: TEXT_SECONDARY,
            border: Border {
                radius: 4.0.into(),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
            shadow: Shadow::default(),
            snap: true,
        };

        match status {
            button::Status::Active | button::Status::Disabled => base,
            button::Status::Hovered => button::Style {
                background: Some(Background::Color(SURFACE_HOVER)),
                text_color: TEXT,
                ..base
            },
            button::Status::Pressed => button::Style {
                background: Some(Background::Color(SURFACE2)),
                text_color: TEXT,
                ..base
            },
        }
    }
}

pub fn active_document_button() -> impl Fn(&Theme, button::Status) -> button::Style {
    |_theme, status| {
        let base = button::Style {
            background: Some(Background::Color(PRIMARY)),
            text_color: TEXT,
            border: Border {
                radius: 4.0.into(),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
            shadow: elevation::level_1(),
            snap: true,
        };

        match status {
            button::Status::Active => base,
            button::Status::Hovered => button::Style {
                background: Some(Background::Color(PRIMARY_HOVER)),
                shadow: elevation::level_2(),
                ..base
            },
            button::Status::Pressed => button::Style {
                background: Some(Background::Color(PRIMARY_PRESSED)),
                shadow: Shadow::default(),
                ..base
            },
            button::Status::Disabled => button::Style {
                background: Some(Background::Color(SURFACE2)),
                text_color: MUTED,
                shadow: Shadow::default(),
                ..base
            },
        }
    }
}

pub fn custom_button() -> impl Fn(&Theme, button::Status) -> button::Style {
    |_theme, status| {
        let base = button::Style {
            background: Some(Background::Color(SURFACE)),
            text_color: TEXT,
            border: Border {
                radius: 6.0.into(),
                width: 1.0,
                color: BORDER_SUBTLE,
            },
            shadow: elevation::level_1(),
            snap: true,
        };

        match status {
            button::Status::Active => base,
            button::Status::Hovered => button::Style {
                background: Some(Background::Color(SURFACE_HOVER)),
                border: Border {
                    radius: 6.0.into(),
                    width: 1.0,
                    color: PRIMARY,
                },
                shadow: elevation::level_2(),
                ..base
            },
            button::Status::Pressed => button::Style {
                background: Some(Background::Color(PRIMARY_PRESSED)),
                border: Border {
                    radius: 6.0.into(),
                    width: 1.0,
                    color: PRIMARY,
                },
                shadow: Shadow::default(),
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

pub fn custom_scrollable() -> impl Fn(&Theme, scrollable::Status) -> scrollable::Style {
    |_theme, status| {
        let scroller_color = match status {
            scrollable::Status::Active { .. } => MUTED,
            scrollable::Status::Hovered { .. } | scrollable::Status::Dragged { .. } => PRIMARY,
        };

        let rail = scrollable::Rail {
            background: Some(Background::Color(SURFACE)),
            border: Border {
                radius: 4.0.into(),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
            scroller: scrollable::Scroller {
                background: Background::Color(scroller_color),
                border: Border {
                    radius: 4.0.into(),
                    width: 0.0,
                    color: Color::TRANSPARENT,
                },
            },
        };

        scrollable::Style {
            container: container::Style {
                background: None,
                text_color: Some(TEXT),
                ..Default::default()
            },
            vertical_rail: rail,
            horizontal_rail: rail,
            gap: None,
            auto_scroll: scrollable::AutoScroll {
                background: Background::Color(SURFACE_HOVER),
                border: Border {
                    radius: 4.0.into(),
                    width: 1.0,
                    color: BORDER_SUBTLE,
                },
                shadow: elevation::level_1(),
                icon: TEXT,
            },
        }
    }
}

pub fn separator_container() -> impl Fn(&Theme) -> container::Style {
    |_theme| container::Style {
        background: Some(Background::Color(BORDER_SUBTLE)),
        ..Default::default()
    }
}
