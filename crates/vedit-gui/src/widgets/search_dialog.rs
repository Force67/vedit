// SearchDialog has some helper methods not yet used by the main app
#![allow(dead_code)]

use crate::message::Message;
use crate::style::panel_container;
use iced::widget::{Space, button, checkbox, column, container, row, text, text_input};
use iced::{Alignment, Color, Element, Length};
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
        let spacing = 8.0 * scale;

        if !self.is_visible {
            return container(text("")).into();
        }

        // Search input field
        let search_input = text_input("Find", &self.search_query)
            .id(self.search_input_id.clone())
            .on_input(Message::SearchQueryChanged)
            .on_paste(Message::SearchQueryChanged)
            .on_submit(Message::SearchExecute)
            .size(14.0 * scale)
            .padding(4.0 * scale);

        let search_row = row![
            text("Find:").size(14.0 * scale),
            search_input,
            button(
                fa_icon_solid("chevron-up")
                    .size(14.0 * scale)
                    .color(Color::from_rgb8(180, 180, 180))
            )
            .on_press(Message::SearchPrevious)
            .padding(4.0 * scale)
            .width(Length::Fixed(30.0 * scale)),
            button(
                fa_icon_solid("chevron-down")
                    .size(14.0 * scale)
                    .color(Color::from_rgb8(180, 180, 180))
            )
            .on_press(Message::SearchNext)
            .padding(4.0 * scale)
            .width(Length::Fixed(30.0 * scale)),
        ]
        .spacing(spacing)
        .align_y(Alignment::Center);

        // Replace input row (visible when replace mode is enabled)
        let replace_row = if self.replace_mode {
            let replace_input = text_input("Replace", &self.replace_text)
                .on_input(Message::ReplaceTextChanged)
                .on_paste(Message::ReplaceTextChanged)
                .size(14.0 * scale)
                .padding(4.0 * scale);

            Some(
                row![
                    text("Replace:").size(14.0 * scale),
                    replace_input,
                    button(
                        row![
                            fa_icon_solid("file")
                                .size(12.0 * scale)
                                .color(Color::from_rgb8(180, 180, 180)),
                            text("Replace").size(12.0 * scale)
                        ]
                        .spacing(4.0 * scale)
                        .align_y(Alignment::Center)
                    )
                    .on_press(Message::ReplaceOne)
                    .padding(4.0 * scale),
                    button(
                        row![
                            fa_icon_solid("file")
                                .size(12.0 * scale)
                                .color(Color::from_rgb8(180, 180, 180)),
                            text("All").size(12.0 * scale)
                        ]
                        .spacing(4.0 * scale)
                        .align_y(Alignment::Center)
                    )
                    .on_press(Message::ReplaceAll)
                    .padding(4.0 * scale),
                ]
                .spacing(spacing)
                .align_y(Alignment::Center),
            )
        } else {
            None
        };

        // Options row
        let case_checkbox = checkbox(self.case_sensitive)
            .label("")
            .on_toggle(Message::SearchCaseSensitive)
            .spacing(4.0 * scale)
            .size(14.0 * scale);

        let whole_word_checkbox = checkbox(self.whole_word)
            .label("")
            .on_toggle(Message::SearchWholeWord)
            .spacing(4.0 * scale)
            .size(14.0 * scale);

        let regex_checkbox = checkbox(self.use_regex)
            .label("")
            .on_toggle(Message::SearchUseRegex)
            .spacing(4.0 * scale)
            .size(14.0 * scale);

        let options_row = row![
            case_checkbox,
            text("Match Case").size(12.0 * scale),
            whole_word_checkbox,
            text("Whole Word").size(12.0 * scale),
            regex_checkbox,
            text("Regex").size(12.0 * scale),
        ]
        .spacing(spacing * 1.5)
        .align_y(Alignment::Center);

        // Results text
        let results_text = match self.search_state {
            SearchState::Searching => "Searching...".to_string(),
            SearchState::Complete => {
                if self.total_matches == 0 {
                    "No matches found".to_string()
                } else {
                    match self.current_match {
                        Some(current) => format!("Match {} of {}", current + 1, self.total_matches),
                        None => format!("{} matches found", self.total_matches),
                    }
                }
            }
            SearchState::NoMatches => "No matches found".to_string(),
            SearchState::Idle => {
                if self.search_query.is_empty() {
                    "Enter search text".to_string()
                } else {
                    "Ready to search".to_string()
                }
            }
        };

        let results_color = match self.search_state {
            SearchState::Searching => Color::from_rgb8(100, 150, 255), // Blue
            SearchState::Complete => Color::from_rgb8(100, 200, 100),  // Green
            SearchState::NoMatches => Color::from_rgb8(255, 100, 100), // Red
            SearchState::Idle => Color::from_rgb8(160, 160, 160),      // Lighter gray
        };

        let results_label = text(results_text).size(12.0 * scale).color(results_color);

        // Close button
        let close_button = button(
            fa_icon_solid("xmark")
                .size(14.0 * scale)
                .color(Color::from_rgb8(180, 180, 180)),
        )
        .on_press(Message::SearchClose)
        .padding(4.0 * scale)
        .width(Length::Fixed(24.0 * scale))
        .height(Length::Fixed(24.0 * scale));

        // Header row with title and close button
        let header_row = row![
            text("Search").size(16.0 * scale),
            Space::new().width(Length::Fill),
            close_button,
        ]
        .align_y(Alignment::Center);

        // Main content column
        let mut content = column![header_row, search_row, options_row, results_label,]
            .spacing(spacing)
            .width(Length::Fill);

        // Add replace row if in replace mode
        if let Some(replace_row) = replace_row {
            content = content.push(replace_row);
        }

        // Action buttons row
        let mut action_buttons = row![
            button(
                row![
                    fa_icon_solid("gear")
                        .size(12.0 * scale)
                        .color(Color::from_rgb8(180, 180, 180)),
                    text("Toggle Replace").size(12.0 * scale)
                ]
                .spacing(4.0 * scale)
                .align_y(Alignment::Center)
            )
            .on_press(Message::SearchToggleReplace)
            .padding(6.0 * scale),
        ]
        .spacing(spacing)
        .align_y(Alignment::Center);

        // Add Next Match button if there are multiple matches
        if self.search_state == SearchState::Complete && self.total_matches > 1 {
            action_buttons = action_buttons.push(
                button(
                    row![
                        fa_icon_solid("angle-down")
                            .size(12.0 * scale)
                            .color(Color::from_rgb8(180, 180, 180)),
                        text("Next Match").size(12.0 * scale)
                    ]
                    .spacing(4.0 * scale)
                    .align_y(Alignment::Center),
                )
                .on_press(Message::SearchNext)
                .padding(6.0 * scale),
            );
        }

        action_buttons = action_buttons.push(
            button(
                row![
                    fa_icon_solid("xmark")
                        .size(12.0 * scale)
                        .color(Color::from_rgb8(180, 180, 180)),
                    text("Close").size(12.0 * scale)
                ]
                .spacing(4.0 * scale)
                .align_y(Alignment::Center),
            )
            .on_press(Message::SearchClose)
            .padding(6.0 * scale),
        );

        content = content.push(action_buttons);

        // Wrap in container with styling
        container(content)
            .padding(spacing)
            .style(panel_container())
            .width(Length::Fill)
            .into()
    }
}
