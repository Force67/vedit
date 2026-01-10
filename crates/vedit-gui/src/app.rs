use crate::commands::{
    self, DebugSessionBreakpoint, DebugSessionRequest, SaveDocumentRequest, SaveKeymapRequest,
    WorkspaceData,
};
use std::path::PathBuf;
use crate::debugger::{DebugLaunchPlan, DebuggerType, DebuggerUiEvent};
use crate::keyboard;
use crate::message::Message;
use crate::session::{SessionManager, SessionState, WindowState, WorkspaceState};
use crate::state::EditorState;
use crate::views;
use crate::notifications::{NotificationKind, NotificationRequest};
use iced::Subscription;
use iced::{event, mouse, window, Element, Theme, time, Task};
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
    // Load session state first to get window settings
    let session_manager = SessionManager::new()
        .unwrap_or_else(|e| {
            eprintln!("Failed to initialize session manager: {}", e);
            let temp_dir = std::env::temp_dir().join("vedit");
            std::fs::create_dir_all(&temp_dir).ok();
            SessionManager::with_config_dir(temp_dir)
        });

    let session_state = match session_manager.load_session_state() {
        Ok(state) => {
            println!("DEBUG: Loaded session for window configuration");
            state
        }
        Err(e) => {
            println!("DEBUG: Failed to load session for window config: {}, using defaults", e);
            SessionState::default()
        }
    };

    let window_state = &session_state.window;
    println!("DEBUG: Restoring window to {}x{} at ({}, {}), maximized: {}",
        window_state.width, window_state.height,
        window_state.x, window_state.y,
        window_state.maximized);

    iced::application(EditorApp::new, EditorApp::update, EditorApp::view)
        .title("vedit")
        .subscription(EditorApp::subscription)
        .theme(EditorApp::theme)
        .window_size(iced::Size::new(window_state.width as f32, window_state.height as f32))
        .centered()
        .resizable(true)
        .decorations(false)
        .scale_factor(EditorApp::scale_factor)
        .run()
}

struct EditorApp {
    state: EditorState,
    session_manager: SessionManager,
    main_window_id: window::Id,
}

