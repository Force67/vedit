pub mod command_palette;
pub mod console_panel;
pub mod document_tabs;
pub mod editor_content;
pub mod notifications;
pub mod open_files;
pub mod scrollbar_style;
pub mod settings;
pub mod solutions;
pub mod status_bar;
pub mod title_bar;

use crate::message::Message;
use crate::state::EditorState;
use crate::style::root_container;
use crate::views::{
    command_palette::render_command_palette_contents, editor_content::render_editor_content,
    notifications::render_notifications, settings::render_settings, status_bar::render_status_bar,
    title_bar::render_title_bar,
};
use crate::widgets::context_menu::render_context_menu_overlay;
use crate::widgets::debugger;
use iced::widget::{column, container, stack};
use iced::{Alignment, Element, Length};

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
        layout.push(render_settings(
            state,
            scale,
            spacing_large,
            spacing_medium,
            spacing_small,
        ))
    } else {
        layout
            .push(render_editor_content(
                state,
                scale,
                spacing_large,
                spacing_medium,
                spacing_small,
            ))
            .push(render_status_bar(
                state,
                scale,
                spacing_small,
                spacing_large,
            ))
    };

    if state.has_notifications() {
        main_element = main_element.push(render_notifications(
            state,
            scale,
            spacing_large,
            spacing_medium,
        ));
    }

    let main_content = container(
        main_element
            .spacing(spacing_large)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Alignment::Start),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .style(root_container());

    let base: Element<'_, Message> = main_content.into();

    // Build a stack of overlays
    let mut layers: Vec<Element<'_, Message>> = vec![base];

    // Overlay the search dialog on top without dimming
    if state.search_dialog().is_visible {
        let search_contents = state.search_dialog().view(scale);
        let search_overlay: Element<'_, Message> = container(search_contents)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::alignment::Horizontal::Right)
            .align_y(iced::alignment::Vertical::Top)
            .into();
        layers.push(search_overlay);
    }

    // Overlay the command prompt on top without dimming
    if state.command_palette().is_open() {
        let contents = render_command_palette_contents(state);
        let palette_overlay: Element<'_, Message> = container(contents)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into();
        layers.push(palette_overlay);
    }

    // Overlay the editor context menu
    if state.context_menu_visible() {
        let (x, y) = state.context_menu_position();
        let has_definition = state.context_menu_definition().is_some();
        let has_selection = state.has_selection();
        let context_menu = render_context_menu_overlay(
            x,
            y,
            scale,
            state.current_window_size,
            has_definition,
            has_selection,
        );
        layers.push(context_menu);
    }

    stack(layers).into()
}
