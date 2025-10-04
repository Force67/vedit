use crate::quick_commands::{commands as quick_commands_list, QuickCommand, QuickCommandId};
use crate::settings::SettingsState;
use crate::scaling;
use crate::syntax::{DocumentKey, SyntaxSettings, SyntaxSystem};
use crate::widgets::text_editor::{Action as TextEditorAction, Content};
use iced::keyboard;
use std::collections::HashSet;
use std::env;
use std::path::{Path, PathBuf};
use vedit_core::{Editor, FileNode, KeyCombination, KeyEvent, Keymap, KeymapError, Language, WorkspaceConfig};

const ZOOM_STEP_ENV: &str = "VEDIT_ZOOM_STEP";
const ZOOM_MIN_ENV: &str = "VEDIT_ZOOM_MIN";
const ZOOM_MAX_ENV: &str = "VEDIT_ZOOM_MAX";
const ZOOM_DEFAULT_ENV: &str = "VEDIT_ZOOM_DEFAULT";

#[derive(Debug, Clone, Copy)]
struct ZoomConfig {
    min: f64,
    max: f64,
    step: f64,
    default_override: Option<f64>,
}

fn collect_directory_paths(nodes: &[FileNode], output: &mut Vec<String>) {
    for node in nodes {
        if node.is_directory {
            output.push(node.path.clone());
            if !node.children.is_empty() {
                collect_directory_paths(&node.children, output);
            }
        }
    }
}

impl ZoomConfig {
    fn load() -> Self {
        let mut config = Self {
            min: 0.6,
            max: 3.0,
            step: 0.1,
            default_override: None,
        };

        if let Some(min) = read_env_f64(ZOOM_MIN_ENV) {
            if min > 0.1 {
                config.min = min;
            }
        }

        if let Some(max) = read_env_f64(ZOOM_MAX_ENV) {
            if max > config.min {
                config.max = max;
            }
        }

        if let Some(step) = read_env_f64(ZOOM_STEP_ENV) {
            if step > 0.0 {
                config.step = step;
            }
        }

        if let Some(default) = read_env_f64(ZOOM_DEFAULT_ENV) {
            config.default_override = Some(default);
        }

        config
    }

    fn clamp(&self, value: f64) -> f64 {
        value.clamp(self.min, self.max)
    }

    fn initial_scale(&self, detected: f64) -> f64 {
        let base = self.default_override.unwrap_or(detected);
        self.clamp(base)
    }
}

fn read_env_f64(name: &str) -> Option<f64> {
    env::var(name).ok().and_then(|value| value.parse::<f64>().ok())
}

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
    workspace_collapsed: HashSet<String>,
    workspace_collapsed_version: u64,
    zoom_config: ZoomConfig,
    modifiers: keyboard::Modifiers,
}

impl Default for EditorState {
    fn default() -> Self {
        let quick_commands = quick_commands_list();
        let keymap = Keymap::default();
        let settings = SettingsState::new(quick_commands, &keymap);

        let keymap_path = env::current_dir()
            .ok()
            .map(|dir| dir.join("keybindings.toml"));

        let zoom_config = ZoomConfig::load();
        let detected_scale = scaling::detect_scale_factor().unwrap_or(1.0);
        let initial_scale = zoom_config.initial_scale(detected_scale);

        let mut state = Self {
            editor: Editor::new(),
            error: None,
            buffer_content: Content::new(),
            keymap,
            quick_commands,
            command_palette: CommandPaletteState::default(),
            scale_factor: initial_scale,
            syntax: SyntaxSystem::new(),
            settings,
            settings_error: None,
            settings_notice: None,
            settings_dirty: false,
            keymap_path,
            workspace_notice: None,
            workspace_collapsed: HashSet::new(),
            workspace_collapsed_version: 0,
            zoom_config,
            modifiers: keyboard::Modifiers::default(),
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

    pub fn workspace_collapsed_paths(&self) -> Vec<String> {
        let mut paths: Vec<String> = self.workspace_collapsed.iter().cloned().collect();
        paths.sort();
        paths
    }

    pub fn workspace_collapsed_version(&self) -> u64 {
        self.workspace_collapsed_version
    }

    pub fn toggle_workspace_directory(&mut self, path: String) -> Result<(), String> {
        if self.workspace_collapsed.remove(&path) {
            match self.editor.load_workspace_directory(&path) {
                Ok(new_directories) => {
                    for directory in new_directories {
                        self.workspace_collapsed.insert(directory);
                    }
                }
                Err(err) => {
                    self.workspace_collapsed.insert(path);
                    return Err(format!("Failed to read directory: {}", err));
                }
            }
        } else {
            self.workspace_collapsed.insert(path);
        }
        self.workspace_collapsed_version = self.workspace_collapsed_version.wrapping_add(1);
        Ok(())
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
            .map(|doc| doc.buffer.to_string())
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
            .map(|doc| doc.buffer.to_string())
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
        self.workspace_collapsed.clear();
        if let Some(nodes) = self.editor.workspace_tree() {
            let mut directories = Vec::new();
            collect_directory_paths(nodes, &mut directories);
            for path in directories {
                self.workspace_collapsed.insert(path);
            }
        }
        self.workspace_collapsed_version = self.workspace_collapsed_version.wrapping_add(1);
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
        format!("Zoom: {:.0}%", self.scale_factor * 100.0)
    }

    pub fn increase_scale_factor(&mut self) -> bool {
        self.set_scale_factor(self.scale_factor + self.zoom_config.step)
    }

    pub fn decrease_scale_factor(&mut self) -> bool {
        self.set_scale_factor(self.scale_factor - self.zoom_config.step)
    }

    fn set_scale_factor(&mut self, value: f64) -> bool {
        let clamped = self.zoom_config.clamp(value);
        if (clamped - self.scale_factor).abs() > f64::EPSILON {
            self.scale_factor = clamped;
            true
        } else {
            false
        }
    }

    pub fn set_modifiers(&mut self, modifiers: keyboard::Modifiers) {
        self.modifiers = modifiers;
    }

    pub fn modifiers(&self) -> keyboard::Modifiers {
        self.modifiers
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
