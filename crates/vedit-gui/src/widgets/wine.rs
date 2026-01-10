//! Wine integration widgets for vedit

use crate::message::Message;
use crate::style::{button_style, container_style, text_input_style};
use iced::widget::{
    button, checkbox, column, container, horizontal_rule, row, scrollable, text, text_input,
    Space, Column, Row, Scrollable,
};
use iced::{Alignment, Element, Length, Renderer, Theme};
use std::collections::HashMap;
use uuid::Uuid;

/// Wine management state
#[derive(Debug, Clone)]
pub struct WineState {
    /// Available Wine environments
    pub environments: HashMap<String, WineEnvironmentView>,

    /// Active processes
    pub processes: HashMap<Uuid, WineProcessView>,

    /// Remote desktop sessions
    pub remote_sessions: HashMap<Uuid, WineRemoteSessionView>,

    /// UI state
    pub ui: WineUiState,

    /// Selected environment for details
    pub selected_environment: Option<String>,

    /// Selected process for details
    pub selected_process: Option<Uuid>,
}

/// Wine environment view data
#[derive(Debug, Clone)]
pub struct WineEnvironmentView {
    pub id: String,
    pub name: String,
    pub path: String,
    pub architecture: String,
    pub windows_version: String,
    pub active_processes: usize,
    pub is_expanded: bool,
}

/// Wine process view data
#[derive(Debug, Clone)]
pub struct WineProcessView {
    pub id: Uuid,
    pub name: String,
    pub executable: String,
    pub status: String,
    pub uptime: String,
    pub environment_id: String,
    pub has_remote_desktop: bool,
}

/// Wine remote session view data
#[derive(Debug, Clone)]
pub struct WineRemoteSessionView {
    pub id: Uuid,
    pub session_type: String,
    pub port: u16,
    pub resolution: (u32, u32),
    pub connection_url: String,
    pub uptime: String,
}

/// Wine UI state
#[derive(Debug, Clone)]
pub struct WineUiState {
    pub active_tab: WineTab,
    pub create_env_dialog_open: bool,
    pub spawn_process_dialog_open: bool,
    pub remote_desktop_dialog_open: bool,
    pub env_name_input: String,
    pub exe_path_input: String,
    pub args_input: String,
    pub selected_architecture: WineArchitecture,
    pub selected_windows_version: WineWindowsVersion,
    pub selected_desktop_type: WineDesktopType,
    pub resolution_width: String,
    pub resolution_height: String,
}

/// Wine UI tabs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WineTab {
    Environments,
    Processes,
    RemoteDesktop,
    Settings,
}

/// Wine architecture options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WineArchitecture {
    Win32,
    Win64,
}

/// Wine Windows version options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WineWindowsVersion {
    WindowsXP,
    Windows7,
    Windows8,
    Windows81,
    Windows10,
    Windows11,
}

/// Wine desktop type options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WineDesktopType {
    Vnc,
    Rdp,
    X11,
}

impl Default for WineState {
    fn default() -> Self {
        Self {
            environments: HashMap::new(),
            processes: HashMap::new(),
            remote_sessions: HashMap::new(),
            ui: WineUiState::default(),
            selected_environment: None,
            selected_process: None,
        }
    }
}

impl Default for WineUiState {
    fn default() -> Self {
        Self {
            active_tab: WineTab::Environments,
            create_env_dialog_open: false,
            spawn_process_dialog_open: false,
            remote_desktop_dialog_open: false,
            env_name_input: String::new(),
            exe_path_input: String::new(),
            args_input: String::new(),
            selected_architecture: WineArchitecture::Win64,
            selected_windows_version: WineWindowsVersion::Windows10,
            selected_desktop_type: WineDesktopType::Vnc,
            resolution_width: "1920".to_string(),
            resolution_height: "1080".to_string(),
        }
    }
}

impl WineState {
    /// Create new Wine state
    pub fn new() -> Self {
        Self::default()
    }

