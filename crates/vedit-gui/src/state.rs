use crate::quick_commands::{commands as quick_commands_list, QuickCommand, QuickCommandId};
use crate::scaling;
use iced::widget::text_editor::{Action as TextEditorAction, Content};
use std::env;
use std::path::Path;
use vedit_core::{Editor, KeyEvent, Keymap, KeymapError};

#[derive(Debug)]
pub struct EditorState {
    editor: Editor,
    error: Option<String>,
    buffer_content: Content,
    keymap: Keymap,
    quick_commands: &'static [QuickCommand],
    command_palette: CommandPaletteState,
    scale_factor: f64,
}

impl Default for EditorState {
    fn default() -> Self {
        let mut state = Self {
            editor: Editor::new(),
            error: None,
            buffer_content: Content::new(),
            keymap: Keymap::default(),
            quick_commands: quick_commands_list(),
            command_palette: CommandPaletteState::default(),
            scale_factor: scaling::detect_scale_factor().unwrap_or(1.0),
        };
        state.sync_buffer_from_editor();

        if let Ok(current_dir) = env::current_dir() {
            let candidate = current_dir.join("keybindings.toml");
            if candidate.exists() {
                if let Err(err) = state.load_keymap_from_file(&candidate) {
                    state.error = Some(format!("Failed to load keybindings: {}", err));
                }
            }
        }

        state
    }
}

impl EditorState {
    pub fn load_keymap_from_file(&mut self, path: impl AsRef<Path>) -> Result<(), KeymapError> {
        let mut merged = Keymap::default();
        merged.merge(Keymap::load_from_file(path)?);
        self.keymap = merged;
        Ok(())
    }

    pub fn editor(&self) -> &Editor {
        &self.editor
    }

    pub fn editor_mut(&mut self) -> &mut Editor {
        &mut self.editor
    }

    pub fn buffer_content(&self) -> &Content {
        &self.buffer_content
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn set_error(&mut self, message: Option<String>) {
        self.error = message;
    }

    pub fn clear_error(&mut self) {
        self.error = None;
    }

    pub fn sync_buffer_from_editor(&mut self) {
        let contents = self
            .editor
            .active_document()
            .map(|doc| doc.buffer.clone())
            .unwrap_or_default();

        self.buffer_content = Content::with_text(&contents);
    }

    pub fn apply_buffer_action(&mut self, action: TextEditorAction) {
        let is_edit = action.is_edit();
        self.buffer_content.perform(action);

        if is_edit {
            let updated = self.editor_contents_to_string();
            self.editor.update_active_buffer(updated);
        }
    }

    pub fn quick_commands(&self) -> &'static [QuickCommand] {
        self.quick_commands
    }

    pub fn command_palette(&self) -> &CommandPaletteState {
        &self.command_palette
    }

    pub fn open_command_palette(&mut self) {
        self.command_palette.open(self.quick_commands);
    }

    pub fn close_command_palette(&mut self) {
        self.command_palette.close();
    }

    pub fn set_command_palette_query(&mut self, query: String) {
        self.command_palette
            .set_query(query, self.quick_commands);
    }

    pub fn selected_quick_command(&self) -> Option<QuickCommandId> {
        self.command_palette
            .selected_command(self.quick_commands)
            .map(|command| command.id)
    }

    pub fn handle_quick_command_navigation(&mut self, delta: i32) {
        self.command_palette
            .move_selection(delta, self.quick_commands);
    }

    pub fn matches_action(&self, action: &str, event: &KeyEvent) -> bool {
        self.keymap
            .binding(action)
            .map(|binding| binding.matches(event))
            .unwrap_or(false)
    }

    pub fn scale_factor(&self) -> f64 {
        self.scale_factor
    }

    pub fn format_scale_factor(&self) -> String {
        format!("Scale factor: {:.2}", self.scale_factor)
    }

    fn editor_contents_to_string(&self) -> String {
        let mut text = self.buffer_content.text();
        if text.ends_with('\n') {
            text.pop();
        }
        text
    }
}

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
