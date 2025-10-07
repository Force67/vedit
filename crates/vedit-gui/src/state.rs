use crate::console::{ConsoleKind, ConsoleLineKind, ConsoleState};
use crate::debugger::{DebugLaunchPlan, DebuggerConsoleEntry, DebuggerState, DebuggerUiEvent, DebugTarget};
use crate::notifications::{Notification, NotificationCenter, NotificationRequest};
use crate::scaling;
use crate::syntax::{DocumentKey, SyntaxSettings, SyntaxSystem};
use crate::widgets::file_explorer::FileExplorer;
use crate::widgets::text_editor::{buffer_scroll_metrics, scroll_to, ScrollMetrics};
use iced::keyboard;
use iced::widget::text_editor::{Action as TextEditorAction, Content};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use vedit_application::{AppState, CommandPaletteState, QuickCommand, QuickCommandId, SettingsState};
use vedit_core::{Editor, FileNode, KeyEvent, Language, StickyNote, WorkspaceConfig};
use vedit_make::Makefile;
use vedit_vs::{Solution as VsSolution, VcxProject};

use crate::commands::DebugSession;
use crate::message::RightRailTab;
use vedit_config::WorkspaceMetadata;

const IGNORED_DIRECTORIES: [&str; 4] = ["target", ".git", ".hg", ".svn"];

#[derive(Debug, Clone)]
pub struct SolutionTreeNode {
    pub name: String,
    pub path: Option<String>,
    pub is_directory: bool,
    pub children: Vec<SolutionTreeNode>,
}

