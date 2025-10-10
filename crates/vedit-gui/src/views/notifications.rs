use crate::message::Message;
use crate::notifications::{Notification, NotificationKind};
use crate::state::EditorState;
use crate::style::notification_container;
use iced::widget::{button, column, container, horizontal_space, text, row};
use iced::{theme, Alignment, Color, Element, Length, Padding};
use crate::style::NotificationTone;

pub fn render_notifications(
    state: &EditorState,
    scale: f32,
    spacing_large: f32,
    spacing_medium: f32,
) -> Element<'_, Message> {
    let bubble_spacing = (spacing_medium * 0.6).max(6.0);
    let mut stack = column![]
        .spacing(bubble_spacing)
        .align_items(Alignment::End);

    for notification in state.notifications() {
        stack = stack.push(render_notification_card(notification, scale));
    }

    let overlay = row![horizontal_space().width(Length::Fill), stack]
        .spacing(bubble_spacing)
        .align_items(Alignment::End);

    container(overlay)
        .width(Length::Fill)
        .height(Length::Shrink)
        .padding([0.0, spacing_large, spacing_large, spacing_large])
        .align_y(iced::alignment::Vertical::Bottom)
        .into()
}

fn render_notification_card(notification: &Notification, scale: f32) -> Element<'_, Message> {
    let tone = notification_tone(notification.kind);
    let accent = notification_accent(notification.kind);
    let padding = (12.0 * scale).max(8.0);
    let spacing = (10.0 * scale).max(6.0);
    let icon_size = (14.0 * scale).max(10.0);

    let icon = container(text("●").size(icon_size).style(accent))
    .width(Length::Fixed((icon_size + 4.0).max(12.0)))
    .center_x()
    .center_y();

    let mut body = column![
        text(&notification.title)
            .size((15.0 * scale).max(11.0))
            .style(Color::from_rgb8(240, 240, 240)),
    ]
    .spacing((4.0 * scale).max(2.0));

    if let Some(details) = notification.body() {
        body = body.push(
            text(details)
                .size((13.0 * scale).max(9.5))
                .style(Color::from_rgb8(190, 190, 190)),
        );
    }

    let close_button = button(text("✕").size((14.0 * scale).max(10.0)))
        .style(theme::Button::Text)
        .on_press(Message::NotificationDismissed(notification.id));

    let content = row![icon, body.width(Length::Fill), close_button]
        .spacing(spacing)
        .align_items(Alignment::Center);

    container(content)
        .padding(Padding::new(padding))
        .max_width((320.0 * scale).max(220.0))
        .style(notification_container(tone))
        .into()
}

fn notification_accent(kind: NotificationKind) -> Color {
    match kind {
        NotificationKind::Info => Color::from_rgb8(52, 152, 219),
        NotificationKind::Success => Color::from_rgb8(39, 174, 96),
        NotificationKind::Error => Color::from_rgb8(231, 76, 60),
    }
}

fn notification_tone(kind: NotificationKind) -> NotificationTone {
    match kind {
        NotificationKind::Info => NotificationTone::Info,
        NotificationKind::Success => NotificationTone::Success,
        NotificationKind::Error => NotificationTone::Error,
    }
}