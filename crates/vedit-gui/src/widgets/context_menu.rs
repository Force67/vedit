use crate::message::Message;
use crate::style;
use iced::widget::{Space, button, column, container, row, text};
use iced::{Element, Length};
use iced_font_awesome::fa_icon_solid;

/// Render the editor context menu at the given position
pub fn render_context_menu(_x: f32, _y: f32, scale: f32) -> Element<'static, Message> {
    let item_padding = (6.0 * scale) as u16;
    let icon_size = 12.0 * scale;
    let text_size = 13.0 * scale;
    let menu_width = 180.0 * scale;

    // Add Sticky Note item
    let sticky_note_item = {
        let icon_el = fa_icon_solid("note-sticky")
            .size(icon_size)
            .color(style::TEXT_SECONDARY);
        let label_el = text("Add Sticky Note").size(text_size);

        let content = row![icon_el, label_el]
            .spacing(8)
            .align_y(iced::Alignment::Center);

        button(content)
            .padding(item_padding)
            .width(Length::Fill)
            .style(style::tree_row_button(false))
            .on_press(Message::EditorContextMenuAddStickyNote)
    };

    // Select All item
    let select_all_item = {
        let icon_el = fa_icon_solid("object-group")
            .size(icon_size)
            .color(style::TEXT_SECONDARY);
        let label_el = text("Select All").size(text_size);
        let shortcut_el = text("Ctrl+A").size(text_size * 0.85).color(style::MUTED);

        let content = row![
            icon_el,
            label_el,
            Space::new().width(Length::Fill),
            shortcut_el
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center);

        button(content)
            .padding(item_padding)
            .width(Length::Fill)
            .style(style::tree_row_button(false))
            .on_press(Message::EditorContextMenuSelectAll)
    };

    // Separator
    let separator = container(Space::new().width(Length::Fill).height(Length::Fixed(1.0)))
        .style(style::separator_container())
        .padding([4, 0]);

    let menu = column![sticky_note_item, separator, select_all_item]
        .spacing(2)
        .width(Length::Fixed(menu_width));

    // Wrap in a styled container with shadow
    let menu_container = container(menu)
        .padding(4)
        .style(style::floating_panel_container());

    container(menu_container)
        .width(Length::Shrink)
        .height(Length::Shrink)
        .into()
}

/// Render an overlay that captures clicks outside the context menu to close it
pub fn render_context_menu_overlay(
    x: f32,
    y: f32,
    scale: f32,
    window_size: iced::Size,
) -> Element<'static, Message> {
    use iced::widget::stack;

    // Background overlay that closes menu when clicked
    let backdrop = iced::widget::mouse_area(
        container(Space::new().width(Length::Fill).height(Length::Fill))
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .on_press(Message::EditorContextMenuHide);

    // The actual menu
    let menu = render_context_menu(x, y, scale);

    // Position the menu using padding from top-left
    // Clamp position so menu stays on screen
    let menu_width = 180.0 * scale;
    let menu_height = 100.0 * scale; // Approximate height

    let clamped_x = x.min(window_size.width - menu_width - 10.0).max(0.0);
    let clamped_y = y.min(window_size.height - menu_height - 10.0).max(0.0);

    let positioned_menu = container(menu)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(iced::Padding {
            top: clamped_y,
            right: 0.0,
            bottom: 0.0,
            left: clamped_x,
        });

    stack![backdrop, positioned_menu].into()
}
