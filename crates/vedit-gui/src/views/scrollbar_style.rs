use iced::widget::slider;
use iced::{Border, Theme};

pub fn editor_scrollbar_style() -> impl Fn(&Theme, slider::Status) -> slider::Style {
    |theme, status| {
        let palette = theme.extended_palette();
        let base = slider::Style {
            rail: slider::Rail {
                backgrounds: (
                    palette.background.base.color.into(),
                    palette.background.base.color.into(),
                ),
                width: 4.0,
                border: Border {
                    radius: 2.0.into(),
                    width: 0.0,
                    color: iced::Color::TRANSPARENT,
                },
            },
            handle: slider::Handle {
                shape: slider::HandleShape::Circle { radius: 5.0 },
                background: palette.primary.weak.color.into(),
                border_color: palette.primary.strong.color,
                border_width: 1.0,
            },
        };

        match status {
            slider::Status::Active => base,
            slider::Status::Hovered => slider::Style {
                handle: slider::Handle {
                    background: palette.primary.base.color.into(),
                    ..base.handle
                },
                ..base
            },
            slider::Status::Dragged => slider::Style {
                handle: slider::Handle {
                    background: palette.primary.strong.color.into(),
                    ..base.handle
                },
                ..base
            },
        }
    }
}