impl Default for EditorApp {
    fn default() -> Self {
        let session_manager = SessionManager::new()
            .unwrap_or_else(|e| {
                eprintln!("Failed to initialize session manager: {}", e);
                // Create a fallback session manager that uses temp directory
                let temp_dir = std::env::temp_dir().join("vedit");
                std::fs::create_dir_all(&temp_dir).ok();
                println!("DEBUG: Using fallback session dir: {}", temp_dir.display());
                SessionManager::with_config_dir(temp_dir)
            });

        println!("DEBUG: Session manager initialized with config dir: {}", session_manager.config_dir.display());

        Self {
            state: EditorState::default(),
            session_manager,
            main_window_id: window::Id::unique(),
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

impl EditorApp {
    fn new() -> (Self, Task<Message>) {
        let mut app = Self::default();

        // Load session state at startup
        let session_manager = app.session_manager.clone();
        let config_dir = session_manager.config_dir.clone();
        let load_command = Task::perform(
            async move {
                println!("DEBUG: Attempting to load session from: {}", config_dir.display());
                let result = session_manager.load_session_state();
                match &result {
                    Ok(_) => println!("DEBUG: Session loaded successfully"),
                    Err(e) => println!("DEBUG: Failed to load session: {}", e),
                }
                result.map_err(|e| format!("Failed to load session: {}", e))
            },
            Message::SessionLoad,
        );

        // Trigger refresh rate detection at startup
        let refresh_command = Task::perform(async {}, |_| Message::DetectMonitorRefreshRates);

        let combined_command = Task::batch(vec![load_command, refresh_command]);
        (app, combined_command)
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        let debugger_events = self.state.process_debugger_events();
        self.state.process_console_events();
        self.handle_debugger_events(debugger_events);
        match message {
            Message::OpenFileRequested => {
                return self.wrap_command(Task::perform(
                    commands::pick_document(),
                    Message::FileLoaded,
                ));
            }
            Message::FileLoaded(result) => match result {
                Ok(Some(document)) => {
                    let file_path = document.path.clone().unwrap_or_else(|| "unnamed".to_string());
                    println!("DEBUG: File loaded successfully: {}", file_path);
                    self.state.editor_mut().open_document(document);
                    self.state.clear_error();
                    self.state.sync_buffer_from_editor();

                    // Update session state with new open file
                    self.state.update_session_open_files();

                    // Also save session immediately when files change
                    if let Some(session_state) = self.state.get_session_state() {
                        let session_state = session_state.clone();
                        let session_manager = self.session_manager.clone();
                        return self.wrap_command(Task::perform(
                            async move {
                                let result = session_manager.save_session_state(&session_state);
                                match &result {
                                    Ok(()) => println!("DEBUG: Session saved after file open"),
                                    Err(e) => println!("DEBUG: Failed to save session after file open: {}", e),
                                }
                                result.map_err(|e| format!("Failed to save session: {}", e))
                            },
                            Message::SessionSave,
                        ));
                    }

                    // Check if we have additional files to restore
                    let additional_files = self.state.take_pending_files_to_restore();
                    if !additional_files.is_empty() {
                        println!("DEBUG: Loading {} additional files", additional_files.len());
                        return self.wrap_command(Task::perform(
                            async move { additional_files },
                            Message::AdditionalFilesRestoreRequested,
                        ));
                    }

                    if let Some((root, config)) = self.state.record_recent_workspace_file() {
                return self.wrap_command(Task::perform(
                    commands::save_workspace_config(root, config),
                    Message::WorkspaceConfigSaved,
                ));
                    }
                }
                Ok(None) => {
                    // user cancelled dialog
                    // editor_log_debug!("FILE", "File loading cancelled by user");
                }
                Err(err) => {
                    // editor_log_error!("FILE", "Failed to load file: {}", err);
                    self.state.set_error(Some(err));
                }
            },
            Message::DocumentSelected(index) => {
                self.state.editor_mut().set_active(index);
                self.state.sync_buffer_from_editor();
            }
            Message::WorkspaceOpenRequested => {
                return self.wrap_command(Task::perform(
                    commands::pick_workspace(),
                    Message::WorkspaceLoaded,
                ));
            }
            Message::SolutionOpenRequested => {
                return self.wrap_command(Task::perform(
                    commands::pick_solution(),
                    Message::SolutionLoaded,
                ));
            }
            Message::SolutionSelected(path) => {
                return self.wrap_command(Task::perform(
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

                    // Update open files in session state
                    self.state.update_session_open_files();

                    // Trigger file restoration if we have pending files
                    let pending_files = self.state.take_pending_files_to_restore();
                    if !pending_files.is_empty() {
                        println!("DEBUG: Triggering restoration of {} pending files", pending_files.len());
                        return self.wrap_command(Task::perform(
                            async move { pending_files },
                            Message::FilesRestoreRequested,
                        ));
                    }

                    // Save workspace state to session
                    let workspace_state = crate::session::WorkspaceState {
                        workspace_root: Some(std::path::PathBuf::from(&root)),
                        last_folder: Some(std::path::PathBuf::from(&root)),
                        open_files: self.state.get_open_file_paths(),
                        active_file_index: self.state.get_active_file_index(),
                    };

                    // Also save complete session state
                    let session_state = crate::session::SessionState {
                        window: crate::session::WindowState::default(), // TODO: Track actual window state
                        workspace: workspace_state.clone(),
                    };

                    println!("DEBUG: Saving complete session for root: {}", root);
                    let session_manager = self.session_manager.clone();
                    return self.wrap_command(Task::perform(
                        async move {
                            // Save both workspace state and complete session
                            let workspace_result = session_manager.save_workspace_state(&workspace_state);
                            let session_result = session_manager.save_session_state(&session_state);

                            match &workspace_result {
                                Ok(()) => println!("DEBUG: Successfully saved workspace state"),
                                Err(e) => println!("DEBUG: Failed to save workspace state: {}", e),
                            }
                            match &session_result {
                                Ok(()) => println!("DEBUG: Successfully saved complete session"),
                                Err(e) => println!("DEBUG: Failed to save complete session: {}", e),
                            }

                            session_result.map_err(|e| format!("Failed to save session: {}", e))
                        },
                        Message::SessionSave,
                    ));
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

                    // Update open files in session state
                    self.state.update_session_open_files();

                    // Trigger file restoration if we have pending files
                    let pending_files = self.state.take_pending_files_to_restore();
                    if !pending_files.is_empty() {
                        println!("DEBUG: Triggering restoration of {} pending files", pending_files.len());
                        return self.wrap_command(Task::perform(
                            async move { pending_files },
                            Message::FilesRestoreRequested,
                        ));
                    }

                    // Save workspace state to session
                    let workspace_state = crate::session::WorkspaceState {
                        workspace_root: Some(std::path::PathBuf::from(&root)),
                        last_folder: Some(std::path::PathBuf::from(&root)),
                        open_files: self.state.get_open_file_paths(),
                        active_file_index: self.state.get_active_file_index(),
                    };

                    // Also save complete session state
                    let session_state = crate::session::SessionState {
                        window: crate::session::WindowState::default(), // TODO: Track actual window state
                        workspace: workspace_state.clone(),
                    };

                    println!("DEBUG: Saving complete session for root: {}", root);
                    let session_manager = self.session_manager.clone();
                    return self.wrap_command(Task::perform(
                        async move {
                            // Save both workspace state and complete session
                            let workspace_result = session_manager.save_workspace_state(&workspace_state);
                            let session_result = session_manager.save_session_state(&session_state);

                            match &workspace_result {
                                Ok(()) => println!("DEBUG: Successfully saved workspace state"),
                                Err(e) => println!("DEBUG: Failed to save workspace state: {}", e),
                            }
                            match &session_result {
                                Ok(()) => println!("DEBUG: Successfully saved complete session"),
                                Err(e) => println!("DEBUG: Failed to save complete session: {}", e),
                            }

                            session_result.map_err(|e| format!("Failed to save session: {}", e))
                        },
                        Message::SessionSave,
                    ));
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
                return self.wrap_command(Task::perform(
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
                    return self.wrap_command(Task::perform(
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
                        return self.wrap_command(Task::perform(
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
                    return self.wrap_command(Task::perform(
                        commands::save_workspace_metadata(root, metadata),
                        Message::WorkspaceMetadataSaved,
                    ));
                }
            }
            Message::StickyNoteContentChanged(id, value) => {
                self.state.update_sticky_note_content(id, value);
                if let Some((root, metadata)) = self.state.take_workspace_metadata_payload() {
                    return self.wrap_command(Task::perform(
                        commands::save_workspace_metadata(root, metadata),
                        Message::WorkspaceMetadataSaved,
                    ));
                }
            }
            Message::StickyNoteDeleted(id) => {
                self.state.remove_sticky_note(id);
                if let Some((root, metadata)) = self.state.take_workspace_metadata_payload() {
                        return self.wrap_command(Task::perform(
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
                return self.wrap_command(Task::perform(
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
                return self.wrap_command(Task::perform(
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
                            let mut commands_list = vec![Task::perform(
                                commands::start_debug_session(request),
                                Message::DebuggerSessionStarted,
                            )];
                            if let Some((root, config)) = save_payload {
                                commands_list.push(Task::perform(
                                    commands::save_workspace_config(root, config),
                                    Message::WorkspaceConfigSaved,
                                ));
                            }
                            return self.wrap_command(Task::batch(commands_list));
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
                        return self.wrap_command(Task::none());
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
                        return self.wrap_command(Task::none());
                    }

                    // Handle Escape key to close search dialog (high priority)
                    if core_event.key == Key::Escape && self.state.search_dialog().is_visible {
                        self.state.search_dialog_mut().hide();
                        return self.wrap_command(Task::none());
                    }

                    // Handle F3 for next match, Shift+F3 for previous match (high priority)
                    if core_event.key == Key::Function(3) {
                        if self.state.search_dialog().is_visible {
                            if core_event.shift {
                                self.state.search_previous();
                            } else {
                                self.state.search_next();
                            }
                            return self.wrap_command(Task::none());
                        }
                    }

                    if self.state.matches_action(QUICK_COMMAND_MENU_ACTION, &core_event) {
                        if self.state.command_palette().is_open() {
                            self.state.close_command_palette();
                        } else {
                            self.state.set_command_palette_query(String::new());
                            self.state.open_command_palette();
                        }
                        return self.wrap_command(Task::none());
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
                                return self.wrap_command(Task::none());
                            }
                            Key::ArrowUp => {
                                self.state.handle_quick_command_navigation(-1);
                                return self.wrap_command(Task::none());
                            }
                            Key::Enter => {
                                if let Some(command) = self.state.selected_quick_command() {
                                    self.state.close_command_palette();
                                    let cmd = self.execute_quick_command(command);
                                    return self.wrap_command(cmd);
                                }
                                return self.wrap_command(Task::none());
                            }
                            Key::Escape => {
                                self.state.close_command_palette();
                                return self.wrap_command(Task::none());
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
                                    return self.wrap_command(Task::none());
                                }
                                Key::ArrowUp => {
                                    let _ = explorer.update(crate::widgets::file_explorer::Message::FocusPrev);
                                    return self.wrap_command(Task::none());
                                }
                                Key::Enter => {
                                    if let Some(cursor) = explorer.cursor() {
                                        explorer.update(crate::widgets::file_explorer::Message::Open(cursor, crate::widgets::file_explorer::OpenKind::InEditor));
                                    }
                                    return self.wrap_command(Task::none());
                                }
                                Key::Function(2) => {
                                    if let Some(cursor) = explorer.cursor() {
                                        explorer.update(crate::widgets::file_explorer::Message::StartRename(cursor));
                                    }
                                    return self.wrap_command(Task::none());
                                }
                                Key::Delete => {
                                    if let Some(cursor) = explorer.cursor() {
                                        explorer.update(crate::widgets::file_explorer::Message::Delete(cursor));
                                    }
                                    return self.wrap_command(Task::none());
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
                    return self.wrap_command(Task::none());
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
            Message::EditorLogShowRequested => {
                self.state.show_editor_log();
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
                return window::minimize(self.main_window_id, true);
            }
            Message::WindowMaximize => {
                if self.state.is_maximized {
                    self.state.is_maximized = false;
                    let mut commands = vec![window::maximize(self.main_window_id, false)];
                    if let Some(size) = self.state.previous_size {
                        commands.push(window::resize(self.main_window_id, size));
                        self.state.current_window_size = size;
                    }
                    return iced::Task::batch(commands);
                } else {
                    self.state.is_maximized = true;
                    self.state.previous_size = Some(self.state.current_window_size);
                    return window::maximize(self.main_window_id, true);
                }
            }
            Message::WindowClose => {
                return window::close(self.main_window_id);
            }
            Message::WindowDragStart => {
                return window::drag(self.main_window_id);
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
                    return window::resize(self.main_window_id, new_size);
                }
            }
            Message::WindowResizeEnd => {
                self.state.resize_start_pos = None;
                self.state.resize_start_size = None;
                self.state.resize_direction = None;
            }
            Message::FileExplorer(msg) => {
                if let crate::widgets::file_explorer::Message::OpenFile(path) = &msg {
                    return self.wrap_command(Task::perform(
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
            // Wine integration messages
            Message::WineCreateEnvironmentDialog => {
                // TODO: Show create environment dialog
            }
            // TODO: Add remaining Wine message handlers
            Message::WineEnvNameChanged(_name) => {
                // TODO: Handle environment name input
            }
            Message::WineExePathChanged(_path) => {
                // TODO: Handle executable path input
            }
            Message::WineArgsChanged(_args) => {
                // TODO: Handle arguments input
            }
            Message::WineEnvironmentToggled(_env_id) => {
                // TODO: Handle environment toggle
            }
            Message::WineCreateEnvironment => {
                // Handle environment creation
                println!("Wine: Create environment requested");
            }
            Message::WineSpawnProcess => {
                // Handle process spawning
                println!("Wine: Spawn process requested");
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
            Message::SearchHighlightTick => {
                if self.state.check_highlight_expiry() {
                    // Highlight expired, view will be updated
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
            // Debug dot messages
            Message::DebugDotAdd(line_number) => {
                self.state.add_debug_dot(line_number);
            }
            Message::DebugDotRemove(line_number) => {
                self.state.remove_debug_dot(line_number);
            }
            Message::DebugDotToggle(line_number) => {
                self.state.toggle_debug_dot(line_number);
            }
            Message::DebugDotsClear => {
                self.state.clear_debug_dots();
            }
            Message::GutterClicked(line_number) => {
                // Toggle debug dot when gutter is clicked
                self.state.toggle_debug_dot(line_number);
            }

            // Session management messages
            Message::SessionLoad(Ok(session_state)) => {
                // Store session state for later use
                self.state.set_session_state(session_state.clone());

                // Debug: Log what we loaded
                println!("DEBUG: Session loaded successfully");
                if let Some(root) = &session_state.workspace.workspace_root {
                    println!("DEBUG: Workspace root in session: {}", root.display());
                } else {
                    println!("DEBUG: No workspace root in session");
                }
                if let Some(last_folder) = &session_state.workspace.last_folder {
                    println!("DEBUG: Last folder in session: {}", last_folder.display());
                } else {
                    println!("DEBUG: No last folder in session");
                }

                // Log window state in session
                println!("DEBUG: Window state in session: {}x{} at ({}, {}), maximized: {}",
                    session_state.window.width, session_state.window.height,
                    session_state.window.x, session_state.window.y,
                    session_state.window.maximized);

                // Log open files in session
                println!("DEBUG: Open files in session: {}", session_state.workspace.open_files.len());
                for (i, file_path) in session_state.workspace.open_files.iter().enumerate() {
                    println!("DEBUG:   File {}: {}", i, file_path.display());
                }
                if let Some(active_index) = session_state.workspace.active_file_index {
                    println!("DEBUG: Active file index: {}", active_index);
                } else {
                    println!("DEBUG: No active file index");
                }

                // Restore workspace if we have a saved workspace root or last folder
                let workspace_to_restore = session_state.workspace.workspace_root.clone()
                    .or(session_state.workspace.last_folder.clone());

                if let Some(workspace_path) = workspace_to_restore {
                    println!("DEBUG: Attempting to restore workspace: {}", workspace_path.display());
                    if workspace_path.exists() {
                        println!("DEBUG: Workspace exists, triggering restore");
                        // Attempt to restore the workspace
                        return self.wrap_command(Task::perform(
                            async move { (workspace_path, session_state) },
                            |(path, state)| Message::WorkspaceRestoreFromPath(path, state),
                        ));
                    } else {
                        println!("DEBUG: Workspace path does not exist: {}", workspace_path.display());
                    }
                } else {
                    println!("DEBUG: No workspace to restore");
                }
            }

            Message::SessionLoad(Err(error)) => {
                eprintln!("Failed to load session: {}", error);
                // Continue with default state
            }

            Message::SessionSave(Ok(())) => {
                // Session saved successfully
            }

            Message::SessionSave(Err(error)) => {
                eprintln!("Failed to save session: {}", error);
            }

            Message::WindowStateUpdate(window_state) => {
                // Save window state
                let session_manager = self.session_manager.clone();
                return self.wrap_command(Task::perform(
                    async move {
                        session_manager.save_window_state(&window_state)
                            .map_err(|e| format!("Failed to save window state: {}", e))
                    },
                    Message::SessionSave,
                ));
            }

            Message::WorkspaceStateUpdate(workspace_state) => {
                // Save workspace state
                let session_manager = self.session_manager.clone();
                return self.wrap_command(Task::perform(
                    async move {
                        session_manager.save_workspace_state(&workspace_state)
                            .map_err(|e| format!("Failed to save workspace state: {}", e))
                    },
                    Message::SessionSave,
                ));
            }

            Message::WorkspaceRestoreFromPath(path, session_state) => {
                // Attempt to restore workspace from saved path
                println!("DEBUG: Restoring workspace and files from path: {}", path.display());

                // Store files to restore in state
                let files_to_restore: Vec<PathBuf> = session_state.workspace.open_files.iter()
                    .filter(|p| p.exists())
                    .cloned()
                    .collect();

                self.state.set_pending_files_to_restore(files_to_restore.clone());

                return self.wrap_command(Task::perform(
                    commands::load_workspace_from_path_with_files(path, session_state),
                    Message::WorkspaceLoaded,
                ));
            }

            Message::FilesRestoreRequested(file_paths) => {
                println!("DEBUG: Restoring {} files", file_paths.len());
                if file_paths.is_empty() {
                    return self.wrap_command(Task::none());
                }

                // Separate first file from additional files
                let first_file = file_paths[0].clone();
                let additional_files: Vec<PathBuf> = file_paths.into_iter().skip(1).collect();

                println!("DEBUG: Loading first restored file: {}", first_file.display());
                if !additional_files.is_empty() {
                    println!("DEBUG: Storing {} additional files for later loading", additional_files.len());
                    self.state.set_pending_files_to_restore(additional_files);
                }

                return self.wrap_command(Task::perform(
                    commands::load_document_from_path(first_file.to_string_lossy().to_string()),
                    |result| Message::FileLoaded(result.map(Some).map_err(|e| e)),
                ));
            }

            Message::AdditionalFilesRestoreRequested(file_paths) => {
                println!("DEBUG: Loading {} additional files", file_paths.len());
                if file_paths.is_empty() {
                    return self.wrap_command(Task::none());
                }

                // Load the next file in the list
                let first_file = file_paths[0].clone();
                let remaining_files: Vec<PathBuf> = file_paths.into_iter().skip(1).collect();

                // Store remaining files for later
                self.state.set_pending_files_to_restore(remaining_files.clone());

                println!("DEBUG: Loading additional file: {}", first_file.display());
                return self.wrap_command(Task::perform(
                    commands::load_document_from_path(first_file.to_string_lossy().to_string()),
                    |result| Message::FileLoaded(result.map(Some).map_err(|e| e)),
                ));
            }

            // Window state tracking messages
            Message::WindowChanged(width, height) => {
                println!("DEBUG: Window resized to {}x{}", width, height);
                // Update window state with current position and new size
                self.state.update_window_state(0, 0, width, height, false);

                // Save session immediately
                if let Some(session_state) = self.state.get_session_state() {
                    let session_state = session_state.clone();
                    let session_manager = self.session_manager.clone();
                    return self.wrap_command(Task::perform(
                        async move {
                            let result = session_manager.save_session_state(&session_state);
                            match &result {
                                Ok(()) => println!("DEBUG: Window state saved to session"),
                                Err(e) => println!("DEBUG: Failed to save window state: {}", e),
                            }
                            result.map_err(|e| format!("Failed to save session: {}", e))
                        },
                        Message::SessionSave,
                    ));
                }
            }

            Message::WindowMoved(x, y) => {
                println!("DEBUG: Window moved to ({}, {})", x, y);
                // Note: We need to track current window dimensions to update properly
                // For now, just update position
                let session_state = self.state.get_session_state().cloned();
                if let Some(session_state) = session_state {
                    let current_state = session_state.window.clone();
                    self.state.update_window_state(x, y, current_state.width, current_state.height, current_state.maximized);

                    let session_manager = self.session_manager.clone();
                    return self.wrap_command(Task::perform(
                        async move {
                            let result = session_manager.save_session_state(&session_state);
                            match &result {
                                Ok(()) => println!("DEBUG: Window state saved after move"),
                                Err(e) => println!("DEBUG: Failed to save window state: {}", e),
                            }
                            result.map_err(|e| format!("Failed to save session: {}", e))
                        },
                        Message::SessionSave,
                    ));
                }
            }

            Message::WindowEvent(event) => {
                if matches!(event, window::Event::Focused) {
                    println!("DEBUG: Window focused");
                }
                // Handle other window state changes as needed
            }
            // TODO: Handle remaining Wine messages when complex widget is re-enabled
        }

        self.wrap_command(Task::none())
    }

    fn view(&self) -> Element<'_, Message> {
        views::view(&self.state)
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }

    fn subscription(&self) -> Subscription<Message> {
        let input = event::listen_with(|event, _status, _id| {
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
                    event::Event::Window(window::Event::Resized(size)) => {
                        Some(Message::WindowChanged(size.width as u32, size.height as u32))
                    }
                    event::Event::Window(window::Event::Moved(pos)) => {
                        Some(Message::WindowMoved(pos.x as i32, pos.y as i32))
                    }
                    event::Event::Window(event) => {
                        Some(Message::WindowEvent(event))
                    }
                    _ => None,
                }
            }
        });

        let tick = time::every(Duration::from_millis(200)).map(|_| Message::DebuggerTick);
        let fps_tick = time::every(Duration::from_millis(8)).map(|_| Message::FpsUpdate); // ~120 FPS for 144Hz monitors
        let debounce_tick = time::every(Duration::from_millis(50)).map(|_| Message::SearchDebounceTick); // Check debounce every 50ms
        let highlight_tick = time::every(Duration::from_millis(100)).map(|_| Message::SearchHighlightTick); // Check highlight expiry every 100ms

        Subscription::batch(vec![input, tick, fps_tick, debounce_tick, highlight_tick])
    }

    fn scale_factor(&self) -> f32 {
        self.state.scale_factor() as f32
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
    fn wrap_command(&mut self, command: Task<Message>) -> Task<Message> {
        if let Some((root, metadata)) = self.state.take_workspace_metadata_payload() {
            let save = Task::perform(
                commands::save_workspace_metadata(root, metadata),
                Message::WorkspaceMetadataSaved,
            );
            Task::batch(vec![command, save])
        } else {
            command
        }
    }

    fn execute_quick_command(&mut self, command: QuickCommandId) -> Task<Message> {
        match command {
            QuickCommandId::OpenFile => {
                Task::perform(commands::pick_document(), Message::FileLoaded)
            }
            QuickCommandId::OpenFolder => {
                Task::perform(commands::pick_workspace(), Message::WorkspaceLoaded)
            }
            QuickCommandId::OpenSolution => {
                Task::perform(commands::pick_solution(), Message::SolutionLoaded)
            }
            QuickCommandId::SaveFile => self.save_active_document(),
            QuickCommandId::NewScratchBuffer => {
                let index = self.state.editor_mut().open_document(Document::default());
                self.state.editor_mut().set_active(index);
                self.state.clear_error();
                self.state.sync_buffer_from_editor();
                Task::none()
            }
            QuickCommandId::ShowScaleFactor => {
                let scale_info = self.state.format_scale_factor();
                println!("{}", scale_info);
                self.state.set_error(Some(scale_info));
                Task::none()
            }
            QuickCommandId::AddStickyNote => {
                match self.state.add_sticky_note_at_cursor() {
                    Ok(()) => self.state.clear_error(),
                    Err(err) => self.state.set_error(Some(err)),
                }
                if let Some((root, metadata)) = self.state.take_workspace_metadata_payload() {
                    Task::perform(
                        commands::save_workspace_metadata(root, metadata),
                        Message::WorkspaceMetadataSaved,
                    )
                } else {
                    Task::none()
                }
            }
            QuickCommandId::IncreaseCodeFontZoom => {
                self.state.increase_code_font_zoom();
                Task::none()
            }
            QuickCommandId::ShowEditorLog => {
                self.state.show_editor_log();
                Task::none()
            }
        }
    }

    fn save_active_document(&mut self) -> Task<Message> {
        if let Some(doc) = self.state.editor().active_document() {
            let request = SaveDocumentRequest {
                path: doc.path.clone(),
                contents: doc.buffer.to_string(),
                suggested_name: Some(doc.display_name().to_string()),
            };
            Task::perform(commands::save_document(request), Message::DocumentSaved)
        } else {
            Task::none()
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
