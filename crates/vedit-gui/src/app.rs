use crate::commands::{
    self, DebugSessionBreakpoint, DebugSessionRequest, SaveDocumentRequest, SaveKeymapRequest,
    WorkspaceData,
};
use crate::debugger::{DebugLaunchPlan, DebuggerType, DebuggerUiEvent};
use crate::keyboard;
use crate::message::Message;
use crate::state::EditorState;
use crate::views;
use crate::notifications::{NotificationKind, NotificationRequest};
use iced::Subscription;
use iced::{event, mouse, window, Application, Command, Element, executor, theme, time, Settings};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use vedit_core::{Document, Key, QUICK_COMMAND_MENU_ACTION, SAVE_ACTION};
use vedit_application::QuickCommandId;

// Global refresh rate configuration
pub static REFRESH_RATE_CONFIG: std::sync::LazyLock<RefreshRateConfig> = std::sync::LazyLock::new(|| {
    RefreshRateConfig::new()
});

#[derive(Debug, Clone)]
pub struct RefreshRateConfig {
    pub highest_refresh_rate: Arc<Mutex<f32>>, // in Hz
    pub current_monitor_refresh: Arc<Mutex<f32>>, // in Hz
}

impl RefreshRateConfig {
    pub fn new() -> Self {
        Self {
            highest_refresh_rate: Arc::new(Mutex::new(60.0)), // Default fallback
            current_monitor_refresh: Arc::new(Mutex::new(60.0)), // Default fallback
        }
    }

    pub fn get_optimal_frame_duration(&self) -> Duration {
        let refresh_rate = *self.highest_refresh_rate.lock().unwrap();
        // Convert Hz to milliseconds, aiming for slightly higher than refresh rate
        let frame_duration_ms = (1000.0 / refresh_rate) * 0.9; // 90% of refresh duration
        Duration::from_millis(frame_duration_ms as u64)
    }

    pub fn get_target_fps(&self) -> f32 {
        *self.highest_refresh_rate.lock().unwrap()
    }

    pub fn set_refresh_rates(&self, highest: f32, current: f32) {
        *self.highest_refresh_rate.lock().unwrap() = highest.max(current);
        *self.current_monitor_refresh.lock().unwrap() = current;
    }
}

pub fn run() -> iced::Result {
    let settings = Settings {
        window: window::Settings {
            decorations: false,
            ..Default::default()
        },
        ..Default::default()
    };
    EditorApp::run(settings)
}

struct EditorApp {
    state: EditorState,
}

impl Default for EditorApp {
    fn default() -> Self {
        Self {
            state: EditorState::default(),
        }
    }
}

impl EditorApp {
    fn detect_monitor_refresh_rates(&self) {
        // Try to detect refresh rates using various methods
        let (highest_refresh, current_refresh) = self.get_system_refresh_rates();

        REFRESH_RATE_CONFIG.set_refresh_rates(highest_refresh, current_refresh);

        // Update timing based on detected refresh rates
        self.update_timing_for_refresh_rate(highest_refresh);
    }

    fn get_system_refresh_rates(&self) -> (f32, f32) {
        let mut highest_refresh: f32 = 60.0;
        let mut current_refresh: f32 = 60.0;

        // Method 1: Try environment variable (common for Linux/X11)
        if let Ok(display) = std::env::var("DISPLAY") {
            if let Ok(rate) = self.detect_x11_refresh_rate(&display) {
                current_refresh = rate;
                highest_refresh = highest_refresh.max(rate);
            }
        }

        // Method 2: Try common refresh rate patterns (Windows/Linux)
        let common_rates = [144.0, 165.0, 240.0, 120.0, 75.0, 60.0];
        for &rate in &common_rates {
            if self.test_refresh_rate_feasibility(rate) {
                highest_refresh = highest_refresh.max(rate);
            }
        }

        // Method 3: Check for known high refresh rate indicators
        if std::env::var("GDK_REFRESH_RATE").is_ok() ||
           std::env::var("QT_SCALE_FACTOR").is_ok() {
            // Likely a modern system that supports high refresh rates
            highest_refresh = highest_refresh.max(144.0);
        }

        (highest_refresh, current_refresh)
    }