    /// Update process status
    pub fn update_process_status(&mut self, process_id: Uuid, status: &str) {
        if let Some(process) = self.processes.get_mut(&process_id) {
            process.status = status.to_string();
        }
    }

    /// Add or update environment
    pub fn upsert_environment(&mut self, env_view: WineEnvironmentView) {
        self.environments.insert(env_view.id.clone(), env_view);
    }

    /// Add or update process
    pub fn upsert_process(&mut self, process_view: WineProcessView) {
        self.processes.insert(process_view.id, process_view);
    }

    /// Remove process
    pub fn remove_process(&mut self, process_id: &Uuid) {
        self.processes.remove(process_id);
    }

    /// Add remote session
    pub fn add_remote_session(&mut self, session_view: WineRemoteSessionView) {
        self.remote_sessions.insert(session_view.id, session_view);
    }

    /// Remove remote session
    pub fn remove_remote_session(&mut self, session_id: &Uuid) {
        self.remote_sessions.remove(session_id);
    }

    /// Get environment processes count
    pub fn get_environment_processes_count(&self, env_id: &str) -> usize {
        self.processes
            .values()
            .filter(|p| p.environment_id == env_id)
            .count()
    }
}

/// Render the main Wine panel
pub fn render_wine_panel<'a>(state: &'a WineState, scale: f32) -> Element<'a, Message, Theme, Renderer> {
    let spacing = (8.0 * scale).max(4.0);
    let padding = (12.0 * scale).max(6.0);

    let tabs = render_wine_tabs(&state.ui, scale, spacing);

    let content = match state.ui.active_tab {
        WineTab::Environments => render_environments_tab(state, scale, spacing, padding),
        WineTab::Processes => render_processes_tab(state, scale, spacing, padding),
        WineTab::RemoteDesktop => render_remote_desktop_tab(state, scale, spacing, padding),
        WineTab::Settings => render_settings_tab(state, scale, spacing, padding),
    };

    let mut main_column = column![]
        .spacing(spacing)
        .width(Length::Fill)
        .height(Length::Fill);

    main_column = main_column.push(tabs);
    main_column = main_column.push(content);

    // Render dialogs
    if state.ui.create_env_dialog_open {
        main_column = main_column.push(render_create_env_dialog(state, scale, spacing, padding));
    }

    if state.ui.spawn_process_dialog_open {
        main_column = main_column.push(render_spawn_process_dialog(state, scale, spacing, padding));
    }

    if state.ui.remote_desktop_dialog_open {
        main_column = main_column.push(render_remote_desktop_dialog(state, scale, spacing, padding));
    }

    container(main_column)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(padding)
        .style(container_style())
        .into()
}

/// Render Wine tabs
fn render_wine_tabs(ui_state: &WineUiState, scale: f32, spacing: f32) -> Element<'_, Message, Theme, Renderer> {
    let tab_width = Length::FillPortion(1);
    let tab_height = Length::Shrink;

    let env_tab = button(text("🍷 Environments").size(12.0 * scale))
        .width(tab_width)
        .height(tab_height)
        .style(if ui_state.active_tab == WineTab::Environments {
            button_style::active()
        } else {
            button_style::secondary()
        })
        .on_press(Message::WineEnvironmentTabSelected);

    let process_tab = button(text("⚙️ Processes").size(12.0 * scale))
        .width(tab_width)
        .height(tab_height)
        .style(if ui_state.active_tab == WineTab::Processes {
            button_style::active()
        } else {
            button_style::secondary()
        })
        .on_press(Message::WineProcessTabSelected);

    let remote_tab = button(text("🖥️ Remote").size(12.0 * scale))
        .width(tab_width)
        .height(tab_height)
        .style(if ui_state.active_tab == WineTab::RemoteDesktop {
            button_style::active()
        } else {
            button_style::secondary()
        })
        .on_press(Message::WineRemoteTabSelected);

    let settings_tab = button(text("⚙️ Settings").size(12.0 * scale))
        .width(tab_width)
        .height(tab_height)
        .style(if ui_state.active_tab == WineTab::Settings {
            button_style::active()
        } else {
            button_style::secondary()
        })
        .on_press(Message::WineSettingsTabSelected);

    row![env_tab, process_tab, remote_tab, settings_tab]
        .spacing(spacing)
        .width(Length::Fill)
        .align_y(Alignment::Center)
        .into()
}

