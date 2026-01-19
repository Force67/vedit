//! Context menu for solutions and projects in the solutions view

use crate::message::{Message, SolutionContextTarget};
use crate::style;
use iced::widget::{Space, button, column, container, row, text};
use iced::{Element, Length};
use iced_font_awesome::fa_icon_solid;

/// Render the solution/project context menu
pub fn render_solution_context_menu(
    target: &SolutionContextTarget,
    scale: f32,
    has_wine_env: bool,
    available_configs: &[&str],
    selected_config: Option<&str>,
) -> Element<'static, Message> {
    let item_padding = (6.0 * scale) as u16;
    let icon_size = 12.0 * scale;
    let text_size = 13.0 * scale;
    let menu_width = 200.0 * scale;

    let mut menu_items: Vec<Element<'static, Message>> = Vec::new();

    let target_path = match target {
        SolutionContextTarget::Solution(p) => p.clone(),
        SolutionContextTarget::Project(p) => p.clone(),
    };

    let target_name = target_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Unknown")
        .to_string();

    // Header showing what we're operating on
    let header = text(target_name).size(text_size * 0.9).color(style::MUTED);
    menu_items.push(container(header).padding([4, 8]).width(Length::Fill).into());

    // Configuration selector section
    if !available_configs.is_empty() {
        menu_items.push(separator(scale));

        // Config header
        let config_header = row![
            fa_icon_solid("gear").size(icon_size).color(style::MUTED),
            text("Configuration")
                .size(text_size * 0.85)
                .color(style::MUTED),
        ]
        .spacing(6)
        .align_y(iced::Alignment::Center);
        menu_items.push(
            container(config_header)
                .padding([4, 8])
                .width(Length::Fill)
                .into(),
        );

        // Configuration options (limit to prevent menu from being too long)
        for config in available_configs.iter().take(6) {
            let is_selected = selected_config == Some(*config);
            let config_item =
                config_menu_item(config, is_selected, icon_size, text_size, item_padding);
            menu_items.push(config_item);
        }
    }

    // Separator
    menu_items.push(separator(scale));

    // Build item
    let build_item = menu_item(
        "hammer",
        "Build",
        Some(Message::SolutionContextMenuBuild(target_path.clone())),
        icon_size,
        text_size,
        item_padding,
        has_wine_env,
    );
    menu_items.push(build_item);

    // Rebuild item
    let rebuild_item = menu_item(
        "arrows-rotate",
        "Rebuild",
        Some(Message::SolutionContextMenuRebuild(target_path.clone())),
        icon_size,
        text_size,
        item_padding,
        has_wine_env,
    );
    menu_items.push(rebuild_item);

    // Clean item
    let clean_item = menu_item(
        "broom",
        "Clean",
        Some(Message::SolutionContextMenuClean(target_path.clone())),
        icon_size,
        text_size,
        item_padding,
        has_wine_env,
    );
    menu_items.push(clean_item);

    // Separator before debug
    menu_items.push(separator(scale));

    // Debug item (only if Wine environment is configured)
    let debug_item = menu_item(
        "bug",
        "Debug",
        if has_wine_env {
            Some(Message::SolutionContextMenuDebug(target_path.clone()))
        } else {
            None
        },
        icon_size,
        text_size,
        item_padding,
        true,
    );
    menu_items.push(debug_item);

    // Separator
    menu_items.push(separator(scale));

    // Open containing folder
    let folder_item = menu_item(
        "folder-open",
        "Open Folder",
        target_path
            .parent()
            .map(|p| Message::SolutionContextMenuOpenFolder(p.to_path_buf())),
        icon_size,
        text_size,
        item_padding,
        true,
    );
    menu_items.push(folder_item);

    // Separator
    menu_items.push(separator(scale));

    // Configure Wine/Proton
    let configure_item = menu_item(
        "wine-glass",
        "Configure Wine...",
        Some(Message::WineEnvironmentSettingsOpened),
        icon_size,
        text_size,
        item_padding,
        true,
    );
    menu_items.push(configure_item);

    let menu = column(menu_items)
        .spacing(2)
        .width(Length::Fixed(menu_width));

    // Wrap in a styled container
    let menu_container = container(menu)
        .padding(4)
        .style(style::floating_panel_container());

    container(menu_container)
        .width(Length::Shrink)
        .height(Length::Shrink)
        .into()
}

