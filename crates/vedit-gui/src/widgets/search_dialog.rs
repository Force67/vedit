use iced::widget::{button, checkbox, container, row, text, text_input, column, horizontal_space};
use iced::{Element, Color, Length, Padding, Alignment};
use crate::message::Message;
use crate::style::panel_container;

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
        }
    }
}

impl SearchDialog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn show(&mut self) {
        self.is_visible = true;
    }

    pub fn hide(&mut self) {
        self.is_visible = false;
        self.search_query.clear();
        self.replace_text.clear();
        self.current_match = None;
        self.total_matches = 0;
    }

    pub fn toggle(&mut self) {
        println!("Search dialog toggle called - current visibility: {}", self.is_visible);
        if self.is_visible {
            self.hide();
        } else {
            self.show();
        }
        println!("Search dialog visibility after toggle: {}", self.is_visible);
    }

    pub fn set_search_query(&mut self, query: String) {
        self.search_query = query;
    }

    pub fn set_replace_text(&mut self, text: String) {
        self.replace_text = text;
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

    pub fn set_matches(&mut self, current: Option<usize>, total: usize) {
        self.current_match = current;
        self.total_matches = total;
    }

    pub fn enable_replace_mode(&mut self) {
        self.replace_mode = true;
    }

    pub fn disable_replace_mode(&mut self) {
        self.replace_mode = false;
    }

    pub fn view(&self, scale: f32) -> Element<Message> {
        println!("Search dialog view called - visibility: {}", self.is_visible);
        if !self.is_visible {
            return container(row![])
                .height(Length::Fixed(0.0))
                .width(Length::Fixed(0.0))
                .into();
        }

        let spacing = 8.0 * scale;
        let padding = Padding::from([spacing, spacing * 1.5]);

        // Search input row
        let search_input = text_input("Find", &self.search_query)
            .on_input(Message::SearchQueryChanged)
            .on_paste(Message::SearchQueryChanged)
            .size(14.0 * scale)
            .padding(4.0 * scale);

        let search_row = row![
            text("Find:").size(14.0 * scale),
            search_input,
            button(text("▲").size(12.0 * scale))
                .on_press(Message::SearchPrevious)
                .padding(4.0 * scale)
                .width(Length::Fixed(30.0 * scale)),
            button(text("▼").size(12.0 * scale))
                .on_press(Message::SearchNext)
                .padding(4.0 * scale)
                .width(Length::Fixed(30.0 * scale)),
        ]
        .spacing(spacing)
        .align_items(Alignment::Center);

        // Replace input row (visible when replace mode is enabled)
        let replace_row = if self.replace_mode {
            let replace_input = text_input("Replace", &self.replace_text)
                .on_input(Message::ReplaceTextChanged)
                .on_paste(Message::ReplaceTextChanged)
                .size(14.0 * scale)
                .padding(4.0 * scale);

            Some(row![
                text("Replace:").size(14.0 * scale),
                replace_input,
                button(text("Replace").size(12.0 * scale))
                    .on_press(Message::ReplaceOne)
                    .padding(4.0 * scale),
                button(text("All").size(12.0 * scale))
                    .on_press(Message::ReplaceAll)
                    .padding(4.0 * scale),
            ]
            .spacing(spacing)
            .align_items(Alignment::Center))
        } else {
            None
        };

        // Options row
        let case_checkbox = checkbox("", self.case_sensitive)
            .on_toggle(Message::SearchCaseSensitive)
            .spacing(4.0 * scale)
            .size(14.0 * scale);

        let whole_word_checkbox = checkbox("", self.whole_word)
            .on_toggle(Message::SearchWholeWord)
            .spacing(4.0 * scale)
            .size(14.0 * scale);

        let regex_checkbox = checkbox("", self.use_regex)
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
        .align_items(Alignment::Center);

        // Results text
        let results_text = if self.total_matches > 0 {
            if let Some(current) = self.current_match {
                format!("{} of {} matches", current + 1, self.total_matches)
            } else {
                format!("{} matches", self.total_matches)
            }
        } else if !self.search_query.is_empty() {
            "No matches".to_string()
        } else {
            "".to_string()
        };

        let results_label = text(results_text)
            .size(12.0 * scale)
            .style(Color::from_rgb8(140, 140, 140));

        // Close button
        let close_button = button(text("✕").size(14.0 * scale))
            .on_press(Message::SearchClose)
            .padding(4.0 * scale)
            .width(Length::Fixed(24.0 * scale))
            .height(Length::Fixed(24.0 * scale));

        // Header row with title and close button
        let header_row = row![
            text("Search").size(16.0 * scale),
            horizontal_space(),
            close_button,
        ]
        .align_items(Alignment::Center);

        // Main content column
        let mut content = column![
            header_row,
            search_row,
            options_row,
            row![results_label, horizontal_space()],
        ]
        .spacing(spacing);

        // Add replace row if in replace mode
        if let Some(replace_row) = replace_row {
            content = content.push(replace_row);
        }

        // Action buttons row
        let action_buttons = row![
            button(text("Toggle Replace").size(12.0 * scale))
                .on_press(Message::SearchToggleReplace)
                .padding(6.0 * scale),
            button(text("Close").size(12.0 * scale))
                .on_press(Message::SearchClose)
                .padding(6.0 * scale),
        ]
        .spacing(spacing)
        .align_items(Alignment::Center);

        content = content.push(action_buttons);

        // Wrap in container with styling
        container(content)
            .padding(padding)
            .width(Length::Fill)
            .style(panel_container())
            .into()
    }
}