/// Render environments tab
fn render_environments_tab(
    state: &WineState,
    scale: f32,
    spacing: f32,
    padding: f32,
) -> Element<'_, Message, Theme, Renderer> {
    let mut content = column![].spacing(spacing);

    // Header with create button
    let header = row![
        text("Wine Environments").size(16.0 * scale).bold(),
        Space::new().width(Length::Fill),
        button(text("+ Create Environment").size(12.0 * scale))
            .style(button_style::primary())
            .on_press(Message::WineCreateEnvironmentDialog)
    ]
    .align_y(Alignment::Center);

    content = content.push(header);
    content = content.push(horizontal_rule(1));

    // Environment list
    if state.environments.is_empty() {
        content = content.push(
            text("No Wine environments found. Create one to get started.")
                .size(14.0 * scale)
                .color(iced::Color::from_rgb(0.7, 0.7, 0.7)),
        );
    } else {
        let mut env_list = column![].spacing(spacing);

        for (env_id, env) in &state.environments {
            env_list = env_list.push(render_environment_item(state, env_id, env, scale, spacing));
        }

        content = content.push(
            scrollable(env_list)
                .width(Length::Fill)
                .height(Length::Fill)
                .direction(scrollable::Direction::Vertical(
                    scrollable::Scrollbar::new(),
                )),
        );
    }

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(padding)
        .into()
}

/// Render individual environment item
fn render_environment_item<'a>(
    state: &'a WineState,
    env_id: &'a str,
    env: &'a WineEnvironmentView,
    scale: f32,
    spacing: f32,
) -> Element<'a, Message, Theme, Renderer> {
    let is_selected = state.selected_environment.as_ref() == Some(env_id);
    let processes_count = state.get_environment_processes_count(env_id);

    let header = row![
        text(format!("🍷 {}", env.name)).size(14.0 * scale).bold(),
        Space::new().width(Length::Fill),
        text(format!("{} processes", processes_count))
            .size(12.0 * scale)
            .color(iced::Color::from_rgb(0.6, 0.6, 0.6)),
        button(text(if env.is_expanded { "▼" } else { "▶" }).size(12.0 * scale))
            .style(button_style::text())
            .on_press(Message::WineEnvironmentToggled(env_id.to_string()))
    ]
    .align_y(Alignment::Center);

    let mut item_content = column![header].spacing(spacing);

    if env.is_expanded {
        let details = column![
            row![
                text("Path:").size(12.0 * scale).bold(),
                Space::new().width(Length::Fixed(8.0 * scale)),
                text(&env.path).size(12.0 * scale)
            ],
            row![
                text("Architecture:").size(12.0 * scale).bold(),
                Space::new().width(Length::Fixed(8.0 * scale)),
                text(&env.architecture).size(12.0 * scale)
            ],
            row![
                text("Windows:").size(12.0 * scale).bold(),
                Space::new().width(Length::Fixed(8.0 * scale)),
                text(&env.windows_version).size(12.0 * scale)
            ]
        ]
        .spacing(spacing / 2.0);

        item_content = item_content.push(details);
        item_content = item_content.push(Space::new().height(Length::Fixed(spacing)));

        // Action buttons
        let actions = row![
            button(text("🚀 Launch App").size(11.0 * scale))
                .style(button_style::primary())
                .on_press(Message::WineSpawnProcessDialog(env_id.to_string())),
            button(text("🖥️ Remote Desktop").size(11.0 * scale))
                .style(button_style::secondary())
                .on_press(Message::WineRemoteDesktopDialog(env_id.to_string())),
            button(text("🗑️ Delete").size(11.0 * scale))
                .style(button_style::destructive())
                .on_press(Message::WineEnvironmentDelete(env_id.to_string()))
        ]
        .spacing(spacing);

        item_content = item_content.push(actions);
    }

    container(item_content)
        .width(Length::Fill)
        .padding(spacing)
        .style(if is_selected {
            container_style::selected()
        } else {
            container_style::card()
        })
        .into()
}

