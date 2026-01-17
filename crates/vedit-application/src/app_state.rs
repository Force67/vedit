use crate::quick_commands::{QuickCommand, QuickCommandId, list as quick_commands_list};
use crate::settings::SettingsState;
use std::env;
use std::path::{Path, PathBuf};
use vedit_config::{DebugTargetRecord, WorkspaceConfig, WorkspaceMetadata};
use vedit_core::{Editor, KeyCombination, KeyEvent, Keymap, KeymapError, StickyNote};

/// Core application state that owns the editor session, keymap, and workspace logic.
#[derive(Debug)]
pub struct AppState {
    editor: Editor,
    error: Option<String>,
    keymap: Keymap,
    quick_commands: &'static [QuickCommand],
    settings: SettingsState,
    settings_error: Option<String>,
    settings_notice: Option<String>,
    settings_dirty: bool,
    keymap_path: Option<PathBuf>,
    workspace_notice: Option<String>,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        let quick_commands = quick_commands_list();
        let keymap = Keymap::default();
        let settings = SettingsState::new(quick_commands, &keymap);
        let keymap_path = env::current_dir()
            .ok()
            .map(|dir| dir.join("keybindings.toml"));

        let mut state = Self {
            editor: Editor::new(),
            error: None,
            keymap,
            quick_commands,
            settings,
            settings_error: None,
            settings_notice: None,
            settings_dirty: false,
            keymap_path,
            workspace_notice: None,
        };

        if let Some(path) = state.keymap_path.clone() {
            if path.exists() {
                if let Err(err) = state.load_keymap_from_file(&path) {
                    state.error = Some(format!("Failed to load keybindings: {}", err));
                }
            }
        }

        state
            .settings
            .sync_bindings(state.quick_commands, &state.keymap);

        state
    }

    pub fn quick_commands(&self) -> &'static [QuickCommand] {
        self.quick_commands
    }

    pub fn editor(&self) -> &Editor {
        &self.editor
    }

    pub fn editor_mut(&mut self) -> &mut Editor {
        &mut self.editor
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

    pub fn set_error(&mut self, message: Option<String>) {
        self.error = message;
        if self.error.is_some() {
            self.workspace_notice = None;
        }
    }

    pub fn clear_messages(&mut self) {
        self.error = None;
        self.settings_error = None;
        self.settings_notice = None;
        self.workspace_notice = None;
    }

    pub fn matches_action(&self, action: &str, event: &KeyEvent) -> bool {
        self.keymap
            .binding(action)
            .map(|binding| binding.matches(event))
            .unwrap_or(false)
    }

    pub fn handle_document_saved(&mut self, path: Option<String>) {
        self.editor.mark_active_document_saved(path);
    }

    pub fn open_settings(&mut self) {
        self.settings.open();
        self.settings
            .sync_bindings(self.quick_commands, &self.keymap);
        self.clear_messages();
    }

    pub fn close_settings(&mut self) {
        self.settings.close();
        self.settings_error = None;
        self.settings_notice = None;
    }

    pub fn settings(&self) -> &SettingsState {
        &self.settings
    }

    pub fn settings_mut(&mut self) -> &mut SettingsState {
        &mut self.settings
    }

    pub fn workspace_recent_files(&self) -> Vec<String> {
        self.editor
            .workspace_config()
            .map(|config| {
                config
                    .recent_files()
                    .map(|entry| entry.to_string())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn workspace_recent_debug_targets(&self) -> Vec<DebugTargetRecord> {
        self.editor
            .workspace_config()
            .map(|config| config.recent_debug_targets().cloned().collect())
            .unwrap_or_default()
    }

    pub fn workspace_last_debug_target(&self) -> Option<DebugTargetRecord> {
        self.editor
            .workspace_config()
            .and_then(|config| config.last_debug_target().cloned())
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

    pub fn record_recent_debug_target(
        &mut self,
        name: &str,
        executable: impl AsRef<Path>,
    ) -> Option<(String, WorkspaceConfig)> {
        let root = self.editor.workspace_root()?.to_string();
        let changed = {
            let config = self.editor.workspace_config_mut()?;
            config.record_debug_target(name, executable)
        };

        if !changed {
            return None;
        }

        let snapshot = self.editor.workspace_config()?.clone();
        Some((root, snapshot))
    }

    pub fn apply_workspace_config_saved(&mut self, root: String) {
        self.workspace_notice = Some(format!("Workspace preferences saved for {}", root));
    }

    pub fn apply_workspace_metadata_saved(&mut self, root: String) {
        self.workspace_notice = Some(format!("Workspace notes saved for {}", root));
    }

    pub fn install_workspace(
        &mut self,
        root: String,
        config: WorkspaceConfig,
        metadata: WorkspaceMetadata,
    ) {
        self.editor.set_workspace(root, config, metadata);
        self.workspace_notice = None;
    }

    pub fn take_workspace_metadata_payload(&mut self) -> Option<(String, WorkspaceMetadata)> {
        self.editor.take_workspace_metadata_payload()
    }

    pub fn active_sticky_notes(&self) -> Vec<StickyNote> {
        self.editor
            .active_sticky_notes()
            .map(|notes| notes.to_vec())
            .unwrap_or_default()
    }

    pub fn update_sticky_note_content(&mut self, id: u64, content: String) {
        if self.editor.update_sticky_note_content(id, content) {
            self.workspace_notice = None;
        } else {
            self.workspace_notice = Some("Unable to update sticky note".to_string());
        }
    }

    pub fn remove_sticky_note(&mut self, id: u64) {
        if self.editor.remove_sticky_note(id) {
            self.workspace_notice = None;
        } else {
            self.workspace_notice = Some("Failed to remove sticky note".to_string());
        }
    }

    pub fn clear_binding_error(&mut self, id: QuickCommandId) {
        self.settings.set_binding_error(id, None);
        self.settings_error = None;
    }

    pub fn apply_quick_command_binding(&mut self, id: QuickCommandId) -> Result<(), String> {
        let command = self
            .quick_commands
            .iter()
            .find(|cmd| cmd.id == id)
            .ok_or_else(|| "Unknown command".to_string())?;

        let action = command
            .action
            .ok_or_else(|| "This command cannot be bound".to_string())?;

        let input = self.settings.binding_input(id).trim().to_string();

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
                self.settings_notice =
                    Some("Binding updated. Save to persist changes.".to_string());
                self.settings_dirty = true;
                Ok(())
            }
            Err(err) => {
                let message = err.to_string();
                self.settings.set_binding_error(id, Some(message.clone()));
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
}
