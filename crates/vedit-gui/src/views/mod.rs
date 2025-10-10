pub mod command_palette;
pub mod console_panel;
pub mod editor_content;
pub mod notifications;
pub mod open_files;
pub mod settings;
pub mod solutions;
pub mod status_bar;
pub mod title_bar;
pub mod scrollbar_style;

use crate::message::Message;
use crate::state::EditorState;
use crate::style::root_container;
use crate::widgets::debugger;
use crate::views::{
    command_palette::render_command_palette_contents,
    editor_content::render_editor_content,
    notifications::render_notifications,
    settings::render_settings,
    status_bar::render_status_bar,
    title_bar::render_title_bar,
};
use iced::widget::{column, container};
use iced::{Alignment, Element, Length};
use iced_aw::Modal;

pub fn view(state: &EditorState) -> Element<'_, Message> {
    let scale = state.scale_factor() as f32;
    let spacing_large = (16.0 * scale).max(8.0);
    let spacing_medium = (12.0 * scale).max(6.0);
    let spacing_small = (8.0 * scale).max(4.0);

    let title_bar = render_title_bar(state, scale, spacing_large, spacing_medium, spacing_small);

    let mut layout = column![title_bar];

    if state.debugger_menu_open() {
        layout = layout.push(debugger::menu(
            state.debugger(),
            scale,
            spacing_large,
            spacing_medium,
            spacing_small,
        ));
    }

    let mut main_element = if state.settings().is_open() {
        layout.push(render_settings(state, scale, spacing_large, spacing_medium, spacing_small))
    } else {
        layout
            .push(render_editor_content(
                state,
                scale,
                spacing_large,
                spacing_medium,
                spacing_small,
            ))
            .push(render_status_bar(state, scale, spacing_small, spacing_large))
    };

    if state.has_notifications() {
        main_element = main_element.push(render_notifications(state, scale, spacing_large, spacing_medium));
    }

    let main_content = container(
        main_element
            .spacing(spacing_large)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_items(Alignment::Start),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x()
    .center_y()
    .style(root_container());

    let base = main_content.into();

    // Overlay the command prompt on top without dimming
    if state.command_palette().is_open() {
        Modal::new(
            base,
            {
                // center the dropdown inside the modal
                let contents = render_command_palette_contents(state);
                container(contents)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center_x()
                    .center_y()
                    .into()
            },
        )
        .into()
    } else {
        base
    }
}