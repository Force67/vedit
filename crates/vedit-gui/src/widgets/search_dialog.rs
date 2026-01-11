// SearchDialog has some helper methods not yet used by the main app
#![allow(dead_code)]

use crate::message::Message;
use crate::style::{self, floating_panel_container};
use iced::widget::{Space, button, column, container, row, text, text_input};
use iced::{Alignment, Element, Length, Padding};
use iced_font_awesome::fa_icon_solid;

#[derive(Debug, Clone)]
pub struct SearchDialog {
    pub is_visible: bool,
    pub search_query: String,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub use_regex: bool,
    pub current_match: Option<usize>,
    pub total_matches: usize,
    pub replace_mode: bool,
    pub replace_text: String,
    pub search_state: SearchState,
    pub pending_search: bool,
    pub search_input_id: iced::widget::Id,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SearchState {
    Idle,
    Searching,
    Complete,
    NoMatches,
}

impl Default for SearchDialog {
    fn default() -> Self {
        Self {
            is_visible: false,
            search_query: String::new(),
            case_sensitive: false,
            whole_word: false,
            use_regex: false,
            current_match: None,
            total_matches: 0,
            replace_mode: false,
            replace_text: String::new(),
            search_state: SearchState::Idle,
            pending_search: false,
            search_input_id: iced::widget::Id::unique(),
        }
    }
}

impl SearchDialog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_search_input_id(&self) -> iced::widget::Id {
        self.search_input_id.clone()
    }

    pub fn show(&mut self) {
        self.is_visible = true;
    }

    pub fn hide(&mut self) {
        self.is_visible = false;
    }

    pub fn toggle(&mut self) {
        self.is_visible = !self.is_visible;
    }

    pub fn toggle_replace(&mut self) {
        self.replace_mode = !self.replace_mode;
    }

    // Methods required by the app.rs and state.rs
    pub fn set_search_query(&mut self, query: String) {
        self.search_query = query;
    }

    pub fn set_search_state(&mut self, state: SearchState) {
        self.search_state = state;
    }

    pub fn set_matches(&mut self, current: Option<usize>, total: usize) {
        self.current_match = current;
        self.total_matches = total;
    }

    pub fn start_search(&mut self) {
        self.search_state = SearchState::Searching;
        self.pending_search = true;
    }

    pub fn complete_search(&mut self, total_matches: usize) {
        self.search_state = if total_matches == 0 {
            SearchState::NoMatches
        } else {
            SearchState::Complete
        };
        self.total_matches = total_matches;
        self.current_match = if total_matches > 0 { Some(0) } else { None };
        self.pending_search = false;
    }

    pub fn set_case_sensitive(&mut self, case_sensitive: bool) {
        self.case_sensitive = case_sensitive;
    }

    pub fn set_whole_word(&mut self, whole_word: bool) {
        self.whole_word = whole_word;
    }

    pub fn set_use_regex(&mut self, use_regex: bool) {
        self.use_regex = use_regex;
    }

    pub fn enable_replace_mode(&mut self) {
        self.replace_mode = true;
    }

    pub fn disable_replace_mode(&mut self) {
        self.replace_mode = false;
    }

    pub fn set_replace_text(&mut self, text: String) {
        self.replace_text = text;
    }

    pub fn view(&self, scale: f32) -> Element<'_, Message> {
        let spacing = (6.0 * scale).max(4.0);
        let text_size = (12.0 * scale).max(10.0);
        let icon_size = (11.0 * scale).max(9.0);

        if !self.is_visible {
            return container(text("")).into();
        }

        // Header with title, match count, and close
        let match_count_text = match self.search_state {
            SearchState::Complete if self.total_matches > 0 => match self.current_match {
                Some(current) => format!("{}/{}", current + 1, self.total_matches),
                None => format!("{} found", self.total_matches),
            },
            SearchState::NoMatches => "No results".to_string(),
            SearchState::Searching => "...".to_string(),
            _ => String::new(),
        };

        let match_color = match self.search_state {
            SearchState::Complete if self.total_matches > 0 => style::SUCCESS,
            SearchState::NoMatches => style::ERROR,
            _ => style::MUTED,
        };

        let close_button = button(fa_icon_solid("xmark").size(icon_size).color(style::MUTED))
            .style(style::close_button())
            .padding(Padding::from([2, 4]))
            .on_press(Message::SearchClose);