/// Render processes tab
fn render_processes_tab(
    state: &WineState,
    scale: f32,
    spacing: f32,
    padding: f32,
) -> Element<'_, Message, Theme, Renderer> {
    let mut content = column![].spacing(spacing);

    // Header
    let header = row![
        text("Running Processes").size(16.0 * scale).bold(),
        Space::new().width(Length::Fill),
        text(format!("{} active", state.processes.len()))
            .size(12.0 * scale)
            .color(iced::Color::from_rgb(0.6, 0.6, 0.6))
    ]
    .align_y(Alignment::Center);

    content = content.push(header);
    content = content.push(horizontal_rule(1));

    // Process list
    if state.processes.is_empty() {
        content = content.push(
            text("No processes are currently running.")
                .size(14.0 * scale)
                .color(iced::Color::from_rgb(0.7, 0.7, 0.7)),
        );
    } else {
        let mut process_list = column![].spacing(spacing);

        for (process_id, process) in &state.processes {
            process_list = process_list.push(render_process_item(process_id, process, scale, spacing));
        }

        content = content.push(
            scrollable(process_list)
                .width(Length::Fill)
                .height(Length::Fill)
                .direction(scrollable::Direction::Vertical(
                    scrollable::Scrollbar::new(),
                )),
        );
    }

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(padding)
        .into()
}

/// Render individual process item
fn render_process_item<'a>(
    process_id: &'a Uuid,
    process: &'a WineProcessView,
    scale: f32,
    spacing: f32,
) -> Element<'a, Message, Theme, Renderer> {
    let status_color = match process.status.as_str() {
        "Running" => iced::Color::from_rgb(0.2, 0.8, 0.2),
        "Starting" => iced::Color::from_rgb(1.0, 0.8, 0.0),
        "Finished" => iced::Color::from_rgb(0.2, 0.4, 0.8),
        _ => iced::Color::from_rgb(0.8, 0.2, 0.2),
    };

    let main_info = row![
        text(process_icon(&process.status)).size(16.0 * scale),
        Space::new().width(Length::Fixed(8.0 * scale)),
        column![
            text(&process.name).size(14.0 * scale).bold(),
            text(&process.executable).size(12.0 * scale)
        ]
        .spacing(2.0),
        Space::new().width(Length::Fill),
        column![
            text(&process.status).size(12.0 * scale).style(status_color),
            text(&process.uptime).size(11.0 * scale)
        ]
        .spacing(2.0)
    ]
    .align_y(Alignment::Center);

    let actions = row![
        if process.has_remote_desktop {
            button(text("🖥️ View Remote").size(11.0 * scale))
                .style(button_style::secondary())
                .on_press(Message::WineRemoteDesktopView(*process_id))
        } else {
            button(text("🖥️ Start Remote").size(11.0 * scale))
                .style(button_style::secondary())
                .on_press(Message::WineRemoteDesktopStart(*process_id))
        },
        button(text("🛑 Terminate").size(11.0 * scale))
            .style(button_style::destructive())
            .on_press(Message::WineProcessTerminate(*process_id))
    ]
    .spacing(spacing);

    column![main_info, actions]
        .spacing(spacing)
        .into()
}

