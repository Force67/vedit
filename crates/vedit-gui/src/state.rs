use crate::debugger::{DebugLaunchPlan, DebuggerState, DebuggerUiEvent, DebugTarget};
use crate::notifications::{Notification, NotificationCenter, NotificationRequest};
use crate::scaling;
use crate::syntax::{DocumentKey, SyntaxSettings, SyntaxSystem};
use crate::widgets::text_editor::{
    Action as TextEditorAction, Content, ScrollMetrics, buffer_scroll_metrics, scroll_to,
};
use iced::keyboard;
use std::collections::HashSet;
use std::env;
use std::time::Duration;
use vedit_application::{AppState, CommandPaletteState, QuickCommand, QuickCommandId, SettingsState};
use vedit_core::{Editor, FileNode, KeyEvent, Language, StickyNote, WorkspaceConfig};
use vedit_debugger::GdbSession;
use vedit_config::WorkspaceMetadata;

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
    app: AppState,
    buffer_content: Content,
    command_palette: CommandPaletteState,
    scale_factor: f64,
    syntax: SyntaxSystem,
    workspace_collapsed: HashSet<String>,
    workspace_collapsed_version: u64,
    zoom_config: ZoomConfig,
    modifiers: keyboard::Modifiers,
    debugger: DebuggerState,
    notifications: NotificationCenter,
}

impl Default for EditorState {
    fn default() -> Self {
        let zoom_config = ZoomConfig::load();
        let detected_scale = scaling::detect_scale_factor().unwrap_or(1.0);
        let initial_scale = zoom_config.initial_scale(detected_scale);

        let mut state = Self {
            app: AppState::new(),
            buffer_content: Content::new(),
            command_palette: CommandPaletteState::default(),
            scale_factor: initial_scale,
            syntax: SyntaxSystem::new(),
            workspace_collapsed: HashSet::new(),
            workspace_collapsed_version: 0,
            zoom_config,
            modifiers: keyboard::Modifiers::default(),
            debugger: DebuggerState::default(),
            notifications: NotificationCenter::new(),
        };
        state.sync_buffer_from_editor();
        state
    }
}

impl EditorState {
    pub fn editor(&self) -> &Editor {
        self.app.editor()
    }

    pub fn editor_mut(&mut self) -> &mut Editor {
        self.app.editor_mut()
    }

    pub fn buffer_content(&self) -> &Content {
        &self.buffer_content
    }

    pub fn buffer_scroll_metrics(&self) -> ScrollMetrics {
        buffer_scroll_metrics(&self.buffer_content)
    }

    pub fn set_buffer_scroll(&mut self, position: f32) {
        let metrics = buffer_scroll_metrics(&self.buffer_content);
        let max_scroll = metrics.max_scroll() as f32;
        let target = position
            .clamp(0.0, max_scroll)
            .round() as usize;
        scroll_to(&mut self.buffer_content, target);
    }

    pub fn syntax_settings(&self) -> SyntaxSettings {
        let fallback = DocumentKey::Index(self.app.editor().active_index());
        let key = self
            .active_document_identity()
            .map(|(key, _)| key)
            .unwrap_or(fallback);
        self.syntax.settings_for(key)
    }

    pub fn error(&self) -> Option<&str> {
        self.app.error()
    }

    pub fn settings_error(&self) -> Option<&str> {
        self.app.settings_error()
    }

    pub fn settings_notice(&self) -> Option<&str> {
        self.app.settings_notice()
    }

    pub fn workspace_notice(&self) -> Option<&str> {
        self.app.workspace_notice()
    }

    pub fn workspace_display_name(&self) -> Option<&str> {
        self.app.workspace_display_name()
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
            let result = {
                let editor = self.app.editor_mut();
                editor.load_workspace_directory(&path)
            };

            match result {
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
        self.app.keymap_path_display()
    }

    pub fn set_error(&mut self, message: Option<String>) {
        self.app.set_error(message);
    }

    pub fn clear_error(&mut self) {
        self.app.clear_messages();
    }

    pub fn sync_buffer_from_editor(&mut self) {
        let contents = self
            .app
            .editor()
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
            self.app.editor_mut().update_active_buffer(updated.clone());
            self.refresh_active_highlighting(&updated);
        }
    }

