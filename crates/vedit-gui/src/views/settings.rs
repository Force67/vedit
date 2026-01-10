use crate::message::Message;
use crate::state::EditorState;
use crate::style::{active_document_button, document_button, panel_container};
use vedit_application::{SettingsCategory, SETTINGS_CATEGORIES};
use iced::widget::{button, column, container, Space, row, text, text_input};
use iced::{Alignment, Color, Element, Length, Padding};

pub fn render_settings(
    state: &EditorState,
    scale: f32,
    spacing_large: f32,
    spacing_medium: f32,
    spacing_small: f32,
) -> Element<'_, Message> {
    let mut categories_list = column![text("Categories").size((16.0 * scale).max(12.0))]
        .spacing(spacing_small);

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
        SettingsCategory::Keybindings =>
            render_keybindings_settings(state, scale, spacing_large, spacing_medium, spacing_small),
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