/// Render remote desktop tab
fn render_remote_desktop_tab(
    state: &WineState,
    scale: f32,
    spacing: f32,
    padding: f32,
) -> Element<'_, Message, Theme, Renderer> {
    let mut content = column![].spacing(spacing);

    // Header
    let header = row![
        text("Remote Desktop Sessions").size(16.0 * scale).bold(),
        Space::new().width(Length::Fill),
        text(format!("{} active", state.remote_sessions.len()))
            .size(12.0 * scale)
            .color(iced::Color::from_rgb(0.6, 0.6, 0.6))
    ]
    .align_y(Alignment::Center);

    content = content.push(header);
    content = content.push(horizontal_rule(1));

    // Session list
    if state.remote_sessions.is_empty() {
        content = content.push(
            text("No remote desktop sessions are active.")
                .size(14.0 * scale)
                .color(iced::Color::from_rgb(0.7, 0.7, 0.7)),
        );
    } else {
        let mut session_list = column![].spacing(spacing);

        for (session_id, session) in &state.remote_sessions {
            session_list = session_list.push(render_remote_session_item(session_id, session, scale, spacing));
        }

        content = content.push(
            scrollable(session_list)
                .width(Length::Fill)
                .height(Length::Fill)
                .direction(scrollable::Direction::Vertical(
                    scrollable::Scrollbar::new(),
                )),
        );
    }

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(padding)
        .into()
}

/// Render individual remote session item
fn render_remote_session_item<'a>(
    session_id: &'a Uuid,
    session: &'a WineRemoteSessionView,
    scale: f32,
    spacing: f32,
) -> Element<'a, Message, Theme, Renderer> {
    let session_icon = match session.session_type.as_str() {
        "VNC" => "🖥️",
        "RDP" => "🪟",
        "X11" => "🎨",
        _ => "🖥️",
    };

    let main_info = row![
        text(session_icon).size(16.0 * scale),
        Space::new().width(Length::Fixed(8.0 * scale)),
        column![
            text(format!("{} Session", session.session_type)).size(14.0 * scale).bold(),
            text(format!("Port: {} | Resolution: {}x{}", session.port, session.resolution.0, session.resolution.1))
                .size(12.0 * scale)
        ]
        .spacing(2.0),
        Space::new().width(Length::Fill),
        column![
            text(&session.connection_url).size(11.0 * scale),
            text(&session.uptime).size(11.0 * scale)
        ]
        .spacing(2.0)
    ]
    .align_y(Alignment::Center);

    let actions = row![
        button(text("📋 Copy URL").size(11.0 * scale))
            .style(button_style::secondary())
            .on_press(Message::WineRemoteCopyUrl(*session_id)),
        button(text("🔗 Connect").size(11.0 * scale))
            .style(button_style::primary())
            .on_press(Message::WineRemoteConnect(*session_id)),
        button(text("🛑 Disconnect").size(11.0 * scale))
            .style(button_style::destructive())
            .on_press(Message::WineRemoteDisconnect(*session_id))
    ]
    .spacing(spacing);

    column![main_info, actions]
        .spacing(spacing)
        .into()
}

