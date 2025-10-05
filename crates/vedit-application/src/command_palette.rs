use crate::quick_commands::QuickCommand;

#[derive(Debug, Default)]
pub struct CommandPaletteState {
    is_open: bool,
    query: String,
    selection: usize,
}

impl CommandPaletteState {
    pub fn is_open(&self) -> bool {
        self.is_open
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn selection_index(&self) -> usize {
        self.selection
    }

    pub fn open(&mut self, commands: &[QuickCommand]) {
        self.is_open = true;
        if self.query.is_empty() {
            self.selection = 0;
        } else {
            self.ensure_selection(commands);
        }
    }

    pub fn close(&mut self) {
        self.is_open = false;
    }

    pub fn set_query(&mut self, query: String, commands: &[QuickCommand]) {
        self.query = query;
        self.selection = 0;
        self.ensure_selection(commands);
    }

    pub fn filtered_indices(&self, commands: &[QuickCommand]) -> Vec<usize> {
        let query = self.query.to_ascii_lowercase();
        commands
            .iter()
            .enumerate()
            .filter(|(_, command)| {
                if query.is_empty() {
                    true
                } else {
                    command
                        .title
                        .to_ascii_lowercase()
                        .contains(&query)
                        || command
                            .description
                            .to_ascii_lowercase()
                            .contains(&query)
                }
            })
            .map(|(index, _)| index)
            .collect()
    }

    pub fn selected_command<'a>(&self, commands: &'a [QuickCommand]) -> Option<&'a QuickCommand> {
        let filtered = self.filtered_indices(commands);
        filtered
            .get(self.selection)
            .and_then(|index| commands.get(*index))
    }

    pub fn move_selection(&mut self, delta: i32, commands: &[QuickCommand]) {
        let filtered = self.filtered_indices(commands);
        if filtered.is_empty() {
            self.selection = 0;
            return;
        }

        let len = filtered.len() as i32;
        let current = self.selection as i32;
        let next = (current + delta).rem_euclid(len);
        self.selection = next as usize;
    }

    pub fn ensure_selection(&mut self, commands: &[QuickCommand]) {
        let filtered = self.filtered_indices(commands);
        if filtered.is_empty() {
            self.selection = 0;
        } else if self.selection >= filtered.len() {
            self.selection = filtered.len() - 1;
        }
    }
}