        let header = row![
            fa_icon_solid("magnifying-glass")
                .size(icon_size)
                .color(style::PRIMARY),
            text("Find").size(text_size).color(style::TEXT),
            Space::new().width(Length::Fill),
            text(match_count_text).size(text_size).color(match_color),
            close_button,
        ]
        .spacing(spacing)
        .align_y(Alignment::Center);

        // Search input with nav buttons
        let search_input = text_input("Search...", &self.search_query)
            .id(self.search_input_id.clone())
            .on_input(Message::SearchQueryChanged)
            .on_paste(Message::SearchQueryChanged)
            .on_submit(Message::SearchExecute)
            .size(text_size)
            .padding(Padding::from([4, 8]));

        let nav_buttons = row![
            button(
                fa_icon_solid("chevron-up")
                    .size(icon_size)
                    .color(style::TEXT_SECONDARY)
            )
            .style(style::chevron_button())
            .padding(Padding::from([4, 6]))
            .on_press(Message::SearchPrevious),
            button(
                fa_icon_solid("chevron-down")
                    .size(icon_size)
                    .color(style::TEXT_SECONDARY)
            )
            .style(style::chevron_button())
            .padding(Padding::from([4, 6]))
            .on_press(Message::SearchNext),
        ]
        .spacing(2);

        let search_row = row![search_input, nav_buttons]
            .spacing(spacing)
            .align_y(Alignment::Center);

        // Options as toggle buttons (more compact than checkboxes)
        let options_row = row![
            button(text("Aa").size(text_size))
                .style(style::search_toggle(self.case_sensitive))
                .padding(Padding::from([3, 8]))
                .on_press(Message::SearchCaseSensitive(!self.case_sensitive)),
            button(text("W").size(text_size))
                .style(style::search_toggle(self.whole_word))
                .padding(Padding::from([3, 8]))
                .on_press(Message::SearchWholeWord(!self.whole_word)),
            button(text(".*").size(text_size))
                .style(style::search_toggle(self.use_regex))
                .padding(Padding::from([3, 8]))
                .on_press(Message::SearchUseRegex(!self.use_regex)),
            Space::new().width(Length::Fill),
            button(
                row![
                    fa_icon_solid("arrow-right-arrow-left")
                        .size(icon_size)
                        .color(style::TEXT_SECONDARY),
                    text("Replace").size(text_size).color(style::TEXT_SECONDARY),
                ]
                .spacing(4)
                .align_y(Alignment::Center)
            )
            .style(style::chevron_button())
            .padding(Padding::from([3, 6]))
            .on_press(Message::SearchToggleReplace),
        ]
        .spacing(4)
        .align_y(Alignment::Center);

        // Replace row (if enabled)
        let mut content = column![header, search_row, options_row]
            .spacing(spacing)
            .width(Length::Fill);

        if self.replace_mode {
            let replace_input = text_input("Replace with...", &self.replace_text)
                .on_input(Message::ReplaceTextChanged)
                .on_paste(Message::ReplaceTextChanged)
                .size(text_size)
                .padding(Padding::from([4, 8]));

            let replace_buttons = row![
                button(text("Replace").size(text_size))
                    .style(style::chevron_button())
                    .padding(Padding::from([4, 8]))
                    .on_press(Message::ReplaceOne),
                button(text("All").size(text_size))
                    .style(style::chevron_button())
                    .padding(Padding::from([4, 8]))
                    .on_press(Message::ReplaceAll),
            ]
            .spacing(4);

            let replace_row = row![replace_input, replace_buttons]
                .spacing(spacing)
                .align_y(Alignment::Center);

            content = content.push(replace_row);
        }

        // Keyboard shortcut hints
        let hints = row![
            text("Enter")
                .size((10.0 * scale).max(8.0))
                .color(style::MUTED),
            text("search")
                .size((10.0 * scale).max(8.0))
                .color(style::MUTED),
            text("  Esc")
                .size((10.0 * scale).max(8.0))
                .color(style::MUTED),
            text("close")
                .size((10.0 * scale).max(8.0))
                .color(style::MUTED),
        ]
        .spacing(4)
        .align_y(Alignment::Center);

        content = content.push(hints);

        // Wrap in floating container
        container(content)
            .padding(Padding::from([8, 10]))
            .width(Length::Fixed((320.0 * scale).clamp(280.0, 400.0)))
            .style(floating_panel_container())
            .into()
    }
}