/// Render settings tab
fn render_settings_tab(
    _state: &WineState,
    scale: f32,
    spacing: f32,
    padding: f32,
) -> Element<'_, Message, Theme, Renderer> {
    let mut content = column![].spacing(spacing);

    // Header
    let header = text("Wine Settings").size(16.0 * scale).bold();
    content = content.push(header);
    content = content.push(horizontal_rule(1));

    // Settings sections
    let general_settings = column![
        text("General Settings").size(14.0 * scale).bold(),
        row![
            text("Wine Status:").size(12.0 * scale),
            Space::new().width(Length::Fixed(8.0 * scale)),
            text("✅ Available").size(12.0 * scale).color(iced::Color::from_rgb(0.2, 0.8, 0.2))
        ],
        row![
            text("System:").size(12.0 * scale),
            Space::new().width(Length::Fixed(8.0 * scale)),
            text(if std::path::Path::new("/etc/nixos").exists() {
                "NixOS (Optimized)"
            } else {
                "Linux (Standard)"
            })
            .size(12.0 * scale)
        ]
    ]
    .spacing(spacing / 2.0);

    let path_settings = column![
        text("Paths").size(14.0 * scale).bold(),
        row![
            text("Wine Environments:").size(12.0 * scale),
            Space::new().width(Length::Fixed(8.0 * scale)),
            text("~/.vedit/wine").size(12.0 * scale)
        ],
        row![
            text("Cache Directory:").size(12.0 * scale),
            Space::new().width(Length::Fixed(8.0 * scale)),
            text("~/.cache/vedit/wine").size(12.0 * scale)
        ]
    ]
    .spacing(spacing / 2.0);

    let remote_settings = column![
        text("Remote Desktop").size(14.0 * scale).bold(),
        row![
            text("VNC Port Range:").size(12.0 * scale),
            Space::new().width(Length::Fixed(8.0 * scale)),
            text("5900-5999").size(12.0 * scale)
        ],
        row![
            text("Default Resolution:").size(12.0 * scale),
            Space::new().width(Length::Fixed(8.0 * scale)),
            text("1920x1080").size(12.0 * scale)
        ]
    ]
    .spacing(spacing / 2.0);

    content = content.push(general_settings);
    content = content.push(Space::new().height(Length::Fixed(spacing)));
    content = content.push(path_settings);
    content = content.push(Space::new().height(Length::Fixed(spacing)));
    content = content.push(remote_settings);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(padding)
        .into()
}

/// Render create environment dialog
fn render_create_env_dialog<'a>(
    state: &'a WineState,
    scale: f32,
    spacing: f32,
    padding: f32,
) -> Element<'a, Message, Theme, Renderer> {
    let content = column![
        text("Create Wine Environment").size(16.0 * scale).bold(),
        horizontal_rule(1),
        column![
            text("Environment Name:").size(12.0 * scale),
            text_input("my-wine-app", &state.ui.env_name_input)
                .size(12.0 * scale)
                .style(text_input_style())
                .on_input(Message::WineEnvNameChanged)
        ]
        .spacing(spacing / 2.0),
        column![
            text("Architecture:").size(12.0 * scale),
            row![
                button(text("32-bit").size(11.0 * scale))
                    .style(if state.ui.selected_architecture == WineArchitecture::Win32 {
                        button_style::active()
                    } else {
                        button_style::secondary()
                    })
                    .on_press(Message::WineArchitectureSelected(WineArchitecture::Win32)),
                button(text("64-bit").size(11.0 * scale))
                    .style(if state.ui.selected_architecture == WineArchitecture::Win64 {
                        button_style::active()
                    } else {
                        button_style::secondary()
                    })
                    .on_press(Message::WineArchitectureSelected(WineArchitecture::Win64))
            ]
            .spacing(spacing)
        ]
        .spacing(spacing / 2.0),
        column![
            text("Windows Version:").size(12.0 * scale),
            // Windows version selection buttons would go here
            text("Windows 10 (default)").size(12.0 * scale)
        ]
        .spacing(spacing / 2.0),
        row![
            button(text("Cancel").size(12.0 * scale))
                .style(button_style::secondary())
                .on_press(Message::WineCreateEnvironmentDialog),
            Space::new().width(Length::Fill),
            button(text("Create").size(12.0 * scale))
                .style(button_style::primary())
                .on_press(Message::WineCreateEnvironment)
        ]
        .spacing(spacing)
    ]
    .spacing(spacing)
    .width(Length::Fill);

    container(content)
        .width(Length::Fill)
        .padding(padding)
        .style(container_style::modal())
        .into()
}

