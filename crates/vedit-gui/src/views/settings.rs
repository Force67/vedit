use crate::message::Message;
use crate::state::EditorState;
use crate::style::{active_document_button, document_button, panel_container};
use iced::widget::{Space, button, column, container, row, text, text_input};
use iced::{Alignment, Color, Element, Length, Padding};
use vedit_application::{SETTINGS_CATEGORIES, SettingsCategory};

pub fn render_settings(
    state: &EditorState,
    scale: f32,
    spacing_large: f32,
    spacing_medium: f32,
    spacing_small: f32,
) -> Element<'_, Message> {
    let mut categories_list =
        column![text("Categories").size((16.0 * scale).max(12.0))].spacing(spacing_small);

    for category in SETTINGS_CATEGORIES.iter().copied() {
        let label = category.label();
        let mut entry = button(text(label).size((14.0 * scale).max(10.0)))
            .style(document_button())
            .width(Length::Fill)
            .on_press(Message::SettingsCategorySelected(category));

        if category == state.settings().selected_category() {
            entry = entry.style(active_document_button());
        }

        categories_list = categories_list.push(entry);
    }

    let categories_panel = container(categories_list)
        .padding(spacing_large)
        .width(Length::Fixed((220.0 * scale).max(160.0)))
        .style(panel_container());

    let detail: Element<'_, Message> = match state.settings().selected_category() {
        SettingsCategory::Keybindings => {
            render_keybindings_settings(state, scale, spacing_large, spacing_medium, spacing_small)
        }
        SettingsCategory::Wine => {
            render_wine_settings(state, scale, spacing_large, spacing_medium, spacing_small)
        }
    };

    row![categories_panel, detail]
        .spacing(spacing_large)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn render_keybindings_settings(
    state: &EditorState,
    scale: f32,
    spacing_large: f32,
    spacing_medium: f32,
    spacing_small: f32,
) -> Element<'_, Message> {
    let mut content = column![
        row![
            text("Quick Command Shortcuts").size((16.0 * scale).max(12.0)),
            Space::new().width(Length::Fill).width(Length::Fill),
            {
                let button_label = text("Save Keybindings").size((14.0 * scale).max(10.0));
                let base = button(button_label);
                if state.settings_dirty() {
                    base.on_press(Message::SettingsBindingsSaveRequested)
                } else {
                    base
                }
            },
        ]
        .spacing(spacing_small)
        .align_y(Alignment::Center),
        text("Assign keyboard shortcuts to launch quick actions directly.")
            .size((14.0 * scale).max(10.0)),
    ]
    .spacing(spacing_small);

    let keymap_path = state
        .keymap_path_display()
        .unwrap_or_else(|| "(default: ./keybindings.toml)".to_string());

    content = content.push(
        row![
            text(format!("Keymap file: {}", keymap_path)).size((13.0 * scale).max(9.0)),
            Space::new().width(Length::Fill).width(Length::Fill),
            button(text("Change File…").size((13.0 * scale).max(9.0)))
                .on_press(Message::SettingsKeymapPathRequested),
        ]
        .spacing(spacing_small)
        .align_y(Alignment::Center),
    );

    if let Some(notice) = state.settings_notice() {
        content = content.push(
            text(notice)
                .size((13.0 * scale).max(9.0))
                .color(Color::from_rgb8(38, 139, 210)),
        );
    }

    if let Some(err) = state.settings_error() {
        content = content.push(
            text(err)
                .size((13.0 * scale).max(9.0))
                .color(Color::from_rgb8(220, 50, 47)),
        );
    }

    for command in state
        .quick_commands()
        .iter()
        .filter(|cmd| cmd.action.is_some())
    {
        let id = command.id;
        let binding_value = state.settings().binding_input(id);
        let field = text_input("e.g. Ctrl+Alt+K", binding_value)
            .padding(Padding::new((4.0 * scale).max(2.0)))
            .on_input(move |value| Message::SettingsBindingChanged(id, value))
            .on_submit(Message::SettingsBindingApplied(id))
            .width(Length::FillPortion(2));

        let apply_button = button(text("Assign").size((14.0 * scale).max(10.0)))
            .on_press(Message::SettingsBindingApplied(id));

        let mut entry = column![
            text(command.title).size((14.0 * scale).max(10.0)),
            text(command.description)
                .size((12.0 * scale).max(9.0))
                .color(Color::from_rgb8(170, 170, 170)),
            row![field, apply_button]
                .spacing(spacing_small)
                .align_y(Alignment::Center),
        ]
        .spacing(spacing_small)
        .padding(Padding::new(spacing_small).right(0.0).left(0.0));

        if let Some(err) = state.settings().binding_error(id) {
            entry = entry.push(
                text(err)
                    .size((12.0 * scale).max(9.0))
                    .color(Color::from_rgb8(220, 50, 47)),
            );
        }

        content = content.push(entry);
    }

    container(content.spacing(spacing_medium))
        .padding(spacing_large)
        .width(Length::Fill)
        .style(panel_container())
        .into()
}

