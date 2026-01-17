// App has some configuration methods that are API for future use
#![allow(dead_code)]

use crate::commands::{
    self, DebugSessionBreakpoint, DebugSessionRequest, SaveDocumentRequest, SaveKeymapRequest,
    WorkspaceData,
};
use crate::debugger::{DebugLaunchPlan, DebuggerType, DebuggerUiEvent};
use crate::keyboard;
use crate::message::Message;
use crate::notifications::{NotificationKind, NotificationRequest};
use crate::session::{SessionManager, SessionState};
use crate::state::EditorState;
use crate::views;
use iced::Subscription;
use iced::{Element, Task, Theme, event, mouse, time, window};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use vedit_application::QuickCommandId;
use vedit_core::{Document, Key, QUICK_COMMAND_MENU_ACTION, SAVE_ACTION};

// Global refresh rate configuration
pub static REFRESH_RATE_CONFIG: std::sync::LazyLock<RefreshRateConfig> =
    std::sync::LazyLock::new(|| RefreshRateConfig::new());

#[derive(Debug, Clone)]
pub struct RefreshRateConfig {
    pub highest_refresh_rate: Arc<Mutex<f32>>,    // in Hz
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

/// Detect monitor refresh rates asynchronously to avoid blocking startup
fn detect_refresh_rates_async() -> (f32, f32) {
    let mut highest_refresh: f32 = 60.0;
    let mut current_refresh: f32 = 60.0;

    // Method 1: Try environment variable for X11/Wayland
    if let Ok(display) = std::env::var("DISPLAY") {
        if !display.is_empty() {
            // Run xrandr in background - this is the slow part
            if let Ok(rate) = detect_x11_refresh_rate() {
                current_refresh = rate;
                highest_refresh = highest_refresh.max(rate);
            }
        }
    }

    // Method 2: Check for high refresh rate indicators (fast)
    if std::env::var("GDK_REFRESH_RATE").is_ok() || std::env::var("QT_SCALE_FACTOR").is_ok() {
        highest_refresh = highest_refresh.max(144.0);
    }

    (highest_refresh, current_refresh)
}

/// Detect X11 refresh rate by parsing xrandr output
fn detect_x11_refresh_rate() -> Result<f32, Box<dyn std::error::Error + Send + Sync>> {
    use std::process::Command;

    let output = Command::new("xrandr").arg("--query").output()?;
    let output_str = String::from_utf8_lossy(&output.stdout);

    // Look for lines with active modes (marked with *)
    for line in output_str.lines() {
        if line.contains("*") {
            // Parse refresh rates like "60.00*" or "144.00*+"
            for word in line.split_whitespace() {
                if word.contains('*') {
                    let rate_str = word.trim_matches(|c| c == '*' || c == '+');
                    if let Ok(rate) = rate_str.parse::<f32>() {
                        if rate > 30.0 && rate < 500.0 {
                            return Ok(rate);
                        }
                    }
                }
            }
        }
    }

    Err("Could not detect refresh rate".into())
}

pub fn run() -> iced::Result {
    // Load session state first to get window settings
    let session_manager = SessionManager::new().unwrap_or_else(|e| {
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
            println!(
                "DEBUG: Failed to load session for window config: {}, using defaults",
                e
            );
            SessionState::default()
        }
    };

    let window_state = &session_state.window;
    println!(
        "DEBUG: Restoring window to {}x{} at ({}, {}), maximized: {}",
        window_state.width,
        window_state.height,
        window_state.x,
        window_state.y,
        window_state.maximized
    );

    iced::application(EditorApp::new, EditorApp::update, EditorApp::view)
        .title("vedit")
        .subscription(EditorApp::subscription)
        .theme(EditorApp::theme)
        .window(window::Settings {
            size: iced::Size::new(window_state.width as f32, window_state.height as f32),
            position: window::Position::Centered,
            min_size: Some(iced::Size::new(400.0, 300.0)),
            resizable: true,
            decorations: false,
            ..Default::default()
        })
        .scale_factor(EditorApp::scale_factor)
        .run()
}

struct EditorApp {
    state: EditorState,
    session_manager: SessionManager,
    main_window_id: Option<window::Id>,
}

impl Default for EditorApp {
    fn default() -> Self {
        let session_manager = SessionManager::new().unwrap_or_else(|e| {
            eprintln!("Failed to initialize session manager: {}", e);
            // Create a fallback session manager that uses temp directory
            let temp_dir = std::env::temp_dir().join("vedit");
            std::fs::create_dir_all(&temp_dir).ok();
            println!("DEBUG: Using fallback session dir: {}", temp_dir.display());
            SessionManager::with_config_dir(temp_dir)
        });

        println!(
            "DEBUG: Session manager initialized with config dir: {}",
            session_manager.config_dir.display()
        );

        Self {
            state: EditorState::default(),
            session_manager,
            main_window_id: None,
        }
    }
}

impl EditorApp {
    fn update_timing_for_refresh_rate(&self, refresh_rate: f32) {
        // This will be used to dynamically update the application timing
        // The actual implementation will update the subscription timing
        println!(
            "Detected refresh rate: {:.0} Hz - Optimizing timing",
            refresh_rate
        );
    }
}

impl EditorApp {
    fn new() -> (Self, Task<Message>) {
        let app = Self::default();

        // Load session state at startup
        let session_manager = app.session_manager.clone();
        let config_dir = session_manager.config_dir.clone();
        let load_command = Task::perform(
            async move {
                println!(
                    "DEBUG: Attempting to load session from: {}",
                    config_dir.display()
                );
                let result = session_manager.load_session_state();
                match &result {
                    Ok(_) => println!("DEBUG: Session loaded successfully"),
                    Err(e) => println!("DEBUG: Failed to load session: {}", e),
                }
                result.map_err(|e| format!("Failed to load session: {}", e))
            },
            Message::SessionLoad,
        );

        // Trigger refresh rate detection asynchronously (xrandr can be slow)
        let refresh_command = Task::perform(
            async { detect_refresh_rates_async() },
            |(highest, current)| Message::RefreshRateDetected(highest, current),
        );

        let combined_command = Task::batch(vec![load_command, refresh_command]);
        (app, combined_command)
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        let debugger_events = self.state.process_debugger_events();
        self.state.process_console_events();
        self.handle_debugger_events(debugger_events);
        match message {
            Message::OpenFileRequested => {
                // Push current location to navigation history before opening new file
                self.state.push_navigation();
                return self.wrap_command(Task::perform(
                    commands::pick_document(),
                    Message::FileLoaded,
                ));
            }
            Message::FileLoaded(result) => match result {
                Ok(Some(document)) => {
                    let file_path = document
                        .path
                        .clone()
                        .unwrap_or_else(|| "unnamed".to_string());
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
                                    Err(e) => println!(
                                        "DEBUG: Failed to save session after file open: {}",
                                        e
                                    ),
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
                // Push current location to navigation history before switching
                self.state.push_navigation();
                self.state.editor_mut().set_active(index);
                self.state.sync_buffer_from_editor();
            }
            Message::CloseDocument(index) => {
                let editor = self.state.editor_mut();
                if editor.open_documents().len() > 1 {
                    editor.close_document(index);
                    self.state.sync_buffer_from_editor();
                }
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
            Message::SolutionTreeToggle(node_id) => {
                self.state.toggle_solution_node(node_id);
            }
            Message::WorkspaceLoaded(result) => match result {
                Ok(Some(WorkspaceData {
                    root,
                    config,
                    metadata,
                })) => {
                    self.state.install_workspace(root.clone(), config, metadata);
                    self.state.refresh_file_explorer();
                    self.state.clear_error();

                    // Update open files in session state
                    self.state.update_session_open_files();

                    // Trigger file restoration if we have pending files
                    let pending_files = self.state.take_pending_files_to_restore();
                    if !pending_files.is_empty() {
                        println!(
                            "DEBUG: Triggering restoration of {} pending files",
                            pending_files.len()
                        );
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

                    // Also save complete session state - use tracked window state
                    let window_state = self
                        .state
                        .get_session_state()
                        .map(|s| s.window.clone())
                        .unwrap_or_default();
                    let session_state = crate::session::SessionState {
                        window: window_state,
                        workspace: workspace_state.clone(),
                    };

                    println!("DEBUG: Saving complete session for root: {}", root);
                    let session_manager = self.session_manager.clone();
                    return self.wrap_command(Task::perform(
                        async move {
                            // Save both workspace state and complete session
                            let workspace_result =
                                session_manager.save_workspace_state(&workspace_state);
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
                    config,
                    metadata,
                })) => {
                    self.state.install_workspace(root.clone(), config, metadata);
                    self.state.refresh_file_explorer();
                    self.state.clear_error();

                    // Update open files in session state
                    self.state.update_session_open_files();

                    // Trigger file restoration if we have pending files
                    let pending_files = self.state.take_pending_files_to_restore();
                    if !pending_files.is_empty() {
                        println!(
                            "DEBUG: Triggering restoration of {} pending files",
                            pending_files.len()
                        );
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

                    // Also save complete session state - use tracked window state
                    let window_state = self
                        .state
                        .get_session_state()
                        .map(|s| s.window.clone())
                        .unwrap_or_default();
                    let session_state = crate::session::SessionState {
                        window: window_state,
                        workspace: workspace_state.clone(),
                    };

                    println!("DEBUG: Saving complete session for root: {}", root);
                    let session_manager = self.session_manager.clone();
                    return self.wrap_command(Task::perform(
                        async move {
                            // Save both workspace state and complete session
                            let workspace_result =
                                session_manager.save_workspace_state(&workspace_state);
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
                // Push current location to navigation history before opening new file
                self.state.push_navigation();

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
                    if let Some((root, metadata)) = self.state.take_workspace_metadata_payload() {
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
            // Context menu messages
            Message::EditorContextMenuShow(x, y, hover_pos) => {
                self.state.show_context_menu(x, y, hover_pos);
            }
            Message::EditorContextMenuHide => {
                self.state.hide_context_menu();
            }
            Message::EditorContextMenuAddStickyNote => {
                self.state.hide_context_menu();
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
            Message::EditorContextMenuCut => {
                // Copy selection to clipboard, then delete it
                if let Some(selection) = self.state.get_selection() {
                    if let Ok(mut clipboard) = arboard::Clipboard::new() {
                        let _ = clipboard.set_text(&selection);
                    }
                    // Delete the selection by inserting empty text
                    self.state
                        .apply_buffer_action(iced::widget::text_editor::Action::Edit(
                            iced::widget::text_editor::Edit::Paste(std::sync::Arc::new(
                                String::new(),
                            )),
                        ));
                }
                self.state.hide_context_menu();
            }
            Message::EditorContextMenuCopy => {
                // Copy selection to clipboard
                if let Some(selection) = self.state.get_selection() {
                    if let Ok(mut clipboard) = arboard::Clipboard::new() {
                        let _ = clipboard.set_text(&selection);
                    }
                }
                self.state.hide_context_menu();
            }
            Message::EditorContextMenuPaste => {
                // Paste from clipboard
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    if let Ok(text) = clipboard.get_text() {
                        self.state
                            .apply_buffer_action(iced::widget::text_editor::Action::Edit(
                                iced::widget::text_editor::Edit::Paste(std::sync::Arc::new(text)),
                            ));
                    }
                }
                self.state.hide_context_menu();
            }
            Message::EditorContextMenuSelectAll => {
                self.state.hide_context_menu();
                self.state
                    .apply_buffer_action(iced::widget::text_editor::Action::SelectAll);
            }
            Message::EditorContextMenuGotoDefinition => {
                // Get the definition before hiding the menu (which clears it)
                let definition = self.state.context_menu_definition().cloned();
                self.state.hide_context_menu();

                if let Some(def) = definition {
                    // Push current location to navigation history before jumping
                    self.state.push_navigation();

                    let path_str = def.file_path.to_string_lossy().to_string();
                    return self.wrap_command(Task::perform(
                        commands::load_document_from_path(path_str.clone()),
                        move |result| Message::FileLoaded(result.map(Some)),
                    ));
                }
            }

            // Hover-to-definition messages
            Message::EditorHover(pos, x, y) => {
                // Debounce hover events
                if !self.state.should_process_hover(pos.line, pos.column) {
                    return Task::none();
                }

                // Look up symbol at position and start delay timer
                if let Some(mut info) = self.state.lookup_symbol_at_position(pos.line, pos.column) {
                    info.tooltip_x = x;
                    info.tooltip_y = y;
                    self.state.start_hover_delay(info);
                } else {
                    // No symbol at this position - hide tooltip if not sticky
                    if !self.state.is_cursor_in_tooltip() {
                        self.state.cancel_pending_hover();
                    }
                }
            }
            Message::HoverDelayTick => {
                // Check if hover delay has elapsed
                if let Some(info) = self.state.check_hover_delay() {
                    self.state.set_hover_info(Some(info));
                }
            }
            Message::HoverCursorMoved(x, y) => {
                // Check if cursor is inside the tooltip bounds
                let in_tooltip = self.state.is_point_in_tooltip(x, y);
                self.state.set_cursor_in_tooltip(in_tooltip);

                // If cursor moved outside tooltip and no pending hover, hide it
                if !in_tooltip
                    && self.state.hover_tooltip_visible()
                    && !self.state.has_pending_hover()
                {
                    self.state.force_hide_hover_tooltip();
                }

                // Also handle window resize dragging (was WindowResizeMove)
                let pos = iced::Point::new(x, y);
                if let Some(id) = self.main_window_id {
                    if let (Some(start_pos), Some(start_size), Some(dir)) = (
                        self.state.resize_start_pos,
                        self.state.resize_start_size,
                        self.state.resize_direction,
                    ) {
                        let delta = pos - start_pos;
                        const MIN_WIDTH: f32 = 400.0;
                        const MIN_HEIGHT: f32 = 300.0;
                        let new_width = if matches!(
                            dir,
                            crate::state::ResizeDirection::Right
                                | crate::state::ResizeDirection::Both
                        ) {
                            (start_size.width + delta.x).max(MIN_WIDTH)
                        } else {
                            start_size.width
                        };
                        let new_height = if matches!(
                            dir,
                            crate::state::ResizeDirection::Bottom
                                | crate::state::ResizeDirection::Both
                        ) {
                            (start_size.height + delta.y).max(MIN_HEIGHT)
                        } else {
                            start_size.height
                        };
                        let new_size = iced::Size::new(new_width, new_height);
                        self.state.current_window_size = new_size;
                        return window::resize(id, new_size);
                    }
                }
            }
            Message::HoverTooltipShow(info) => {
                self.state.set_hover_info(Some(info));
            }
            Message::HoverTooltipHide => {
                self.state.hide_hover_tooltip();
            }
            Message::HoverGotoDefinition(file_path, _line, _column) => {
                self.state.force_hide_hover_tooltip();

                // Push current location to navigation history before jumping
                self.state.push_navigation();

                // Open the file and navigate to line
                let path_str = file_path.to_string_lossy().to_string();
                return self.wrap_command(Task::perform(
                    commands::load_document_from_path(path_str.clone()),
                    move |result| {
                        // After loading, we need to scroll to the line
                        // For now, just load the file - scrolling will be handled separately
                        Message::FileLoaded(result.map(Some))
                    },
                ));
            }
            Message::SymbolIndexRefresh => match self.state.refresh_symbol_index() {
                Ok(count) => {
                    editor_log_info!("SYMBOLS", "Indexed {} files", count);
                }
                Err(e) => {
                    editor_log_error!("SYMBOLS", "Failed to refresh symbol index: {}", e);
                }
            },
            Message::SymbolIndexUpdated(result) => match result {
                Ok(count) => {
                    editor_log_info!("SYMBOLS", "Symbol index updated: {} files", count);
                }
                Err(e) => {
                    editor_log_error!("SYMBOLS", "Symbol index update failed: {}", e);
                }
            },

            // Navigation history (back/forward)
            Message::NavigateBack => {
                println!("DEBUG: NavigateBack pressed");
                if let Some(entry) = self.state.navigate_back() {
                    println!("DEBUG: Navigating back to: {:?}, line {}", entry.file_path, entry.line);
                    return self.navigate_to_entry(entry);
                } else {
                    println!("DEBUG: No navigation history to go back to");
                }
            }
            Message::NavigateForward => {
                println!("DEBUG: NavigateForward pressed");
                if let Some(entry) = self.state.navigate_forward() {
                    println!("DEBUG: Navigating forward to: {:?}, line {}", entry.file_path, entry.line);
                    return self.navigate_to_entry(entry);
                } else {
                    println!("DEBUG: No navigation history to go forward to");
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
            Message::SettingsBindingsSaveRequested => match self.state.keymap_save_payload() {
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
            },
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
                            let request = session_request_from_plan(
                                plan,
                                self.state.debugger().debugger_type(),
                            );
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
                            self.state
                                .set_error(Some("No debug targets selected".to_string()));
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
                self.state.debugger_mut().set_breakpoint_draft_file(value);
            }
            Message::DebuggerBreakpointDraftLineChanged(value) => {
                self.state.debugger_mut().set_breakpoint_draft_line(value);
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
                self.state.debugger_mut().set_manual_target_name(value);
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
                self.state.debugger_mut().set_manual_target_arguments(value);
            }
            Message::DebuggerManualTargetSaved => match self.state.commit_manual_debug_target() {
                Ok(()) => self.state.clear_error(),
                Err(err) => self.state.set_error(Some(err)),
            },
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
                    // Handle Ctrl+C for copy (using arboard for Wayland compatibility)
                    if core_event.key == Key::Character('C')
                        && (core_event.ctrl || core_event.command)
                        && !core_event.shift
                    {
                        if let Some(selection) = self.state.get_selection() {
                            if let Ok(mut clipboard) = arboard::Clipboard::new() {
                                let _ = clipboard.set_text(&selection);
                            }
                        }
                        return self.wrap_command(Task::none());
                    }

                    // Handle Ctrl+X for cut (using arboard for Wayland compatibility)
                    if core_event.key == Key::Character('X')
                        && (core_event.ctrl || core_event.command)
                    {
                        if let Some(selection) = self.state.get_selection() {
                            if let Ok(mut clipboard) = arboard::Clipboard::new() {
                                let _ = clipboard.set_text(&selection);
                            }
                            // Delete the selection
                            self.state.apply_buffer_action(
                                iced::widget::text_editor::Action::Edit(
                                    iced::widget::text_editor::Edit::Paste(std::sync::Arc::new(
                                        String::new(),
                                    )),
                                ),
                            );
                        }
                        return self.wrap_command(Task::none());
                    }

                    // Handle Ctrl+V for paste (using arboard for Wayland compatibility)
                    if core_event.key == Key::Character('V')
                        && (core_event.ctrl || core_event.command)
                    {
                        if let Ok(mut clipboard) = arboard::Clipboard::new() {
                            if let Ok(text) = clipboard.get_text() {
                                self.state.apply_buffer_action(
                                    iced::widget::text_editor::Action::Edit(
                                        iced::widget::text_editor::Edit::Paste(
                                            std::sync::Arc::new(text),
                                        ),
                                    ),
                                );
                            }
                        }
                        return self.wrap_command(Task::none());
                    }

                    // Handle Ctrl+Z for undo
                    if core_event.key == Key::Character('Z')
                        && (core_event.ctrl || core_event.command)
                        && !core_event.shift
                    {
                        self.state.undo();
                        return self.wrap_command(Task::none());
                    }

                    // Handle Ctrl+Y or Ctrl+Shift+Z for redo
                    if ((core_event.key == Key::Character('Y')
                        && (core_event.ctrl || core_event.command))
                        || (core_event.key == Key::Character('Z')
                            && (core_event.ctrl || core_event.command)
                            && core_event.shift))
                    {
                        self.state.redo();
                        return self.wrap_command(Task::none());
                    }

                    // Handle Ctrl+F for search (high priority)
                    if core_event.key == Key::Character('F')
                        && (core_event.ctrl || core_event.command)
                    {
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

                    if self
                        .state
                        .matches_action(QUICK_COMMAND_MENU_ACTION, &core_event)
                    {
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
                    if self.state.selected_right_rail_tab()
                        == crate::message::RightRailTab::Workspace
                    {
                        if let Some(explorer) = self.state.file_explorer_mut() {
                            match core_event.key {
                                Key::ArrowDown => {
                                    let _ = explorer
                                        .update(crate::widgets::file_explorer::Message::FocusNext);
                                    return self.wrap_command(Task::none());
                                }
                                Key::ArrowUp => {
                                    let _ = explorer
                                        .update(crate::widgets::file_explorer::Message::FocusPrev);
                                    return self.wrap_command(Task::none());
                                }
                                Key::Enter => {
                                    if let Some(cursor) = explorer.cursor() {
                                        let _ = explorer.update(
                                            crate::widgets::file_explorer::Message::Open(
                                                cursor,
                                                crate::widgets::file_explorer::OpenKind::InEditor,
                                            ),
                                        );
                                    }
                                    return self.wrap_command(Task::none());
                                }
                                Key::Function(2) => {
                                    if let Some(cursor) = explorer.cursor() {
                                        let _ = explorer.update(
                                            crate::widgets::file_explorer::Message::StartRename(
                                                cursor,
                                            ),
                                        );
                                    }
                                    return self.wrap_command(Task::none());
                                }
                                Key::Delete => {
                                    if let Some(cursor) = explorer.cursor() {
                                        let _ = explorer.update(
                                            crate::widgets::file_explorer::Message::Delete(cursor),
                                        );
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
            Message::RefreshRateDetected(highest_refresh, current_refresh) => {
                // Apply the detected refresh rates (detection ran in background)
                REFRESH_RATE_CONFIG.set_refresh_rates(highest_refresh, current_refresh);
                self.update_timing_for_refresh_rate(highest_refresh);
            }
            Message::NotificationDismissed(id) => {
                self.state.dismiss_notification(id);
            }
            Message::WindowIdDiscovered(id) => {
                if self.main_window_id.is_none() {
                    self.main_window_id = Some(id);
                }
            }
            Message::WindowMinimize => {
                if let Some(id) = self.main_window_id {
                    return window::minimize(id, true);
                }
            }
            Message::WindowMaximize => {
                if let Some(id) = self.main_window_id {
                    if self.state.is_maximized {
                        self.state.is_maximized = false;
                        let mut commands = vec![window::maximize(id, false)];
                        if let Some(size) = self.state.previous_size {
                            commands.push(window::resize(id, size));
                            self.state.current_window_size = size;
                        }
                        return iced::Task::batch(commands);
                    } else {
                        self.state.is_maximized = true;
                        self.state.previous_size = Some(self.state.current_window_size);
                        return window::maximize(id, true);
                    }
                }
            }
            Message::WindowClose => {
                return iced::exit();
            }
            Message::WindowDragStart => {
                if let Some(id) = self.main_window_id {
                    return window::drag(id);
                }
            }
            Message::WindowResizeStart(pos) => {
                let size = self.state.current_window_size;
                let right = pos.x > size.width - 20.0;
                let bottom = pos.y > size.height - 20.0;
                if right || bottom {
                    self.state.resize_start_pos = Some(pos);
                    self.state.resize_start_size = Some(size);
                    self.state.resize_direction = Some(if right && bottom {
                        crate::state::ResizeDirection::Both
                    } else if right {
                        crate::state::ResizeDirection::Right
                    } else {
                        crate::state::ResizeDirection::Bottom
                    });
                }
            }
            Message::WindowResizeEnd => {
                self.state.resize_start_pos = None;
                self.state.resize_start_size = None;
                self.state.resize_direction = None;
            }
            Message::FileExplorer(msg) => {
                if let crate::widgets::file_explorer::Message::OpenFile(path) = &msg {
                    // Push current location to navigation history before opening new file
                    self.state.push_navigation();
                    return self.wrap_command(Task::perform(
                        commands::load_document_from_path(path.clone()),
                        |result| Message::FileLoaded(result.map(Some)),
                    ));
                }

                if let Some(explorer) = self.state.file_explorer_mut() {
                    let command = explorer.update(msg);
                    return self.wrap_command(command.map(Message::FileExplorer));
                }
            }
            Message::RightRailTabSelected(tab) => {
                self.state.set_selected_right_rail_tab(tab);
            }
            // Wine integration messages (WineState currently disabled in state.rs)
            // These handlers are stubs pending re-enablement of the full Wine widget
            Message::WineCreateEnvironmentDialog => {
                // Stub: Wine widget temporarily disabled
            }
            Message::WineEnvNameChanged(_name) => {
                // Stub: Wine widget temporarily disabled
            }
            Message::WineExePathChanged(_path) => {
                // Stub: Wine widget temporarily disabled
            }
            Message::WineArgsChanged(_args) => {
                // Stub: Wine widget temporarily disabled
            }
            Message::WineEnvironmentToggled(_env_id) => {
                // Stub: Wine widget temporarily disabled
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
                self.state
                    .search_dialog_mut()
                    .set_case_sensitive(case_sensitive);
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

                // Sync current_window_size with session state
                self.state.current_window_size = iced::Size::new(
                    session_state.window.width as f32,
                    session_state.window.height as f32,
                );

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
                println!(
                    "DEBUG: Window state in session: {}x{} at ({}, {}), maximized: {}",
                    session_state.window.width,
                    session_state.window.height,
                    session_state.window.x,
                    session_state.window.y,
                    session_state.window.maximized
                );

                // Log open files in session
                println!(
                    "DEBUG: Open files in session: {}",
                    session_state.workspace.open_files.len()
                );
                for (i, file_path) in session_state.workspace.open_files.iter().enumerate() {
                    println!("DEBUG:   File {}: {}", i, file_path.display());
                }
                if let Some(active_index) = session_state.workspace.active_file_index {
                    println!("DEBUG: Active file index: {}", active_index);
                } else {
                    println!("DEBUG: No active file index");
                }

                // Restore workspace if we have a saved workspace root or last folder
                let workspace_to_restore = session_state
                    .workspace
                    .workspace_root
                    .clone()
                    .or(session_state.workspace.last_folder.clone());

                if let Some(workspace_path) = workspace_to_restore {
                    println!(
                        "DEBUG: Attempting to restore workspace: {}",
                        workspace_path.display()
                    );
                    if workspace_path.exists() {
                        println!("DEBUG: Workspace exists, triggering restore");
                        // Attempt to restore the workspace
                        return self.wrap_command(Task::perform(
                            async move { (workspace_path, session_state) },
                            |(path, state)| Message::WorkspaceRestoreFromPath(path, state),
                        ));
                    } else {
                        println!(
                            "DEBUG: Workspace path does not exist: {}",
                            workspace_path.display()
                        );
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
                        session_manager
                            .save_window_state(&window_state)
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
                        session_manager
                            .save_workspace_state(&workspace_state)
                            .map_err(|e| format!("Failed to save workspace state: {}", e))
                    },
                    Message::SessionSave,
                ));
            }

            Message::WorkspaceRestoreFromPath(path, session_state) => {
                // Attempt to restore workspace from saved path
                println!(
                    "DEBUG: Restoring workspace and files from path: {}",
                    path.display()
                );

                // Store files to restore in state
                let files_to_restore: Vec<PathBuf> = session_state
                    .workspace
                    .open_files
                    .iter()
                    .filter(|p| p.exists())
                    .cloned()
                    .collect();

                self.state
                    .set_pending_files_to_restore(files_to_restore.clone());

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

                println!(
                    "DEBUG: Loading first restored file: {}",
                    first_file.display()
                );
                if !additional_files.is_empty() {
                    println!(
                        "DEBUG: Storing {} additional files for later loading",
                        additional_files.len()
                    );
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
                self.state
                    .set_pending_files_to_restore(remaining_files.clone());

                println!("DEBUG: Loading additional file: {}", first_file.display());
                return self.wrap_command(Task::perform(
                    commands::load_document_from_path(first_file.to_string_lossy().to_string()),
                    |result| Message::FileLoaded(result.map(Some).map_err(|e| e)),
                ));
            }

            // Window state tracking messages
            Message::WindowChanged(width, height) => {
                println!("DEBUG: Window resized to {}x{}", width, height);
                // Keep current_window_size in sync for resize operations
                self.state.current_window_size = iced::Size::new(width as f32, height as f32);
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
                    self.state.update_window_state(
                        x,
                        y,
                        current_state.width,
                        current_state.height,
                        current_state.maximized,
                    );

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
        use std::sync::atomic::{AtomicBool, Ordering};
        static WINDOW_ID_SENT: AtomicBool = AtomicBool::new(false);
        // Reset the flag when we don't have a window ID yet (app restart scenario)
        if self.main_window_id.is_none() {
            WINDOW_ID_SENT.store(false, Ordering::Relaxed);
        }
        let input = event::listen_with(|event, _status, id| unsafe {
            static mut CURSOR_POS: iced::Point = iced::Point::ORIGIN;
            // Send window ID discovery message once
            if !WINDOW_ID_SENT.swap(true, Ordering::Relaxed) {
                return Some(Message::WindowIdDiscovered(id));
            }
            match event {
                event::Event::Keyboard(key_event) => Some(Message::Keyboard(key_event)),
                event::Event::Mouse(mouse::Event::CursorMoved { position }) => {
                    CURSOR_POS = position;
                    // Note: We send HoverCursorMoved for tooltip stickiness tracking
                    // WindowResizeMove is handled separately in update()
                    Some(Message::HoverCursorMoved(position.x, position.y))
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
                event::Event::Window(window::Event::Resized(size)) => Some(Message::WindowChanged(
                    size.width as u32,
                    size.height as u32,
                )),
                event::Event::Window(window::Event::Moved(pos)) => {
                    Some(Message::WindowMoved(pos.x as i32, pos.y as i32))
                }
                event::Event::Window(event) => Some(Message::WindowEvent(event)),
                _ => None,
            }
        });

        let tick = time::every(Duration::from_millis(200)).map(|_| Message::DebuggerTick);
        let fps_tick = time::every(Duration::from_millis(8)).map(|_| Message::FpsUpdate); // ~120 FPS for 144Hz monitors
        let debounce_tick =
            time::every(Duration::from_millis(50)).map(|_| Message::SearchDebounceTick); // Check debounce every 50ms
        let highlight_tick =
            time::every(Duration::from_millis(100)).map(|_| Message::SearchHighlightTick); // Check highlight expiry every 100ms
        let hover_tick = time::every(Duration::from_millis(100)).map(|_| Message::HoverDelayTick); // Check hover delay every 100ms

        Subscription::batch(vec![
            input,
            tick,
            fps_tick,
            debounce_tick,
            highlight_tick,
            hover_tick,
        ])
    }

    fn scale_factor(&self) -> f32 {
        self.state.scale_factor() as f32
    }
}

fn session_request_from_plan(
    plan: &DebugLaunchPlan,
    debugger_type: DebuggerType,
) -> DebugSessionRequest {
    DebugSessionRequest {
        executable: plan.target.executable.to_string_lossy().to_string(),
        working_directory: plan.target.working_directory.to_string_lossy().to_string(),
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

    /// Navigate to a saved navigation entry (for back/forward)
    fn navigate_to_entry(&mut self, entry: crate::state::NavigationEntry) -> Task<Message> {
        // Check if document is already open by path
        if let Some(ref path) = entry.file_path {
            let docs = self.state.editor().open_documents();
            println!("DEBUG: Looking for path '{}' in {} open docs", path, docs.len());
            for (i, doc) in docs.iter().enumerate() {
                println!("DEBUG:   Doc {}: {:?}", i, doc.path);
            }
            if let Some(index) = docs.iter().position(|doc| doc.path.as_ref() == Some(path)) {
                // Document is already open - just switch to it
                println!("DEBUG: Found at index {}, switching", index);
                self.state.editor_mut().set_active(index);
                self.state.sync_buffer_from_editor();
                self.state.move_cursor_to(entry.line, entry.column);
                return Task::none();
            }

            // Document not open - need to load it
            println!("DEBUG: Document not open, loading from disk");
            let path_clone = path.clone();
            return self.wrap_command(Task::perform(
                commands::load_document_from_path(path_clone),
                move |result| Message::FileLoaded(result.map(Some)),
            ));
        }

        // No file path - try to use document index (for scratch buffers)
        println!("DEBUG: No file path, using document index {}", entry.document_index);
        let num_docs = self.state.editor().open_documents().len();
        if entry.document_index < num_docs {
            self.state.editor_mut().set_active(entry.document_index);
            self.state.sync_buffer_from_editor();
            self.state.move_cursor_to(entry.line, entry.column);
        }

        Task::none()
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