    pub fn quick_commands(&self) -> &'static [QuickCommand] {
        self.app.quick_commands()
    }

    pub fn command_palette(&self) -> &CommandPaletteState {
        &self.command_palette
    }

    pub fn open_command_palette(&mut self) {
        let commands = self.app.quick_commands();
        self.command_palette.open(commands);
    }

    pub fn close_command_palette(&mut self) {
        self.command_palette.close();
    }

    pub fn set_command_palette_query(&mut self, query: String) {
        let commands = self.app.quick_commands();
        self.command_palette.set_query(query, commands);
    }

    pub fn selected_quick_command(&self) -> Option<QuickCommandId> {
        self.command_palette
            .selected_command(self.app.quick_commands())
            .map(|command| command.id)
    }

    pub fn handle_quick_command_navigation(&mut self, delta: i32) {
        let commands = self.app.quick_commands();
        self.command_palette.move_selection(delta, commands);
    }

    pub fn matches_action(&self, action: &str, event: &KeyEvent) -> bool {
        self.app.matches_action(action, event)
    }

    pub fn handle_document_saved(&mut self, path: Option<String>) {
        self.app.handle_document_saved(path);
        if let Some(buffer) = self
            .app
            .editor()
            .active_document()
            .map(|doc| doc.buffer.to_string())
        {
            self.refresh_active_highlighting(&buffer);
        }
    }

    pub fn open_settings(&mut self) {
        self.app.open_settings();
        self.command_palette.close();
    }

    pub fn close_settings(&mut self) {
        self.app.close_settings();
    }

    pub fn install_workspace(
        &mut self,
        root: String,
        tree: Vec<FileNode>,
        config: WorkspaceConfig,
        metadata: WorkspaceMetadata,
    ) {
        self.app
            .install_workspace(root, tree, config, metadata);
        self.workspace_collapsed.clear();
        if let Some(nodes) = self.app.editor().workspace_tree() {
            let mut directories = Vec::new();
            collect_directory_paths(nodes, &mut directories);
            for path in directories {
                self.workspace_collapsed.insert(path);
            }
        }
        self.workspace_collapsed_version = self.workspace_collapsed_version.wrapping_add(1);
        if let Err(err) = self.refresh_debug_targets() {
            self.set_error(Some(err));
        }
        self.sync_buffer_from_editor();
    }

    pub fn workspace_recent_files(&self) -> Vec<String> {
        self.app.workspace_recent_files()
    }

    pub fn record_recent_workspace_file(&mut self) -> Option<(String, WorkspaceConfig)> {
        self.app.record_recent_workspace_file()
    }

    pub fn apply_workspace_config_saved(&mut self, root: String) {
        self.app.apply_workspace_config_saved(root);
    }

    pub fn apply_workspace_metadata_saved(&mut self, root: String) {
        self.app.apply_workspace_metadata_saved(root);
    }

    pub fn take_workspace_metadata_payload(&mut self) -> Option<(String, WorkspaceMetadata)> {
        self.app.take_workspace_metadata_payload()
    }

    pub fn active_sticky_notes(&self) -> Vec<StickyNote> {
        self.app.active_sticky_notes()
    }

    pub fn add_sticky_note_at_cursor(&mut self) -> Result<(), String> {
        if self.app.editor().workspace_root().is_none() {
            return Err("Sticky notes require an open workspace".to_string());
        }

        let (line_idx, byte_offset) = self.buffer_content.cursor_position();
        let line_number = line_idx.saturating_add(1);
        let column = self
            .buffer_content
            .line(line_idx)
            .map(|line| {
                let clamped = byte_offset.min(line.len());
                line[..clamped].chars().count().saturating_add(1)
            })
            .unwrap_or(1);

        self
            .app
            .editor_mut()
            .add_sticky_note(line_number, column, String::new())
            .map(|_| ())
            .ok_or_else(|| "Unable to add sticky note".to_string())
    }

    pub fn update_sticky_note_content(&mut self, id: u64, content: String) {
        self.app.update_sticky_note_content(id, content);
    }

    pub fn remove_sticky_note(&mut self, id: u64) {
        self.app.remove_sticky_note(id);
    }

    pub fn settings(&self) -> &SettingsState {
        self.app.settings()
    }

    pub fn settings_mut(&mut self) -> &mut SettingsState {
        self.app.settings_mut()
    }

    pub fn clear_binding_error(&mut self, id: QuickCommandId) {
        self.app.clear_binding_error(id);
    }

    pub fn apply_quick_command_binding(
        &mut self,
        id: QuickCommandId,
    ) -> Result<(), String> {
        self.app.apply_quick_command_binding(id)
    }

    pub fn settings_dirty(&self) -> bool {
        self.app.settings_dirty()
    }

    pub fn keymap_save_payload(&self) -> Result<(String, String), String> {
        self.app.keymap_save_payload()
    }

    pub fn mark_keymap_saved(&mut self, path: String) {
        self.app.mark_keymap_saved(path);
    }

    pub fn apply_selected_keymap_path(&mut self, path: String) -> Result<(), String> {
        self.app.apply_selected_keymap_path(path)
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

    pub fn debugger(&self) -> &DebuggerState {
        &self.debugger
    }

    pub fn debugger_mut(&mut self) -> &mut DebuggerState {
        &mut self.debugger
    }

    pub fn refresh_debug_targets(&mut self) -> Result<(), String> {
        let root = self.app.editor().workspace_root();
        self.debugger.refresh_targets(root)
    }

    pub fn debugger_menu_open(&self) -> bool {
        self.debugger.menu_open()
    }

    pub fn toggle_debugger_menu(&mut self) {
        self.debugger.toggle_menu();
    }

    pub fn close_debugger_menu(&mut self) {
        self.debugger.close_menu();
    }

    pub fn set_debug_target_selected(&mut self, id: u64, selected: bool) {
        self.debugger.set_target_selected(id, selected);
    }

    pub fn set_debug_target_filter(&mut self, value: String) {
        self.debugger.set_target_filter(value);
    }

    pub fn commit_manual_debug_target(&mut self) -> Result<(), String> {
        self.debugger.commit_manual_target()
    }

    pub fn commit_breakpoint_from_draft(&mut self) -> Result<(), String> {
        self.debugger.commit_breakpoint_from_draft()
    }

    pub fn toggle_breakpoint(&mut self, id: u64) {
        self.debugger.toggle_breakpoint(id);
    }

    pub fn remove_breakpoint(&mut self, id: u64) {
        self.debugger.remove_breakpoint(id);
    }

    pub fn set_breakpoint_condition(&mut self, id: u64, condition: String) {
        self.debugger.set_breakpoint_condition(id, condition);
    }

    pub fn prepare_debug_launches(&mut self) -> Result<Vec<DebugLaunchPlan>, String> {
        self.debugger.prepare_launches()
    }

    pub fn begin_debug_launch(&mut self, target: &DebugTarget) {
        self.debugger.begin_launch_for(target);
    }

    pub fn stop_debug_session(&mut self) {
        self.debugger.stop_session();
    }

    pub fn submit_gdb_command(&mut self) -> Result<(), String> {
        self.debugger.submit_gdb_command()
    }

    pub fn attach_debugger_session(&mut self, session: GdbSession) {
        self.debugger.attach_runtime(session);
    }

    pub fn debugger_has_runtime(&self) -> bool {
        self.debugger.has_runtime()
    }

    pub fn process_debugger_events(&mut self) -> Vec<DebuggerUiEvent> {
        self.debugger.process_runtime_events()
    }

    pub fn push_notification(&mut self, request: NotificationRequest) {
        self.notifications.notify(request);
    }

    pub fn notifications(&self) -> &[Notification] {
        self.notifications.notifications()
    }

    pub fn has_notifications(&self) -> bool {
        self.notifications.has_active()
    }

    pub fn dismiss_notification(&mut self, id: u64) {
        self.notifications.dismiss(id);
    }

    pub fn tick_notifications(&mut self, delta: Duration) {
        self.notifications.tick(delta);
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
        let editor = self.app.editor();
        let index = editor.active_index();
        editor.active_document().map(|doc| {
            let key = doc
                .fingerprint
                .map(DocumentKey::Fingerprint)
                .unwrap_or(DocumentKey::Index(index));
            (key, doc.language())
        })
    }
}