fn render_wine_settings(
    state: &EditorState,
    scale: f32,
    spacing_large: f32,
    spacing_medium: f32,
    spacing_small: f32,
) -> Element<'_, Message> {
    let discovery = state.wine_discovery();
    let prefix_manager = state.wine_prefix_manager();

    let mut content = column![
        text("Wine / Proton Configuration").size((16.0 * scale).max(12.0)),
        text("Configure Wine prefixes for building and debugging Windows applications.")
            .size((14.0 * scale).max(10.0))
            .color(Color::from_rgb8(170, 170, 170)),
    ]
    .spacing(spacing_small);

    // NixOS warning if steam-run is not available
    if vedit_wine::is_nixos() && !vedit_wine::has_steam_run() {
        content = content.push(
            container(
                row![
                    text("NixOS detected: ").size((13.0 * scale).max(9.0)).color(Color::from_rgb8(251, 191, 36)),
                    text("steam-run is required for Wine to work. Add steam-run to your NixOS config or run: ")
                        .size((13.0 * scale).max(9.0)),
                    text("nix-shell -p steam-run")
                        .size((13.0 * scale).max(9.0))
                        .color(Color::from_rgb8(129, 199, 132)),
                ]
            )
            .padding(spacing_small)
            .style(|_theme| container::Style {
                background: Some(iced::Background::Color(Color::from_rgb8(50, 40, 20))),
                border: iced::Border {
                    color: Color::from_rgb8(251, 191, 36),
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..Default::default()
            })
        );
    } else if vedit_wine::is_nixos() {
        // NixOS with steam-run available
        content = content.push(
            container(
                text("NixOS detected: steam-run is available. Wine commands will run in FHS-compatible environment.")
                    .size((13.0 * scale).max(9.0))
                    .color(Color::from_rgb8(129, 199, 132))
            )
            .padding(spacing_small)
            .style(|_theme| container::Style {
                background: Some(iced::Background::Color(Color::from_rgb8(30, 45, 30))),
                border: iced::Border {
                    color: Color::from_rgb8(129, 199, 132),
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..Default::default()
            })
        );
    }

    // Configured Prefixes section
    content = content.push(Space::new().height(Length::Fixed(spacing_large)));
    content = content.push(
        row![
            text("Wine Prefixes").size((15.0 * scale).max(11.0)),
            Space::new().width(Length::Fill),
            button(text("+ Create Prefix").size((13.0 * scale).max(9.0)))
                .on_press(Message::WinePrefixCreateStart),
        ]
        .align_y(Alignment::Center),
    );

    if prefix_manager.prefixes.is_empty() {
        content = content.push(
            text("No Wine prefixes configured. Create one to build VS solutions.")
                .size((13.0 * scale).max(9.0))
                .color(Color::from_rgb8(170, 170, 170)),
        );
    } else {
        let mut prefix_list = column![].spacing(spacing_small);

        for (i, prefix) in prefix_manager.prefixes.iter().enumerate() {
            let is_selected = prefix_manager.selected == Some(i);
            let bg_color = if is_selected {
                Color::from_rgb8(45, 55, 72)
            } else {
                Color::from_rgb8(30, 35, 45)
            };

            let build_tools_status = if prefix.has_build_tools {
                ("✓ Build Tools", Color::from_rgb8(42, 161, 152))
            } else {
                ("✗ No Build Tools", Color::from_rgb8(220, 50, 47))
            };

            let mut action_buttons = row![].spacing(spacing_small);

            // Add "Install Build Tools" button if not installed
            if !prefix.has_build_tools {
                action_buttons = action_buttons.push(
                    button(text("Install Build Tools").size((11.0 * scale).max(8.0)))
                        .on_press(Message::VsBuildToolsInstallStart(i)),
                );
            }

            action_buttons = action_buttons.push(
                button(text("Delete").size((11.0 * scale).max(8.0)))
                    .on_press(Message::WinePrefixDelete(i)),
            );

            let prefix_row = button(
                row![
                    column![
                        text(&prefix.name).size((14.0 * scale).max(10.0)),
                        text(format!("{} | {}", prefix.arch, prefix.path.display()))
                            .size((11.0 * scale).max(8.0))
                            .color(Color::from_rgb8(140, 140, 140)),
                        text(build_tools_status.0)
                            .size((11.0 * scale).max(8.0))
                            .color(build_tools_status.1),
                    ],
                    Space::new().width(Length::Fill),
                    action_buttons,
                ]
                .align_y(Alignment::Center)
                .padding(spacing_small),
            )
            .on_press(Message::WinePrefixSelected(i))
            .width(Length::Fill)
            .style(move |_theme, _status| button::Style {
                background: Some(iced::Background::Color(bg_color)),
                text_color: Color::WHITE,
                border: iced::Border {
                    color: if is_selected {
                        Color::from_rgb8(66, 153, 225)
                    } else {
                        Color::TRANSPARENT
                    },
                    width: if is_selected { 1.0 } else { 0.0 },
                    radius: 4.0.into(),
                },
                ..Default::default()
            });

            prefix_list = prefix_list.push(prefix_row);
        }

        content = content.push(prefix_list);
    }

    // Create prefix form (if open)
    if state.wine_create_prefix_open() {
        content = content.push(Space::new().height(Length::Fixed(spacing_large)));
        content = content.push(
            container(render_create_prefix_form(
                state,
                scale,
                spacing_medium,
                spacing_small,
            ))
            .padding(spacing_medium)
            .style(|_theme| container::Style {
                background: Some(iced::Background::Color(Color::from_rgb8(40, 45, 55))),
                border: iced::Border {
                    color: Color::from_rgb8(66, 153, 225),
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..Default::default()
            }),
        );
    }

    // Detection section
    content = content.push(Space::new().height(Length::Fixed(spacing_large)));
    content = content.push(text("Environment Detection").size((15.0 * scale).max(11.0)));

    content = content.push(
        row![
            button(text("Detect Wine/Proton").size((14.0 * scale).max(10.0)))
                .on_press(Message::WineEnvironmentDiscoveryRequested),
            Space::new().width(Length::Fixed(spacing_medium)),
            text("Scan for installed Wine and Proton environments")
                .size((13.0 * scale).max(9.0))
                .color(Color::from_rgb8(170, 170, 170)),
        ]
        .align_y(Alignment::Center),
    );

    // Show discovered environments
    if let Some(disc) = discovery {
        let mut env_list = column![].spacing(spacing_small / 2.0);

        if let Some(wine_path) = &disc.system_wine {
            env_list = env_list.push(
                text(format!("• System Wine: {}", wine_path.display()))
                    .size((12.0 * scale).max(9.0))
                    .color(Color::from_rgb8(140, 140, 140)),
            );
        }

        for proton in &disc.proton_installations {
            env_list = env_list.push(
                text(format!("• {}: {}", proton.name, proton.path.display()))
                    .size((12.0 * scale).max(9.0))
                    .color(Color::from_rgb8(140, 140, 140)),
            );
        }

        if disc.system_wine.is_none() && disc.proton_installations.is_empty() {
            env_list = env_list.push(
                text("No Wine or Proton found")
                    .size((12.0 * scale).max(9.0))
                    .color(Color::from_rgb8(220, 50, 47)),
            );
        }

        content = content.push(env_list);
    }

    // Info section
    content = content.push(Space::new().height(Length::Fixed(spacing_large)));
    content = content.push(text("Setup Instructions").size((15.0 * scale).max(11.0)));

    content = content.push(
        column![
            text("1. Click 'Detect Wine/Proton' to find installations")
                .size((13.0 * scale).max(9.0))
                .color(Color::from_rgb8(170, 170, 170)),
            text("2. Click '+ Create Prefix' to create a new Wine prefix")
                .size((13.0 * scale).max(9.0))
                .color(Color::from_rgb8(170, 170, 170)),
            text("3. Install VS Build Tools in the prefix using winetricks or manually")
                .size((13.0 * scale).max(9.0))
                .color(Color::from_rgb8(170, 170, 170)),
            text("4. Select the prefix and right-click solutions to build")
                .size((13.0 * scale).max(9.0))
                .color(Color::from_rgb8(170, 170, 170)),
        ]
        .spacing(spacing_small / 2.0),
    );

    container(content.spacing(spacing_medium))
        .padding(spacing_large)
        .width(Length::Fill)
        .style(panel_container())
        .into()
}

fn render_create_prefix_form(
    state: &EditorState,
    scale: f32,
    spacing_medium: f32,
    spacing_small: f32,
) -> Element<'_, Message> {
    let discovery = state.wine_discovery();

    let mut form = column![text("Create New Wine Prefix").size((15.0 * scale).max(11.0)),]
        .spacing(spacing_small);

    // Name input
    form = form.push(
        column![
            text("Prefix Name:").size((13.0 * scale).max(9.0)),
            text_input("e.g., vs-build-tools", state.wine_create_prefix_name())
                .on_input(Message::WinePrefixNameChanged)
                .padding(Padding::new((6.0 * scale).max(4.0)))
                .width(Length::Fill),
        ]
        .spacing(spacing_small / 2.0),
    );

    // Wine binary selection
    form = form.push(
        column![text("Wine Binary:").size((13.0 * scale).max(9.0)),].spacing(spacing_small / 2.0),
    );

    if let Some(disc) = discovery {
        let mut wine_options = column![].spacing(spacing_small / 2.0);
        let mut index = 0usize;

        // System Wine option
        if disc.system_wine.is_some() {
            let is_selected = state.wine_create_prefix_wine_index() == Some(index);
            let idx = index;
            wine_options = wine_options.push(
                button(
                    row![
                        text(if is_selected { "●" } else { "○" }).size((14.0 * scale).max(10.0)),
                        Space::new().width(Length::Fixed(spacing_small)),
                        text("System Wine").size((13.0 * scale).max(9.0)),
                    ]
                    .align_y(Alignment::Center),
                )
                .on_press(Message::WinePrefixWineBinarySelected(idx))
                .style(|_theme, _status| button::Style {
                    background: None,
                    text_color: Color::WHITE,
                    ..Default::default()
                }),
            );
            index += 1;
        }

        // Proton options
        for proton in &disc.proton_installations {
            let is_selected = state.wine_create_prefix_wine_index() == Some(index);
            let idx = index;
            let name = proton.name.clone();
            wine_options = wine_options.push(
                button(
                    row![
                        text(if is_selected { "●" } else { "○" }).size((14.0 * scale).max(10.0)),
                        Space::new().width(Length::Fixed(spacing_small)),
                        text(name).size((13.0 * scale).max(9.0)),
                    ]
                    .align_y(Alignment::Center),
                )
                .on_press(Message::WinePrefixWineBinarySelected(idx))
                .style(|_theme, _status| button::Style {
                    background: None,
                    text_color: Color::WHITE,
                    ..Default::default()
                }),
            );
            index += 1;
        }

        form = form.push(wine_options);
    } else {
        form = form.push(
            text("Run 'Detect Wine/Proton' first to see available options")
                .size((12.0 * scale).max(9.0))
                .color(Color::from_rgb8(220, 50, 47)),
        );
    }

    // Buttons
    let can_create = !state.wine_create_prefix_name().is_empty()
        && state.wine_create_prefix_wine_index().is_some();

    form = form.push(Space::new().height(Length::Fixed(spacing_medium)));
    form = form.push(
        row![
            button(text("Cancel").size((13.0 * scale).max(9.0)))
                .on_press(Message::WinePrefixCancelCreate),
            Space::new().width(Length::Fill),
            {
                let btn = button(text("Create Prefix").size((13.0 * scale).max(9.0)));
                if can_create {
                    btn.on_press(Message::WinePrefixCreateConfirm)
                } else {
                    btn
                }
            },
        ]
        .align_y(Alignment::Center),
    );

    form.into()
}