    fn detect_x11_refresh_rate(&self, display: &str) -> Result<f32, Box<dyn std::error::Error>> {
        // Try to parse refresh rate from xrandr output
        use std::process::Command;

        let output = Command::new("xrandr")
            .arg("--query")
            .output()?;

        let output_str = String::from_utf8_lossy(&output.stdout);

        for line in output_str.lines() {
            if line.contains("connected") && line.contains(display) {
                // Look for refresh rate pattern like "144.00Hz" or "144Hz"
                if let Some(rate_str) = line.split_whitespace()
                    .find(|s| s.ends_with("Hz") || s.ends_with("hz")) {
                    let rate_num = rate_str.trim_end_matches("Hz").trim_end_matches("hz");
                    if let Ok(rate) = rate_num.parse::<f32>() {
                        return Ok(rate);
                    }
                }
            }
        }

        // Fallback: try to get current mode
        for line in output_str.lines() {
            if line.contains("*") && (line.contains("144") || line.contains("165") || line.contains("240")) {
                if let Some(rate_str) = line.split_whitespace()
                    .find(|s| s.contains("Hz") || s.contains("hz")) {
                    let rate_num_clean = rate_str.replace("Hz", "").replace("hz", "").trim().to_string();
                    if let Ok(rate) = rate_num_clean.parse::<f32>() {
                        return Ok(rate);
                    }
                }
            }
        }

        Err("Could not detect refresh rate".into())
    }

    fn test_refresh_rate_feasibility(&self, rate: f32) -> bool {
        // Simple feasibility test based on system capabilities
        // This is a heuristic approach
        let frame_time_ms = 1000.0 / rate;

        // If the system can handle sub-10ms frame times, it's likely capable
        frame_time_ms >= 4.0 && frame_time_ms <= 50.0
    }

    fn update_timing_for_refresh_rate(&self, refresh_rate: f32) {
        // This will be used to dynamically update the application timing
        // The actual implementation will update the subscription timing
        println!("Detected refresh rate: {:.0} Hz - Optimizing timing", refresh_rate);
    }
}

impl Application for EditorApp {
    type Executor = executor::Default;
    type Message = Message;
    type Theme = theme::Theme;
    type Flags = ();

    fn new(_flags: Self::Flags) -> (Self, Command<Self::Message>) {
        let app = Self::default();
        // Trigger refresh rate detection at startup
        let command = Command::perform(async {}, |_| Message::DetectMonitorRefreshRates);
        (app, command)
    }

