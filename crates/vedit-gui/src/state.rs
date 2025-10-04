use crate::quick_commands::{commands as quick_commands_list, QuickCommand, QuickCommandId};
use crate::settings::SettingsState;
use crate::scaling;
use crate::syntax::{DocumentKey, SyntaxSettings, SyntaxSystem};
use crate::widgets::text_editor::{Action as TextEditorAction, Content};
use std::env;
use std::path::{Path, PathBuf};
use vedit_core::{Editor, FileNode, KeyCombination, KeyEvent, Keymap, KeymapError, Language, WorkspaceConfig};

#[derive(Debug)]
pub struct EditorState {
    editor: Editor,
    error: Option<String>,
    buffer_content: Content,
    keymap: Keymap,
    quick_commands: &'static [QuickCommand],
    command_palette: CommandPaletteState,
    scale_factor: f64,
    syntax: SyntaxSystem,
    settings: SettingsState,
    settings_error: Option<String>,
    settings_notice: Option<String>,
    settings_dirty: bool,
    keymap_path: Option<PathBuf>,
    workspace_notice: Option<String>,
}

impl Default for EditorState {
    fn default() -> Self {
        let quick_commands = quick_commands_list();
        let keymap = Keymap::default();
        let settings = SettingsState::new(quick_commands, &keymap);

        let keymap_path = env::current_dir()
            .ok()
            .map(|dir| dir.join("keybindings.toml"));

        let mut state = Self {
            editor: Editor::new(),
            error: None,
            buffer_content: Content::new(),
            keymap,
            quick_commands,
            command_palette: CommandPaletteState::default(),
            scale_factor: scaling::detect_scale_factor().unwrap_or(1.0),
            syntax: SyntaxSystem::new(),
            settings,
            settings_error: None,
            settings_notice: None,
            settings_dirty: false,
            keymap_path,
            workspace_notice: None,
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

        state.settings.sync_bindings(state.quick_commands, &state.keymap);

        state
    }
}

impl EditorState {
    pub fn load_keymap_from_file(&mut self, path: impl AsRef<Path>) -> Result<(), KeymapError> {
        let mut merged = Keymap::default();
        let path_ref = path.as_ref();
        merged.merge(Keymap::load_from_file(path_ref)?);
        self.keymap = merged;
        self.keymap_path = Some(path_ref.to_path_buf());
        self.settings
            .sync_bindings(self.quick_commands, &self.keymap);
        self.settings_dirty = false;
        self.settings_notice = None;
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

    pub fn syntax_settings(&self) -> SyntaxSettings {
        let fallback = DocumentKey::Index(self.editor.active_index());
        let key = self
            .active_document_identity()
            .map(|(key, _)| key)
            .unwrap_or(fallback);
        self.syntax.settings_for(key)
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn settings_error(&self) -> Option<&str> {
        self.settings_error.as_deref()
    }

    pub fn settings_notice(&self) -> Option<&str> {
        self.settings_notice.as_deref()
    }

    pub fn workspace_notice(&self) -> Option<&str> {
        self.workspace_notice.as_deref()
    }

    pub fn workspace_display_name(&self) -> Option<&str> {
        self.editor.workspace_name()
    }

    pub fn keymap_path_display(&self) -> Option<String> {
        self.keymap_path
            .as_ref()
            .map(|path| path.to_string_lossy().to_string())
    }

    pub fn set_error(&mut self, message: Option<String>) {
        self.error = message;
        if self.error.is_some() {
            self.workspace_notice = None;
        }
    }

    pub fn clear_error(&mut self) {
        self.error = None;
        self.settings_error = None;
        self.settings_notice = None;
        self.workspace_notice = None;
    }

    pub fn sync_buffer_from_editor(&mut self) {
        let contents = self
            .editor
            .active_document()
            .map(|doc| doc.buffer.clone())
            .unwrap_or_default();

        self.buffer_content = Content::with_text(&contents);
        self.refresh_active_highlighting(&contents);
    }

    pub fn apply_buffer_action(&mut self, action: TextEditorAction) {
        let is_edit = action.is_edit();
        self.buffer_content.perform(action);

        if is_edit {
            let updated = self.editor_contents_to_string();
            self.editor.update_active_buffer(updated.clone());
            self.refresh_active_highlighting(&updated);
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

    pub fn handle_document_saved(&mut self, path: Option<String>) {
        self.editor.mark_active_document_saved(path);
        if let Some(buffer) = self
            .editor
            .active_document()
            .map(|doc| doc.buffer.clone())
        {
            self.refresh_active_highlighting(&buffer);
        }
    }

    pub fn open_settings(&mut self) {
        self.settings.open();
        self.settings
            .sync_bindings(self.quick_commands, &self.keymap);
        self.clear_error();
        self.command_palette.close();
    }

    pub fn close_settings(&mut self) {
        self.settings.close();
        self.settings_error = None;
        self.settings_notice = None;
    }

    pub fn install_workspace(
        &mut self,
        root: String,
        tree: Vec<FileNode>,
        config: WorkspaceConfig,
    ) {
        self.editor.set_workspace(root, tree, config);
        self.workspace_notice = None;
        self.sync_buffer_from_editor();
    }

    pub fn workspace_recent_files(&self) -> Vec<String> {
        self.editor
            .workspace_config()
            .map(|config| config.recent_files().map(|entry| entry.to_string()).collect())
            .unwrap_or_default()
    }

    pub fn record_recent_workspace_file(&mut self) -> Option<(String, WorkspaceConfig)> {
        let path = self
            .editor
            .active_document()
            .and_then(|doc| doc.path.clone())?;
        let root = self.editor.workspace_root()?.to_string();

        {
            let config = self.editor.workspace_config_mut()?;
            if !config.record_recent_file(&path) {
                return None;
            }
        }

        let snapshot = self.editor.workspace_config()?.clone();
        Some((root, snapshot))
    }

    pub fn apply_workspace_config_saved(&mut self, root: String) {
        self.workspace_notice = Some(format!("Workspace preferences saved for {}", root));
    }

    pub fn settings(&self) -> &SettingsState {
        &self.settings
    }

    pub fn settings_mut(&mut self) -> &mut SettingsState {
        &mut self.settings
    }

    pub fn clear_binding_error(&mut self, id: QuickCommandId) {
        self.settings.set_binding_error(id, None);
        self.settings_error = None;
    }

    pub fn apply_quick_command_binding(
        &mut self,
        id: QuickCommandId,
    ) -> Result<(), String> {
        let command = self
            .quick_commands
            .iter()
            .find(|cmd| cmd.id == id)
            .ok_or_else(|| "Unknown command".to_string())?;

        let action = command
            .action
            .ok_or_else(|| "This command cannot be bound".to_string())?;

        let input = self
            .settings
            .binding_input(id)
            .trim()
            .to_string();

        if input.is_empty() {
            self.keymap.set_binding(action, None);
            self.settings.set_binding_error(id, None);
            self.settings.set_binding_input(id, String::new());
            self.settings_error = None;
            self.settings_notice = Some("Binding removed. Save to persist changes.".to_string());
            self.settings_dirty = true;
            return Ok(());
        }

        match KeyCombination::parse(&input) {
            Ok(combo) => {
                let display = combo.to_string();
                self.keymap.set_binding(action, Some(combo));
                self.settings.set_binding_input(id, display);
                self.settings.set_binding_error(id, None);
                self.settings_error = None;
                self.settings_notice = Some("Binding updated. Save to persist changes.".to_string());
                self.settings_dirty = true;
                Ok(())
            }
            Err(err) => {
                let message = err.to_string();
                self.settings
                    .set_binding_error(id, Some(message.clone()));
                self.settings_error = Some(message.clone());
                self.settings_notice = None;
                Err(message)
            }
        }
    }

    pub fn settings_dirty(&self) -> bool {
        self.settings_dirty
    }

    pub fn keymap_save_payload(&self) -> Result<(String, String), String> {
        let path = self
            .keymap_path
            .clone()
            .ok_or_else(|| "No keymap file path available".to_string())?;

        let contents = self
            .keymap
            .to_toml_string()
            .map_err(|err| format!("Failed to serialize keymap: {}", err))?;

        Ok((path.to_string_lossy().to_string(), contents))
    }

    pub fn mark_keymap_saved(&mut self, path: String) {
        self.keymap_path = Some(PathBuf::from(&path));
        self.settings_dirty = false;
        self.settings_error = None;
        self.settings_notice = Some(format!("Saved keybindings to {}", path));
    }

    pub fn apply_selected_keymap_path(&mut self, path: String) -> Result<(), String> {
        let candidate = PathBuf::from(&path);

        if candidate.exists() {
            match Keymap::load_from_file(&candidate) {
                Ok(loaded) => {
                    let mut merged = Keymap::default();
                    merged.merge(loaded);
                    self.keymap = merged;
                    self.settings
                        .sync_bindings(self.quick_commands, &self.keymap);
                    self.keymap_path = Some(candidate);
                    self.settings_dirty = false;
                    self.settings_error = None;
                    self.settings_notice = Some(format!("Loaded keybindings from {}", path));
                    Ok(())
                }
                Err(err) => Err(err.to_string()),
            }
        } else {
            self.keymap_path = Some(candidate);
            self.settings_dirty = true;
            self.settings_notice = Some(format!(
                "New keymap location selected: {}. Save to create this file.",
                path
            ));
            self.settings_error = None;
            Ok(())
        }
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

    fn refresh_active_highlighting(&mut self, contents: &str) {
        if let Some((key, language)) = self.active_document_identity() {
            self.syntax.update_document(key, language, contents);
        }
    }

    fn active_document_identity(&self) -> Option<(DocumentKey, Language)> {
        let index = self.editor.active_index();
        self.editor.active_document().map(|doc| {
            let key = doc
                .fingerprint
                .map(DocumentKey::Fingerprint)
                .unwrap_or(DocumentKey::Index(index));
            (key, doc.language())
        })
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
