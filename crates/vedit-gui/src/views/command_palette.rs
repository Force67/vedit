use crate::message::Message;
use crate::state::EditorState;
use crate::style::{floating_panel_container, panel_container};
use iced::widget::{button, column, container, horizontal_space, row, scrollable, text, text_input};
use iced::{theme, Alignment, Color, Element, Length};
use iced_aw::Modal;
use iced_font_awesome::fa_icon_solid;

pub fn render_command_palette_contents(state: &EditorState) -> Element<'_, Message> {
    let palette = state.command_palette();
    let commands = state.quick_commands();
    let filtered = palette.filtered_indices(commands);
    let selection = palette.selection_index();
    let scale = state.scale_factor() as f32;
    let spacing_large = (16.0 * scale).max(8.0);
    let spacing_medium = (12.0 * scale).max(6.0);
    let spacing_small = (8.0 * scale).max(4.0);
    let drop_width = (600.0 * scale).clamp(400.0, 800.0);

    let submit_message = state
        .selected_quick_command()
        .map(Message::CommandPaletteCommandInvoked)
        .unwrap_or(Message::CommandPaletteClosed);

    let input = text_input("Type a command…", palette.query())
        .on_input(Message::CommandPaletteInputChanged)
        .on_submit(submit_message)
        .padding(spacing_small)
        .size((16.0 * scale).max(12.0))
        .width(Length::Fill);

    let mut command_list = column![]
        .spacing(spacing_small)
        .width(Length::Fill);

    if filtered.is_empty() {
        command_list = command_list.push(
            container(text("No commands match your search").size((14.0 * scale).max(10.0)))
                .padding(spacing_small)
                .width(Length::Fill)
                .style(panel_container()),
        );
    } else {
        let window_size = 6;
        let half_window = window_size / 2;
        let start = selection.saturating_sub(half_window);
        let end = (start + window_size).min(filtered.len());
        let adjusted_start = if end - start < window_size && start > 0 {
            start.saturating_sub(window_size - (end - start))
        } else {
            start
        };

        for i in adjusted_start..end {
            if let Some(index) = filtered.get(i) {
                if let Some(command) = commands.get(*index) {
                    let label = column![
                        text(command.title).size((16.0 * scale).max(12.0)),
                        text(command.description).size((12.0 * scale).max(9.0)),
                    ]
                    .spacing(spacing_small / 2.0)
                    .width(Length::Fill);

                    let mut entry = button(label)
                        .padding(spacing_small)
                        .width(Length::Fill)
                        .on_press(Message::CommandPaletteCommandInvoked(command.id));

                    if i == selection {
                        entry = entry.style(theme::Button::Primary);
                    } else {
                        entry = entry.style(theme::Button::Text);
                    }

                    command_list = command_list.push(entry);
                }
            }
        }
    }

    let header = row![
        text("Command Prompt").size((18.0 * scale).max(14.0)),
        horizontal_space().width(Length::Fill),
        button(fa_icon_solid("xmark").size((16.0 * scale).max(12.0)).color(iced::Color::WHITE))
            .style(theme::Button::Text)
            .on_press(Message::CommandPaletteClosed),
    ]
    .spacing(spacing_small)
    .align_items(Alignment::Center);

    let palette_column = column![
        header,
        input,
        scrollable(command_list)
            .height(Length::Fixed(240.0 * scale))
            .style(crate::style::custom_scrollable()),
    ]
    .spacing(spacing_medium)
    .width(Length::Fill);

    let scale = state.scale_factor() as f32;
      let spacing_large = (16.0 * scale).max(8.0);
      let drop_width = (600.0 * scale).clamp(400.0, 800.0);

      let dropdown = container(palette_column)
          .padding(spacing_large)
          .width(Length::Fixed(drop_width))
          .style(floating_panel_container());

      // return only the dropdown; no Fill×Fill wrapper here
      dropdown.into()
}