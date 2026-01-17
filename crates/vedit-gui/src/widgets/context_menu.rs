use crate::message::Message;
use crate::style;
use iced::widget::{Space, button, column, container, row, text};
use iced::{Element, Length};
use iced_font_awesome::fa_icon_solid;

/// Render the editor context menu at the given position
pub fn render_context_menu(
    _x: f32,
    _y: f32,
    scale: f32,
    has_definition: bool,
    has_selection: bool,
) -> Element<'static, Message> {
    let item_padding = (6.0 * scale) as u16;
    let icon_size = 12.0 * scale;
    let text_size = 13.0 * scale;
    let menu_width = 200.0 * scale;

    let mut menu_items: Vec<Element<'static, Message>> = Vec::new();

    // Jump to Definition item (only shown when a symbol with definition is under cursor)
    if has_definition {
        let goto_item = {
            let icon_el = fa_icon_solid("arrow-right")
                .size(icon_size)
                .color(style::TEXT_SECONDARY);
            let label_el = text("Jump to Definition").size(text_size);
            let shortcut_el = text("F12").size(text_size * 0.85).color(style::MUTED);

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
                .on_press(Message::EditorContextMenuGotoDefinition)
        };
        menu_items.push(goto_item.into());

        // Separator after goto definition
        let separator = container(Space::new().width(Length::Fill).height(Length::Fixed(1.0)))
            .style(style::separator_container())
            .padding([4, 0]);
        menu_items.push(separator.into());
    }

    // Cut item (only shown when there's a selection)
    if has_selection {
        let cut_item = {
            let icon_el = fa_icon_solid("scissors")
                .size(icon_size)
                .color(style::TEXT_SECONDARY);
            let label_el = text("Cut").size(text_size);
            let shortcut_el = text("Ctrl+X").size(text_size * 0.85).color(style::MUTED);

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
                .on_press(Message::EditorContextMenuCut)
        };
        menu_items.push(cut_item.into());
    }

    // Copy item (only shown when there's a selection)
    if has_selection {
        let copy_item = {
            let icon_el = fa_icon_solid("copy")
                .size(icon_size)
                .color(style::TEXT_SECONDARY);
            let label_el = text("Copy").size(text_size);
            let shortcut_el = text("Ctrl+C").size(text_size * 0.85).color(style::MUTED);

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
                .on_press(Message::EditorContextMenuCopy)
        };
        menu_items.push(copy_item.into());
    }

    // Paste item (always available)
    let paste_item = {
        let icon_el = fa_icon_solid("paste")
            .size(icon_size)
            .color(style::TEXT_SECONDARY);
        let label_el = text("Paste").size(text_size);
        let shortcut_el = text("Ctrl+V").size(text_size * 0.85).color(style::MUTED);

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
            .on_press(Message::EditorContextMenuPaste)
    };
    menu_items.push(paste_item.into());

    // Separator before other items
    let separator = container(Space::new().width(Length::Fill).height(Length::Fixed(1.0)))
        .style(style::separator_container())
        .padding([4, 0]);
    menu_items.push(separator.into());

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
    menu_items.push(sticky_note_item.into());

    // Separator
    let separator = container(Space::new().width(Length::Fill).height(Length::Fixed(1.0)))
        .style(style::separator_container())
        .padding([4, 0]);
    menu_items.push(separator.into());

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
    menu_items.push(select_all_item.into());

    let menu = column(menu_items)
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
    has_definition: bool,
    has_selection: bool,
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
    let menu = render_context_menu(x, y, scale, has_definition, has_selection);

    // Position the menu using padding from top-left
    // Clamp position so menu stays on screen
    let menu_width = 200.0 * scale;
    // Calculate dynamic height based on visible items
    let mut item_count = 3; // Paste, Sticky Note, Select All (always shown)
    if has_definition {
        item_count += 1;
    }
    if has_selection {
        item_count += 2; // Cut and Copy
    }
    let menu_height = (30.0 * item_count as f32 + 20.0) * scale;

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