    fn title(&self) -> String {
        "vedit".into()
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        let debugger_events = self.state.process_debugger_events();
        self.state.process_console_events();
        self.handle_debugger_events(debugger_events);
        match message {
            Message::OpenFileRequested => {
                return self.wrap_command(Command::perform(
                    commands::pick_document(),
                    Message::FileLoaded,
                ));
            }
            Message::FileLoaded(result) => match result {
                Ok(Some(document)) => {
                    self.state.editor_mut().open_document(document);
                    self.state.clear_error();
                    self.state.sync_buffer_from_editor();
                    if let Some((root, config)) = self.state.record_recent_workspace_file() {
                return self.wrap_command(Command::perform(
                    commands::save_workspace_config(root, config),
                    Message::WorkspaceConfigSaved,
                ));
                    }
                }
                Ok(None) => {
                    // user cancelled dialog
                }
                Err(err) => {
                    self.state.set_error(Some(err));
                }
            },
            Message::DocumentSelected(index) => {
                self.state.editor_mut().set_active(index);
                self.state.sync_buffer_from_editor();
            }
            Message::WorkspaceOpenRequested => {
                return self.wrap_command(Command::perform(
                    commands::pick_workspace(),
                    Message::WorkspaceLoaded,
                ));
            }
            Message::SolutionOpenRequested => {
                return self.wrap_command(Command::perform(
                    commands::pick_solution(),
                    Message::SolutionLoaded,
                ));
            }
            Message::SolutionSelected(path) => {
                return self.wrap_command(Command::perform(
                    commands::load_solution_from_path(path),
                    Message::SolutionLoaded,
                ));
            }
            Message::WorkspaceLoaded(result) => match result {
                Ok(Some(WorkspaceData {
                    root,
                    tree,
                    config,
                    metadata,
                })) => {
                    self.state
                        .install_workspace(root.clone(), tree, config, metadata);
                    self.state.refresh_file_explorer();
                    self.state.clear_error();
                }
                Ok(None) => {
                    // user cancelled dialog
                }
                Err(err) => {
                    self.state.set_error(Some(err));
                }
            },
            Message::SolutionLoaded(result) => match result {
                Ok(Some(WorkspaceData {
                    root,
                    tree,
                    config,
                    metadata,
                })) => {
                    self.state
                        .install_workspace(root.clone(), tree, config, metadata);
                    self.state.refresh_file_explorer();
                    self.state.clear_error();
                }
                Ok(None) => {}
                Err(err) => {
                    self.state.set_error(Some(err));
                }
            },
            Message::WorkspaceFileActivated(path) => {
                // Add to recent files
                self.state.recent_files.retain(|p| p != &path);
                self.state.recent_files.insert(0, path.clone());
                if self.state.recent_files.len() > 10 {
                    self.state.recent_files.truncate(10);
                }
                return self.wrap_command(Command::perform(
                    commands::load_document_from_path(path),
                    |result| Message::FileLoaded(result.map(Some)),
                ));
            }
            Message::WorkspaceDirectoryToggled(path) => {
                if let Err(err) = self.state.toggle_workspace_directory(path) {
                    self.state.set_error(Some(err));
                }
            }
            Message::BufferAction(action) => {
                self.state.apply_buffer_action(action);
                if let Some((root, metadata)) = self.state.take_workspace_metadata_payload() {
                    return self.wrap_command(Command::perform(
                        commands::save_workspace_metadata(root, metadata),
                        Message::WorkspaceMetadataSaved,
                    ));
                }
            }
            Message::BufferScrollChanged(position) => {
                self.state.set_buffer_scroll(position);
            }
            Message::DocumentSaved(result) => match result {
                Ok(Some(path)) => {
                    self.state.handle_document_saved(Some(path));
                    self.state.clear_error();
                    if let Some((root, metadata)) =
                        self.state.take_workspace_metadata_payload()
                    {
                        return self.wrap_command(Command::perform(
                            commands::save_workspace_metadata(root, metadata),
                            Message::WorkspaceMetadataSaved,
                        ));
                    }
                }
                Ok(None) => {}
                Err(err) => {
                    self.state.set_error(Some(err));
                }
            },
            Message::WorkspaceConfigSaved(result) => match result {
                Ok(root) => {
                    self.state.apply_workspace_config_saved(root);
                }
                Err(err) => {
                    self.state.set_error(Some(err));
                }
            },
            Message::WorkspaceMetadataSaved(result) => match result {
                Ok(root) => {
                    self.state.apply_workspace_metadata_saved(root);
                }
                Err(err) => {
                    self.state.set_error(Some(err));
                }
            },
            Message::StickyNoteCreateRequested => {
                if let Err(err) = self.state.add_sticky_note_at_cursor() {
                    self.state.set_error(Some(err));
                } else {
                    self.state.clear_error();
                }
                if let Some((root, metadata)) = self.state.take_workspace_metadata_payload() {
                    return self.wrap_command(Command::perform(
                        commands::save_workspace_metadata(root, metadata),
                        Message::WorkspaceMetadataSaved,
                    ));
                }
            }
            Message::StickyNoteContentChanged(id, value) => {
                self.state.update_sticky_note_content(id, value);
                if let Some((root, metadata)) = self.state.take_workspace_metadata_payload() {
                    return self.wrap_command(Command::perform(
                        commands::save_workspace_metadata(root, metadata),
                        Message::WorkspaceMetadataSaved,
                    ));
                }
            }
            Message::StickyNoteDeleted(id) => {
                self.state.remove_sticky_note(id);
                if let Some((root, metadata)) = self.state.take_workspace_metadata_payload() {
                        return self.wrap_command(Command::perform(
                            commands::save_workspace_metadata(root, metadata),
                            Message::WorkspaceMetadataSaved,
                        ));
                }
            }
            Message::SettingsOpened => {
                self.state.close_debugger_menu();
                self.state.open_settings();
            }
            Message::SettingsClosed => {
                self.state.close_debugger_menu();
                self.state.close_settings();
            }
            Message::SettingsCategorySelected(category) => {
                self.state.settings_mut().select_category(category);
            }
            Message::SettingsBindingChanged(id, value) => {
                self.state.settings_mut().set_binding_input(id, value);
                self.state.clear_binding_error(id);
            }
            Message::SettingsBindingApplied(id) => {
                if let Err(err) = self.state.apply_quick_command_binding(id) {
                    self.state.set_error(Some(err));
                } else {
                    self.state.clear_error();
                }
            }
            Message::SettingsBindingsSaveRequested => {
                match self.state.keymap_save_payload() {
                    Ok((path, contents)) => {
                        let request = SaveKeymapRequest { path, contents };
                return self.wrap_command(Command::perform(
                    commands::save_keymap(request),
                    Message::SettingsBindingsSaved,
                ));
                    }
                    Err(err) => {
                        self.state.set_error(Some(err));
                    }
                }
            }
            Message::SettingsBindingsSaved(result) => match result {
                Ok(path) => {
                    self.state.mark_keymap_saved(path);
                }
                Err(err) => {
                    self.state.set_error(Some(err));
                }
            },
            Message::SettingsKeymapPathRequested => {
                let current = self.state.keymap_path_display();
                return self.wrap_command(Command::perform(
                    commands::pick_keymap_location(current),
                    Message::SettingsKeymapPathSelected,
                ));
            }
            Message::SettingsKeymapPathSelected(result) => match result {
                Ok(Some(path)) => {
                    if let Err(err) = self.state.apply_selected_keymap_path(path) {
                        self.state.set_error(Some(err));
                    }
                }
                Ok(None) => {}
                Err(err) => {
                    self.state.set_error(Some(err));
                }
            },
            Message::DebuggerTargetsRefreshRequested => {
                if let Err(err) = self.state.refresh_debug_targets() {
                    self.state.set_error(Some(err));
                }
            }
            Message::DebuggerMenuToggled => {
                self.state.toggle_debugger_menu();
            }
            Message::DebuggerTargetToggled(id, selected) => {
                self.state.set_debug_target_selected(id, selected);
            }
            Message::DebuggerTargetFilterChanged(value) => {
                self.state.debugger_mut().set_target_filter(value);
            }
            Message::DebuggerTypeChanged(debugger_type) => {
                self.state.debugger_mut().set_debugger_type(debugger_type);
            }
            Message::DebuggerLaunchRequested => {
                if self.state.debugger_has_runtime() {
                    self.state.stop_debug_session();
                }
                match self.state.prepare_debug_launches() {
                    Ok(plans) => {
                        if let Some(plan) = plans.first() {
                            self.state.clear_error();
                            self.state.close_debugger_menu();
                            let save_payload = self.state.begin_debug_launch(&plan.target);
                            let request = session_request_from_plan(plan, self.state.debugger().debugger_type());
                            let mut commands_list = vec![Command::perform(
                                commands::start_debug_session(request),
                                Message::DebuggerSessionStarted,
                            )];
                            if let Some((root, config)) = save_payload {
                                commands_list.push(Command::perform(
                                    commands::save_workspace_config(root, config),
                                    Message::WorkspaceConfigSaved,
                                ));
                            }
                            return self.wrap_command(Command::batch(commands_list));
                        } else {
                            self.state.set_error(Some("No debug targets selected".to_string()));
                        }
                    }
                    Err(err) => {
                        self.state.set_error(Some(err));
                    }
                }
            }
            Message::DebuggerSessionStarted(result) => match result {
                Ok(session) => {
                    self.state.attach_debugger_session(session);
                    let events = self.state.process_debugger_events();
                    self.handle_debugger_events(events);
                }
                Err(err) => {
                    self.state.set_error(Some(err));
                }
            },
            Message::DebuggerStopRequested => {
                self.state.stop_debug_session();
            }
            Message::DebuggerGdbCommandInputChanged(value) => {
                self.state.debugger_mut().set_command_input(value);
            }
            Message::DebuggerGdbCommandSubmitted => {
                if let Err(err) = self.state.submit_command() {
                    self.state.set_error(Some(err));
                }
            }
            Message::DebuggerBreakpointToggled(id) => {
                self.state.toggle_breakpoint(id);
            }
            Message::DebuggerBreakpointRemoved(id) => {
                self.state.remove_breakpoint(id);
            }
            Message::DebuggerBreakpointConditionChanged(id, value) => {
                self.state.set_breakpoint_condition(id, value);
            }
            Message::DebuggerBreakpointDraftFileChanged(value) => {
                self.state
                    .debugger_mut()
                    .set_breakpoint_draft_file(value);
            }
            Message::DebuggerBreakpointDraftLineChanged(value) => {
                self.state
                    .debugger_mut()
                    .set_breakpoint_draft_line(value);
            }
            Message::DebuggerBreakpointDraftConditionChanged(value) => {
                self.state
                    .debugger_mut()
                    .set_breakpoint_draft_condition(value);
            }
            Message::DebuggerBreakpointDraftSubmitted => {
                match self.state.commit_breakpoint_from_draft() {
                    Ok(()) => self.state.clear_error(),
                    Err(err) => self.state.set_error(Some(err)),
                }
            }
            Message::DebuggerManualTargetNameChanged(value) => {
                self.state
                    .debugger_mut()
                    .set_manual_target_name(value);
            }
            Message::DebuggerManualTargetExecutableChanged(value) => {
                self.state
                    .debugger_mut()
                    .set_manual_target_executable(value);
            }
            Message::DebuggerManualTargetWorkingDirectoryChanged(value) => {
                self.state
                    .debugger_mut()
                    .set_manual_target_working_directory(value);
            }
            Message::DebuggerManualTargetArgumentsChanged(value) => {
                self.state
                    .debugger_mut()
                    .set_manual_target_arguments(value);
            }
            Message::DebuggerManualTargetSaved => {
                match self.state.commit_manual_debug_target() {
                    Ok(()) => self.state.clear_error(),
                    Err(err) => self.state.set_error(Some(err)),
                }
            }
            Message::DebuggerLaunchScriptChanged(value) => {
                self.state.debugger_mut().set_launch_script(value);
            }
            Message::Keyboard(key_event) => {
                match key_event {
                    iced::keyboard::Event::ModifiersChanged(modifiers) => {
                        self.state.set_modifiers(modifiers);
                        return self.wrap_command(Command::none());
                    }
                    iced::keyboard::Event::KeyPressed { modifiers, .. }
                    | iced::keyboard::Event::KeyReleased { modifiers, .. } => {
                        self.state.set_modifiers(modifiers);
                    }
                }

                if let Some(core_event) = keyboard::key_event_from_iced(&key_event) {
                    // Handle Ctrl+F for search (high priority)
                    if core_event.key == Key::Character('F') &&
                       (core_event.ctrl || core_event.command) {
                        self.state.search_dialog_mut().toggle();
                        return self.wrap_command(Command::none());
                    }

                    // Handle Escape key to close search dialog (high priority)
                    if core_event.key == Key::Escape && self.state.search_dialog().is_visible {
                        self.state.search_dialog_mut().hide();
                        return self.wrap_command(Command::none());
                    }

                    // Handle F3 for next match, Shift+F3 for previous match (high priority)
                    if core_event.key == Key::Function(3) {
                        if self.state.search_dialog().is_visible {
                            if core_event.shift {
                                self.state.search_previous();
                            } else {
                                self.state.search_next();
                            }
                            return self.wrap_command(Command::none());
                        }
                    }

                    if self.state.matches_action(QUICK_COMMAND_MENU_ACTION, &core_event) {
                        if self.state.command_palette().is_open() {
                            self.state.close_command_palette();
                        } else {
                            self.state.set_command_palette_query(String::new());
                            self.state.open_command_palette();
                        }
                        return self.wrap_command(Command::none());
                    }

                    for command in self.state.quick_commands() {
                        if let Some(action) = command.action {
                            if self.state.matches_action(action, &core_event) {
                                let cmd = self.execute_quick_command(command.id);
                                return self.wrap_command(cmd);
                            }
                        }
                    }

                    if self.state.matches_action(SAVE_ACTION, &core_event) {
                        let cmd = self.save_active_document();
                        return self.wrap_command(cmd);
                    }

                    if self.state.command_palette().is_open() {
                        match core_event.key {
                            Key::ArrowDown => {
                                self.state.handle_quick_command_navigation(1);
                                return self.wrap_command(Command::none());
                            }
                            Key::ArrowUp => {
                                self.state.handle_quick_command_navigation(-1);
                                return self.wrap_command(Command::none());
                            }
                            Key::Enter => {
                                if let Some(command) = self.state.selected_quick_command() {
                                    self.state.close_command_palette();
                                    let cmd = self.execute_quick_command(command);
                                    return self.wrap_command(cmd);
                                }
                                return self.wrap_command(Command::none());
                            }
                            Key::Escape => {
                                self.state.close_command_palette();
                                return self.wrap_command(Command::none());
                            }
                            _ => {}
                        }
                    }

                    // Handle file explorer keyboard shortcuts when workspace tab is active
                    if self.state.selected_right_rail_tab() == crate::message::RightRailTab::Workspace {
                        if let Some(explorer) = self.state.file_explorer_mut() {
                            match core_event.key {
                                Key::ArrowDown => {
                                    let _ = explorer.update(crate::widgets::file_explorer::Message::FocusNext);
                                    return self.wrap_command(Command::none());
                                }
                                Key::ArrowUp => {
                                    let _ = explorer.update(crate::widgets::file_explorer::Message::FocusPrev);
                                    return self.wrap_command(Command::none());
                                }
                                Key::Enter => {
                                    if let Some(cursor) = explorer.cursor() {
                                        explorer.update(crate::widgets::file_explorer::Message::Open(cursor, crate::widgets::file_explorer::OpenKind::InEditor));
                                    }
                                    return self.wrap_command(Command::none());
                                }
                                Key::Function(2) => {
                                    if let Some(cursor) = explorer.cursor() {
                                        explorer.update(crate::widgets::file_explorer::Message::StartRename(cursor));
                                    }
                                    return self.wrap_command(Command::none());
                                }
                                Key::Delete => {
                                    if let Some(cursor) = explorer.cursor() {
                                        explorer.update(crate::widgets::file_explorer::Message::Delete(cursor));
                                    }
                                    return self.wrap_command(Command::none());
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            Message::MouseWheelScrolled(delta) => {
                let modifiers = self.state.modifiers();
                if !(modifiers.control() || modifiers.command()) {
                    return self.wrap_command(Command::none());
                }

                let delta_y = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => y,
                    mouse::ScrollDelta::Pixels { y, .. } => y,
                };

                if delta_y > 0.0 {
                    self.state.increase_scale_factor();
                } else if delta_y < 0.0 {
                    self.state.decrease_scale_factor();
                }
            }
            Message::CommandPaletteInputChanged(query) => {
                self.state.set_command_palette_query(query);
            }
            Message::CommandPaletteCommandInvoked(command_id) => {
                self.state.close_command_palette();
                let cmd = self.execute_quick_command(command_id);
                return self.wrap_command(cmd);
            }
            Message::CommandPaletteClosed => {
                self.state.close_command_palette();
            }
            Message::CommandPromptToggled => {
                self.state.close_debugger_menu();
                if self.state.command_palette().is_open() {
                    self.state.close_command_palette();
                } else {
                    self.state.set_command_palette_query(String::new());
                    self.state.open_command_palette();
                }
            }
            Message::ConsoleVisibilityToggled => {
                if let Err(err) = self.state.toggle_console_visibility() {
                    self.state.set_error(Some(err));
                }
            }
            Message::ConsoleNewRequested => {
                if let Err(err) = self.state.create_console_tab() {
                    self.state.set_error(Some(err));
                }
            }
            Message::ConsoleTabSelected(id) => {
                self.state.select_console_tab(id);
            }
            Message::ConsoleInputChanged(id, value) => {
                self.state.set_console_input(id, value);
            }
            Message::ConsoleInputSubmitted(id) => {
                if let Err(err) = self.state.submit_console_input(id) {
                    self.state.set_error(Some(err));
                }
            }
            Message::DebuggerTick => {
                self.state.tick_notifications(Duration::from_millis(200));
            }
            Message::FpsUpdate => {
                self.state.update_fps_counter();
                // Reset rapid scroll counter to re-enable syntax highlighting
                self.state.reset_rapid_scroll();
            }
            Message::DetectMonitorRefreshRates => {
                self.detect_monitor_refresh_rates();
            }
            Message::NotificationDismissed(id) => {
                self.state.dismiss_notification(id);
            }
            Message::WindowMinimize => {
                return window::minimize(window::Id::MAIN, true);
            }
            Message::WindowMaximize => {
                if self.state.is_maximized {
                    self.state.is_maximized = false;
                    let mut commands = vec![window::maximize(window::Id::MAIN, false)];
                    if let Some(size) = self.state.previous_size {
                        commands.push(window::resize(window::Id::MAIN, size));
                        self.state.current_window_size = size;
                    }
                    return iced::Command::batch(commands);
                } else {
                    self.state.is_maximized = true;
                    self.state.previous_size = Some(self.state.current_window_size);
                    return window::maximize(window::Id::MAIN, true);
                }
            }
            Message::WindowClose => {
                return window::close(window::Id::MAIN);
            }
            Message::WindowDragStart => {
                return window::drag(window::Id::MAIN);
            }
            Message::WindowResizeStart(pos) => {
                let size = self.state.current_window_size;
                let right = pos.x > size.width - 20.0;
                let bottom = pos.y > size.height - 20.0;
                if right || bottom {
                    self.state.resize_start_pos = Some(pos);
                    self.state.resize_start_size = Some(size);
                    self.state.resize_direction = Some(if right && bottom { crate::state::ResizeDirection::Both } else if right { crate::state::ResizeDirection::Right } else { crate::state::ResizeDirection::Bottom });
                }
            }
            Message::WindowResizeMove(pos) => {
                if let (Some(start_pos), Some(start_size), Some(dir)) = (self.state.resize_start_pos, self.state.resize_start_size, self.state.resize_direction) {
                    let delta = pos - start_pos;
                    let new_width = if matches!(dir, crate::state::ResizeDirection::Right | crate::state::ResizeDirection::Both) { (start_size.width + delta.x).max(200.0) } else { start_size.width };
                    let new_height = if matches!(dir, crate::state::ResizeDirection::Bottom | crate::state::ResizeDirection::Both) { (start_size.height + delta.y).max(100.0) } else { start_size.height };
                    let new_size = iced::Size::new(new_width, new_height);
                    self.state.current_window_size = new_size;
                    return window::resize(window::Id::MAIN, new_size);
                }
            }
            Message::WindowResizeEnd => {
                self.state.resize_start_pos = None;
                self.state.resize_start_size = None;
                self.state.resize_direction = None;
            }
            Message::FileExplorer(msg) => {
                if let crate::widgets::file_explorer::Message::OpenFile(path) = &msg {
                    return self.wrap_command(Command::perform(
                        commands::load_document_from_path(path.clone()),
                        |result| Message::FileLoaded(result.map(Some)),
                    ));
                }

                if let Some(explorer) = self.state.file_explorer_mut() {
                    let command = explorer.update(msg);
                    return self
                        .wrap_command(command.map(Message::FileExplorer));
                }
            }
            Message::RightRailTabSelected(tab) => {
                self.state.set_selected_right_rail_tab(tab);
            }
            // Search dialog messages
            Message::SearchOpen => {
                self.state.search_dialog_mut().show();
            }
            Message::SearchClose => {
                self.state.search_dialog_mut().hide();
            }
            Message::SearchQueryChanged(query) => {
                self.state.update_search_query(query);
            }
            Message::SearchExecute => {
                self.state.execute_search();
            }
            Message::SearchDebounceTick => {
                if self.state.check_search_debounce() {
                    // Search was executed due to debounce
                }
            }
            Message::SearchNext => {
                self.state.search_next();
            }
            Message::SearchPrevious => {
                self.state.search_previous();
            }
            Message::SearchCaseSensitive(case_sensitive) => {
                self.state.search_dialog_mut().set_case_sensitive(case_sensitive);
                self.state.execute_search();
            }
            Message::SearchWholeWord(whole_word) => {
                self.state.search_dialog_mut().set_whole_word(whole_word);
                self.state.execute_search();
            }
            Message::SearchUseRegex(use_regex) => {
                self.state.search_dialog_mut().set_use_regex(use_regex);
                self.state.execute_search();
            }
            Message::SearchToggleReplace => {
                let dialog = self.state.search_dialog_mut();
                if dialog.replace_mode {
                    dialog.disable_replace_mode();
                } else {
                    dialog.enable_replace_mode();
                }
            }
            Message::ReplaceTextChanged(text) => {
                self.state.search_dialog_mut().set_replace_text(text);
            }
            Message::ReplaceOne => {
                self.state.replace_one();
            }
            Message::ReplaceAll => {
                self.state.replace_all();
            }
        }

        self.wrap_command(Command::none())
    }

    fn view(&self) -> Element<'_, Self::Message> {
        views::view(&self.state)
    }

    fn theme(&self) -> Self::Theme {
        theme::Theme::Dark
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        let input = event::listen_with(|event, _status| {
            unsafe {
                static mut CURSOR_POS: iced::Point = iced::Point::ORIGIN;
                match event {
                    event::Event::Keyboard(key_event) => Some(Message::Keyboard(key_event)),
                    event::Event::Mouse(mouse::Event::CursorMoved { position }) => {
                        CURSOR_POS = position;
                        Some(Message::WindowResizeMove(position))
                    }
                    event::Event::Mouse(mouse::Event::ButtonPressed(button)) => {
                        if button == mouse::Button::Left {
                            Some(Message::WindowResizeStart(CURSOR_POS))
                        } else {
                            None
                        }
                    }
                    event::Event::Mouse(mouse::Event::ButtonReleased(button)) => {
                        if button == mouse::Button::Left {
                            Some(Message::WindowResizeEnd)
                        } else {
                            None
                        }
                    }
                    event::Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                        Some(Message::MouseWheelScrolled(delta))
                    }
                    _ => None,
                }
            }
        });

        let tick = time::every(Duration::from_millis(200)).map(|_| Message::DebuggerTick);
        let fps_tick = time::every(Duration::from_millis(8)).map(|_| Message::FpsUpdate); // ~120 FPS for 144Hz monitors
        let debounce_tick = time::every(Duration::from_millis(50)).map(|_| Message::SearchDebounceTick); // Check debounce every 50ms

        Subscription::batch(vec![input, tick, fps_tick, debounce_tick])
    }

    fn scale_factor(&self) -> f64 {
        self.state.scale_factor()
    }
}

fn session_request_from_plan(plan: &DebugLaunchPlan, debugger_type: DebuggerType) -> DebugSessionRequest {
    DebugSessionRequest {
        executable: plan.target.executable.to_string_lossy().to_string(),
        working_directory: plan
            .target
            .working_directory
            .to_string_lossy()
            .to_string(),
        arguments: plan.target.args.clone(),
        breakpoints: plan
            .breakpoints
            .iter()
            .map(|bp| DebugSessionBreakpoint {
                file: bp.file.to_string_lossy().to_string(),
                line: bp.line,
                condition: bp.condition.clone(),
            })
            .collect(),
        launch_script: plan.launch_script.clone(),
        debugger_type,
    }
}

impl EditorApp {
    fn wrap_command(&mut self, command: Command<Message>) -> Command<Message> {
        if let Some((root, metadata)) = self.state.take_workspace_metadata_payload() {
            let save = Command::perform(
                commands::save_workspace_metadata(root, metadata),
                Message::WorkspaceMetadataSaved,
            );
            Command::batch(vec![command, save])
        } else {
            command
        }
    }

    fn execute_quick_command(&mut self, command: QuickCommandId) -> Command<Message> {
        match command {
            QuickCommandId::OpenFile => {
                Command::perform(commands::pick_document(), Message::FileLoaded)
            }
            QuickCommandId::OpenFolder => {
                Command::perform(commands::pick_workspace(), Message::WorkspaceLoaded)
            }
            QuickCommandId::OpenSolution => {
                Command::perform(commands::pick_solution(), Message::SolutionLoaded)
            }
            QuickCommandId::SaveFile => self.save_active_document(),
            QuickCommandId::NewScratchBuffer => {
                let index = self.state.editor_mut().open_document(Document::default());
                self.state.editor_mut().set_active(index);
                self.state.clear_error();
                self.state.sync_buffer_from_editor();
                Command::none()
            }
            QuickCommandId::ShowScaleFactor => {
                let scale_info = self.state.format_scale_factor();
                println!("{}", scale_info);
                self.state.set_error(Some(scale_info));
                Command::none()
            }
            QuickCommandId::AddStickyNote => {
                match self.state.add_sticky_note_at_cursor() {
                    Ok(()) => self.state.clear_error(),
                    Err(err) => self.state.set_error(Some(err)),
                }
                if let Some((root, metadata)) = self.state.take_workspace_metadata_payload() {
                    Command::perform(
                        commands::save_workspace_metadata(root, metadata),
                        Message::WorkspaceMetadataSaved,
                    )
                } else {
                    Command::none()
                }
            }
            QuickCommandId::IncreaseCodeFontZoom => {
                self.state.increase_code_font_zoom();
                Command::none()
            }
        }
    }

    fn save_active_document(&mut self) -> Command<Message> {
        if let Some(doc) = self.state.editor().active_document() {
            let request = SaveDocumentRequest {
                path: doc.path.clone(),
                contents: doc.buffer.to_string(),
                suggested_name: Some(doc.display_name().to_string()),
            };
            Command::perform(commands::save_document(request), Message::DocumentSaved)
        } else {
            Command::none()
        }
    }

    fn handle_debugger_events(&mut self, events: Vec<DebuggerUiEvent>) {
        for event in events {
            match event {
                DebuggerUiEvent::SessionStarted { target } => {
                    let (title, body) = match target {
                        Some(name) => (
                            format!("{} is running", name),
                            format!("Debugger attached to \"{}\" successfully.", name),
                        ),
                        None => (
                            "Debug session started".to_string(),
                            "Debugger attached successfully.".to_string(),
                        ),
                    };
                    let request = NotificationRequest::title(title)
                        .body(body)
                        .kind(NotificationKind::Success);
                    self.state.push_notification(request);
                }
                DebuggerUiEvent::SessionError { message } => {
                    let request = NotificationRequest::title("Debugger error")
                        .body(message)
                        .kind(NotificationKind::Error)
                        .timeout(None);
                    self.state.push_notification(request);
                }
            }
        }
    }
}