#[derive(Debug, Clone)]
pub struct VisualStudioProjectEntry {
    pub name: String,
    pub path: String,
    pub files: Vec<SolutionTreeNode>,
    pub load_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct VisualStudioSolutionEntry {
    pub name: String,
    pub path: String,
    pub projects: Vec<VisualStudioProjectEntry>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct MakefileEntry {
    pub name: String,
    pub path: String,
    pub files: Vec<SolutionTreeNode>,
}

#[derive(Debug, Clone)]
pub struct SolutionErrorEntry {
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub enum SolutionBrowserEntry {
    VisualStudio(VisualStudioSolutionEntry),
    Makefile(MakefileEntry),
    Error(SolutionErrorEntry),
}

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

#[derive(Debug, Clone, Copy)]
pub enum ResizeDirection {
    Right,
    Bottom,
    Both,
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
    file_explorer: Option<FileExplorer>,
    solution_browser: Vec<SolutionBrowserEntry>,
    pub recent_files: Vec<String>,
    zoom_config: ZoomConfig,
    modifiers: keyboard::Modifiers,
    debugger: DebuggerState,
    console: ConsoleState,
    active_debug_console: Option<u64>,
    debug_console_counter: u32,
    notifications: NotificationCenter,
    selected_right_rail_tab: RightRailTab,
    pub current_window_size: iced::Size,
    pub is_maximized: bool,
    pub previous_size: Option<iced::Size>,
    pub resize_start_pos: Option<iced::Point>,
    pub resize_start_size: Option<iced::Size>,
    pub resize_direction: Option<ResizeDirection>,
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
            file_explorer: None,
            solution_browser: Vec::new(),
            recent_files: vec![],
            zoom_config,
            modifiers: keyboard::Modifiers::default(),
            debugger: DebuggerState::default(),
            console: ConsoleState::new(),
            active_debug_console: None,
            debug_console_counter: 0,
            notifications: NotificationCenter::new(),
            selected_right_rail_tab: RightRailTab::Workspace,
            current_window_size: iced::Size::new(800.0, 600.0),
            is_maximized: false,
            previous_size: None,
            resize_start_pos: None,
            resize_start_size: None,
            resize_direction: None,
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

    pub fn console(&self) -> &ConsoleState {
        &self.console
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

    pub fn toggle_console_visibility(&mut self) -> Result<(), String> {
        if self.console.is_visible() {
            self.console.set_visible(false);
            self.notify_console_metadata_changed();
            return Ok(());
        }

        if self.console.tabs().is_empty() {
            self.console.spawn_shell_tab()?;
        }
        self.console.set_visible(true);
        self.notify_console_metadata_changed();
        Ok(())
    }

    pub fn create_console_tab(&mut self) -> Result<(), String> {
        self.console.spawn_shell_tab()?;
        self.console.set_visible(true);
        self.notify_console_metadata_changed();
        Ok(())
    }

    pub fn select_console_tab(&mut self, id: u64) {
        if self.console.select_tab(id) {
            self.notify_console_metadata_changed();
        }
    }

    pub fn set_console_input(&mut self, id: u64, value: String) {
        self.console.set_input(id, value);
    }

    pub fn submit_console_input(&mut self, id: u64) -> Result<(), String> {
        self.console.submit_input(id)
    }

    pub fn process_console_events(&mut self) {
        self.console.process_events();
    }

    pub fn selected_right_rail_tab(&self) -> RightRailTab {
        self.selected_right_rail_tab
    }

    pub fn set_selected_right_rail_tab(&mut self, tab: RightRailTab) {
        self.selected_right_rail_tab = tab;
    }

    pub fn file_explorer(&self) -> Option<&FileExplorer> {
        self.file_explorer.as_ref()
    }

    pub fn file_explorer_mut(&mut self) -> Option<&mut FileExplorer> {
        self.file_explorer.as_mut()
    }

    pub fn set_file_explorer(&mut self, explorer: Option<FileExplorer>) {
        self.file_explorer = explorer;
    }

    pub fn refresh_file_explorer(&mut self) {
        let Some(root) = self.app.editor().workspace_root() else {
            self.file_explorer = None;
            return;
        };

        let mut explorer_root = PathBuf::from(root);

        if explorer_root.is_file() {
            if let Some(parent) = explorer_root.parent() {
                explorer_root = parent.to_path_buf();
            }
        }

        self.file_explorer = Some(FileExplorer::new(explorer_root));
    }

    pub fn workspace_solutions(&self) -> &[SolutionBrowserEntry] {
        &self.solution_browser
    }

    pub fn refresh_solution_browser(&mut self) -> Result<(), String> {
        self.solution_browser.clear();

        let Some(root) = self.app.editor().workspace_root() else {
            return Ok(());
        };

        let root_path = PathBuf::from(root);
        let mut solution_paths = Vec::new();
        let mut makefile_paths = Vec::new();
        let mut warnings = Vec::new();

        scan_workspace_artifacts(&root_path, &mut solution_paths, &mut makefile_paths, &mut warnings);

        solution_paths.sort();
        solution_paths.dedup();
        makefile_paths.sort();
        makefile_paths.dedup();

        let mut entries = Vec::new();

        for warning in warnings {
            entries.push(SolutionBrowserEntry::Error(SolutionErrorEntry {
                path: root_path.to_string_lossy().to_string(),
                message: warning,
            }));
        }

        for path in solution_paths {
            match VsSolution::from_path(&path) {
                Ok(solution) => {
                    entries.push(SolutionBrowserEntry::VisualStudio(convert_solution(solution)));
                }
                Err(err) => {
                    entries.push(SolutionBrowserEntry::Error(SolutionErrorEntry {
                        path: path.to_string_lossy().to_string(),
                        message: err.to_string(),
                    }));
                }
            }
        }

        for path in makefile_paths {
            match Makefile::from_path(&path) {
                Ok(makefile) => {
                    entries.push(SolutionBrowserEntry::Makefile(convert_makefile(makefile)));
                }
                Err(err) => {
                    entries.push(SolutionBrowserEntry::Error(SolutionErrorEntry {
                        path: path.to_string_lossy().to_string(),
                        message: err.to_string(),
                    }));
                }
            }
        }

        entries.sort_by(|a, b| {
            let (order_a, name_a) = solution_entry_sort_key(a);
            let (order_b, name_b) = solution_entry_sort_key(b);
            order_a
                .cmp(&order_b)
                .then_with(|| name_a.cmp(&name_b))
        });

        self.solution_browser = entries;
        Ok(())
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
        let recent_targets = self.app.workspace_recent_debug_targets();
        let last_target = self.app.workspace_last_debug_target();
        self.debugger
            .set_recent_target_history(recent_targets, last_target);
        if let Err(err) = self.restore_console_from_metadata() {
            self.set_error(Some(err));
        }
        self.workspace_collapsed.clear();
        // Don't collapse directories by default - let users expand them manually
        // if let Some(nodes) = self.app.editor().workspace_tree() {
        //     let mut directories = Vec::new();
        //     collect_directory_paths(nodes, &mut directories);
        //     for path in directories {
        //         self.workspace_collapsed.insert(path);
        //     }
        // }
        self.workspace_collapsed_version = self.workspace_collapsed_version.wrapping_add(1);
        if let Err(err) = self.refresh_debug_targets() {
            self.set_error(Some(err));
        }
        if let Err(err) = self.refresh_solution_browser() {
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
        let result = self.debugger.refresh_targets(root);
        self.drain_debugger_console_updates();
        result
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

    pub fn begin_debug_launch(
        &mut self,
        target: &DebugTarget,
    ) -> Option<(String, WorkspaceConfig)> {
        self.debug_console_counter = self.debug_console_counter.wrapping_add(1);
        let title = format!("Debug {}: {}", self.debug_console_counter, target.name);
        let tab_id = self.console.create_debug_tab(title);
        self.console.set_visible(true);
        self.active_debug_console = Some(tab_id);
        self.debugger.begin_launch_for(target);
        self.drain_debugger_console_updates();
        self.notify_console_metadata_changed();
        self.app
            .record_recent_debug_target(&target.name, &target.executable)
    }

    pub fn stop_debug_session(&mut self) {
        self.debugger.stop_session();
        self.drain_debugger_console_updates();
    }

    pub fn submit_command(&mut self) -> Result<(), String> {
        let result = self.debugger.submit_command();
        self.drain_debugger_console_updates();
        result
    }

    pub fn attach_debugger_session(&mut self, session: DebugSession) {
        match session {
            DebugSession::Gdb(sess) => self.debugger.attach_gdb_runtime(sess),
            DebugSession::Vedit(sess) => self.debugger.attach_vedit_runtime(sess),
        }
    }

    pub fn debugger_has_runtime(&self) -> bool {
        self.debugger.has_runtime()
    }

    pub fn process_debugger_events(&mut self) -> Vec<DebuggerUiEvent> {
        let events = self.debugger.process_runtime_events();
        self.drain_debugger_console_updates();
        events
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

    fn drain_debugger_console_updates(&mut self) {
        let entries = self.debugger.take_console_updates();
        if entries.is_empty() {
            return;
        }

        self.push_debug_console_entries(entries);
    }

    fn push_debug_console_entries(&mut self, entries: Vec<DebuggerConsoleEntry>) {
        if entries.is_empty() {
            return;
        }

        if self.active_debug_console.is_none() {
            self.debug_console_counter = self.debug_console_counter.wrapping_add(1);
            let title = format!("Debug {}", self.debug_console_counter);
            let tab_id = self.console.create_debug_tab(title);
            self.console.set_visible(true);
            self.active_debug_console = Some(tab_id);
        }

        if let Some(tab_id) = self.active_debug_console {
            let mapped: Vec<(ConsoleLineKind, String)> = entries
                .into_iter()
                .flat_map(|entry| Self::map_debug_entry(entry))
                .collect();
            if !mapped.is_empty() {
                self.console.push_lines(tab_id, mapped);
            }
        }

        self.notify_console_metadata_changed();
    }

    fn map_debug_entry(entry: DebuggerConsoleEntry) -> Vec<(ConsoleLineKind, String)> {
        let kind = match entry.kind {
            crate::debugger::DebuggerConsoleEntryKind::Command => ConsoleLineKind::Command,
            crate::debugger::DebuggerConsoleEntryKind::Output => ConsoleLineKind::Output,
            crate::debugger::DebuggerConsoleEntryKind::Error => ConsoleLineKind::Error,
            crate::debugger::DebuggerConsoleEntryKind::Info => ConsoleLineKind::Info,
        };

        if entry.message.is_empty() {
            return vec![(kind, String::new())];
        }

        entry
            .message
            .split('\n')
            .map(|line| (kind, line.to_string()))
            .collect()
    }

    fn restore_console_from_metadata(&mut self) -> Result<(), String> {
        self.console = ConsoleState::new();
        self.active_debug_console = None;
        self.debug_console_counter = 0;

        let Some(metadata) = self.app.editor().workspace_metadata() else {
            self.console.set_visible(false);
            return Ok(());
        };

        for _ in 0..metadata.console.shell_tabs {
            self.console.spawn_shell_tab()?;
        }

        if let Some(active_shell) = metadata.console.active_shell {
            if active_shell < self.console.shell_tab_count() {
                self.console.select_shell_at(active_shell);
            }
        }

        self.console.set_visible(metadata.console.visible);
        Ok(())
    }

    fn notify_console_metadata_changed(&mut self) {
        if self.app.editor().workspace_root().is_none() {
            return;
        }

        let visible = self.console.is_visible();
        let shell_ids: Vec<u64> = self
            .console
            .tabs()
            .iter()
            .filter(|tab| tab.kind() == ConsoleKind::Shell)
            .map(|tab| tab.id())
            .collect();
        let shell_count = shell_ids.len();
        let mut active_shell = self
            .console
            .active_tab_id()
            .and_then(|id| shell_ids.iter().position(|tab_id| *tab_id == id));

        let editor = self.app.editor_mut();
        let mut mark_dirty = false;
        {
            if let Some(metadata) = editor.workspace_metadata_mut() {
                let previous_active = metadata.console.active_shell;
                if active_shell.is_none() {
                    if let Some(prev) = previous_active {
                        if prev < shell_count {
                            active_shell = Some(prev);
                        }
                    }
                }

                metadata.console.visible = visible;
                metadata.console.shell_tabs = shell_count;
                metadata.console.active_shell = active_shell;
                mark_dirty = true;
            }
        }

        if mark_dirty {
            editor.mark_workspace_metadata_dirty();
        }
    }

}

fn solution_entry_sort_key(entry: &SolutionBrowserEntry) -> (u8, String) {
    match entry {
        SolutionBrowserEntry::VisualStudio(solution) => (0, solution.name.clone()),
        SolutionBrowserEntry::Makefile(makefile) => (1, makefile.name.clone()),
        SolutionBrowserEntry::Error(error) => (2, error.path.clone()),
    }
}

fn scan_workspace_artifacts(
    root: &Path,
    solutions: &mut Vec<PathBuf>,
    makefiles: &mut Vec<PathBuf>,
    warnings: &mut Vec<String>,
) {
    let read_dir = match fs::read_dir(root) {
        Ok(read_dir) => read_dir,
        Err(err) => {
            warnings.push(format!("Unable to read {}: {}", root.display(), err));
            return;
        }
    };

    for entry in read_dir {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                warnings.push(format!("Failed to read directory entry: {}", err));
                continue;
            }
        };

        let path = entry.path();
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(err) => {
                warnings.push(format!(
                    "Failed to resolve file type for {}: {}",
                    path.display(),
                    err
                ));
                continue;
            }
        };

        if file_type.is_dir() {
            if should_ignore_directory(&path) {
                continue;
            }
            scan_workspace_artifacts(&path, solutions, makefiles, warnings);
            continue;
        }

        if !file_type.is_file() {
            continue;
        }

        if is_solution_file(&path) {
            solutions.push(path.clone());
            continue;
        }

        if is_makefile(&path) {
            makefiles.push(path);
        }
    }
}

fn should_ignore_directory(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(OsStr::to_str) else {
        return false;
    };

    IGNORED_DIRECTORIES
        .iter()
        .any(|ignored| name.eq_ignore_ascii_case(ignored))
}

fn is_solution_file(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .map(|ext| ext.eq_ignore_ascii_case("sln"))
        == Some(true)
}

fn is_makefile(path: &Path) -> bool {
    if let Some(ext) = path.extension().and_then(OsStr::to_str) {
        if ext.eq_ignore_ascii_case("mk") {
            return true;
        }
    }

    path.file_name()
        .and_then(OsStr::to_str)
        .map(|name| name.eq_ignore_ascii_case("makefile"))
        == Some(true)
}

fn convert_solution(solution: VsSolution) -> VisualStudioSolutionEntry {
    let mut warnings = Vec::new();
    let mut projects = Vec::new();

    for project in solution.projects {
        let path = project
            .absolute_path
            .to_string_lossy()
            .to_string();
        let load_error = project.load_error.clone();

        if let Some(ref err) = load_error {
            warnings.push(format!("{}: {}", project.name, err));
        }

        let files = project
            .project
            .map(|vcx| build_vcx_tree(&vcx))
            .unwrap_or_default();

        projects.push(VisualStudioProjectEntry {
            name: project.name,
            path,
            files,
            load_error,
        });
    }

    projects.sort_by(|a, b| a.name.cmp(&b.name));

    VisualStudioSolutionEntry {
        name: solution.name,
        path: solution.path.to_string_lossy().to_string(),
        projects,
        warnings,
    }
}

fn convert_makefile(makefile: Makefile) -> MakefileEntry {
    let mut files = build_tree_from_paths(
        makefile
            .files
            .iter()
            .map(|item| (item.include.clone(), item.full_path.to_string_lossy().to_string())),
    );
    sort_solution_nodes(&mut files);

    MakefileEntry {
        name: makefile.name,
        path: makefile.path.to_string_lossy().to_string(),
        files,
    }
}

fn build_vcx_tree(project: &VcxProject) -> Vec<SolutionTreeNode> {
    let mut nodes = build_tree_from_paths(
        project
            .files
            .iter()
            .map(|item| (item.include.clone(), item.full_path.to_string_lossy().to_string())),
    );
    sort_solution_nodes(&mut nodes);
    nodes
}

fn build_tree_from_paths<I>(paths: I) -> Vec<SolutionTreeNode>
where
    I: Iterator<Item = (PathBuf, String)>,
{
    let mut roots = Vec::new();

    for (path, full_path) in paths {
        let mut components: Vec<String> = path
            .components()
            .filter_map(|component| match component {
                std::path::Component::Normal(part) => part.to_str().map(|value| value.to_string()),
                _ => None,
            })
            .collect();

        if components.is_empty() {
            if let Some(name) = Path::new(&full_path)
                .file_name()
                .and_then(|part| part.to_str())
            {
                components.push(name.to_string());
            }
        }

        if components.is_empty() {
            continue;
        }

        insert_tree_node(&mut roots, &components, Some(full_path));
    }

    roots
}

fn insert_tree_node(nodes: &mut Vec<SolutionTreeNode>, components: &[String], path: Option<String>) {
    if components.is_empty() {
        return;
    }

    let name = &components[0];
    let is_last = components.len() == 1;

    let mut node = nodes
        .iter_mut()
        .find(|candidate| candidate.name == *name);

    if node.is_none() {
        nodes.push(SolutionTreeNode {
            name: name.clone(),
            path: if is_last { path.clone() } else { None },
            is_directory: !is_last,
            children: Vec::new(),
        });
        node = nodes.iter_mut().find(|candidate| candidate.name == *name);
    }

    if let Some(node) = node {
        if is_last {
            if path.is_some() {
                node.path = path.clone();
            }
            node.is_directory = node.is_directory || path.is_none();
        } else {
            node.is_directory = true;
            insert_tree_node(&mut node.children, &components[1..], path);
        }
    }
}

fn sort_solution_nodes(nodes: &mut [SolutionTreeNode]) {
    nodes.sort_by(|a, b| match (a.is_directory, b.is_directory) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        _ => a.name.cmp(&b.name),
    });

    for node in nodes.iter_mut() {
        sort_solution_nodes(&mut node.children);
    }
}
