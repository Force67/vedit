use iced::widget::slider;
use iced::Theme;

pub struct EditorScrollbarStyle;

impl slider::StyleSheet for EditorScrollbarStyle {
    type Style = Theme;

    fn active(&self, theme: &Self::Style) -> slider::Appearance {
        let palette = theme.extended_palette();
        slider::Appearance {
            rail: slider::Rail {
                colors: (
                    palette.background.base.color,
                    palette.background.base.color,
                ),
                width: 4.0,
                border_radius: 2.0.into(),
            },
            handle: slider::Handle {
                shape: slider::HandleShape::Circle { radius: 5.0 },
                color: palette.primary.weak.color,
                border_color: palette.primary.strong.color,
                border_width: 1.0,
            },
        }
    }

    fn hovered(&self, theme: &Self::Style) -> slider::Appearance {
        let mut active = self.active(theme);
        active.handle.color = theme.extended_palette().primary.base.color;
        active
    }

    fn dragging(&self, theme: &Self::Style) -> slider::Appearance {
        let mut active = self.active(theme);
        active.handle.color = theme.extended_palette().primary.strong.color;
        active
    }
}