/// Render spawn process dialog
fn render_spawn_process_dialog<'a>(
    state: &'a WineState,
    scale: f32,
    spacing: f32,
    padding: f32,
) -> Element<'a, Message, Theme, Renderer> {
    let content = column![
        text("Launch Windows Application").size(16.0 * scale).bold(),
        horizontal_rule(1),
        column![
            text("Executable Path:").size(12.0 * scale),
            text_input("C:\\path\\to\\app.exe", &state.ui.exe_path_input)
                .size(12.0 * scale)
                .style(text_input_style())
                .on_input(Message::WineExePathChanged)
        ]
        .spacing(spacing / 2.0),
        column![
            text("Arguments:").size(12.0 * scale),
            text_input("Optional arguments...", &state.ui.args_input)
                .size(12.0 * scale)
                .style(text_input_style())
                .on_input(Message::WineArgsChanged)
        ]
        .spacing(spacing / 2.0),
        row![
            button(text("Cancel").size(12.0 * scale))
                .style(button_style::secondary())
                .on_press(Message::WineSpawnProcessDialog),
            Space::new().width(Length::Fill),
            button(text("Launch").size(12.0 * scale))
                .style(button_style::primary())
                .on_press(Message::WineSpawnProcess)
        ]
        .spacing(spacing)
    ]
    .spacing(spacing)
    .width(Length::Fill);

    container(content)
        .width(Length::Fill)
        .padding(padding)
        .style(container_style::modal())
        .into()
}

/// Render remote desktop dialog
fn render_remote_desktop_dialog<'a>(
    state: &'a WineState,
    scale: f32,
    spacing: f32,
    padding: f32,
) -> Element<'a, Message, Theme, Renderer> {
    let content = column![
        text("Remote Desktop Settings").size(16.0 * scale).bold(),
        horizontal_rule(1),
        column![
            text("Desktop Type:").size(12.0 * scale),
            row![
                button(text("VNC").size(11.0 * scale))
                    .style(if state.ui.selected_desktop_type == WineDesktopType::Vnc {
                        button_style::active()
                    } else {
                        button_style::secondary()
                    })
                    .on_press(Message::WineDesktopTypeSelected(WineDesktopType::Vnc)),
                button(text("RDP").size(11.0 * scale))
                    .style(if state.ui.selected_desktop_type == WineDesktopType::Rdp {
                        button_style::active()
                    } else {
                        button_style::secondary()
                    })
                    .on_press(Message::WineDesktopTypeSelected(WineDesktopType::Rdp)),
                button(text("X11").size(11.0 * scale))
                    .style(if state.ui.selected_desktop_type == WineDesktopType::X11 {
                        button_style::active()
                    } else {
                        button_style::secondary()
                    })
                    .on_press(Message::WineDesktopTypeSelected(WineDesktopType::X11))
            ]
            .spacing(spacing)
        ]
        .spacing(spacing / 2.0),
        row![
            column![
                text("Width:").size(12.0 * scale),
                text_input("1920", &state.ui.resolution_width)
                    .size(12.0 * scale)
                    .style(text_input_style())
                    .on_input(Message::WineResolutionWidthChanged)
            ]
            .width(Length::FillPortion(1)),
            Space::new().width(Length::Fixed(spacing)),
            column![
                text("Height:").size(12.0 * scale),
                text_input("1080", &state.ui.resolution_height)
                    .size(12.0 * scale)
                    .style(text_input_style())
                    .on_input(Message::WineResolutionHeightChanged)
            ]
            .width(Length::FillPortion(1))
        ]
        .spacing(spacing),
        row![
            button(text("Cancel").size(12.0 * scale))
                .style(button_style::secondary())
                .on_press(Message::WineRemoteDesktopDialog),
            Space::new().width(Length::Fill),
            button(text("Start Session").size(12.0 * scale))
                .style(button_style::primary())
                .on_press(Message::WineRemoteDesktopStart)
        ]
        .spacing(spacing)
    ]
    .spacing(spacing)
    .width(Length::Fill);

    container(content)
        .width(Length::Fill)
        .padding(padding)
        .style(container_style::modal())
        .into()
}

/// Get icon for process status
fn process_icon(status: &str) -> &'static str {
    match status {
        "Running" => "🟢",
        "Starting" => "🟡",
        "Finished" => "✅",
        _ => "❌",
    }
}