/// Render an overlay that captures clicks outside the context menu to close it
pub fn render_solution_context_menu_overlay(
    target: &SolutionContextTarget,
    x: f32,
    y: f32,
    scale: f32,
    window_size: iced::Size,
    has_wine_env: bool,
    available_configs: &[&str],
    selected_config: Option<&str>,
) -> Element<'static, Message> {
    use iced::widget::stack;

    // Background overlay that closes menu when clicked
    let backdrop = iced::widget::mouse_area(
        container(Space::new().width(Length::Fill).height(Length::Fill))
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .on_press(Message::SolutionContextMenuHide);

    // The actual menu
    let menu = render_solution_context_menu(
        target,
        scale,
        has_wine_env,
        available_configs,
        selected_config,
    );

    // Position the menu using padding from top-left
    // Clamp position so menu stays on screen
    let menu_width = 200.0 * scale;
    // Height depends on number of config items
    let base_height = 280.0 * scale;
    let config_height = (available_configs.len().min(6) as f32) * 28.0 * scale;
    let menu_height = base_height + config_height;

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

/// Create a menu item button
fn menu_item(
    icon: &'static str,
    label: &'static str,
    message: Option<Message>,
    icon_size: f32,
    text_size: f32,
    padding: u16,
    enabled: bool,
) -> Element<'static, Message> {
    let icon_color = if enabled && message.is_some() {
        style::TEXT_SECONDARY
    } else {
        style::MUTED
    };

    let text_color = if enabled && message.is_some() {
        style::TEXT
    } else {
        style::MUTED
    };

    let icon_el = fa_icon_solid(icon).size(icon_size).color(icon_color);

    let label_el = text(label).size(text_size).color(text_color);

    let content = row![icon_el, label_el]
        .spacing(8)
        .align_y(iced::Alignment::Center);

    let btn = button(content)
        .padding(padding)
        .width(Length::Fill)
        .style(style::tree_row_button(false));

    if let Some(msg) = message {
        if enabled {
            btn.on_press(msg).into()
        } else {
            btn.into()
        }
    } else {
        btn.into()
    }
}

/// Create a separator line
fn separator(scale: f32) -> Element<'static, Message> {
    container(Space::new().width(Length::Fill).height(Length::Fixed(1.0)))
        .style(style::separator_container())
        .padding([4.0 * scale, 0.0])
        .into()
}

/// Create a configuration selection item
fn config_menu_item(
    config: &str,
    is_selected: bool,
    icon_size: f32,
    text_size: f32,
    padding: u16,
) -> Element<'static, Message> {
    let icon_color = if is_selected {
        style::SUCCESS
    } else {
        style::SURFACE
    };

    let text_color = if is_selected {
        style::TEXT
    } else {
        style::TEXT_SECONDARY
    };

    // Show checkmark for selected config
    let icon_el = if is_selected {
        fa_icon_solid("check").size(icon_size).color(icon_color)
    } else {
        fa_icon_solid("circle")
            .size(icon_size * 0.5)
            .color(icon_color)
    };

    let label_el = text(config.to_string())
        .size(text_size * 0.95)
        .color(text_color);

    let content = row![icon_el, label_el]
        .spacing(8)
        .align_y(iced::Alignment::Center);

    let msg = Message::BuildConfigurationSelected(config.to_string());

    button(content)
        .padding(padding)
        .width(Length::Fill)
        .style(style::tree_row_button(is_selected))
        .on_press(msg)
        .into()
}
