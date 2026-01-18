//! GUI integration for vedit-wine

use crate::environment::{Runtime, WindowsVersion, WineArchitecture, WineEnvironmentConfig};
use crate::error::WineError;
use crate::msbuild::{MSBuildAction, MSBuildEvent, MSBuildTarget};
use crate::process::{ProcessMode, WineProcessConfig};
use crate::proton::{EnvironmentDiscovery, ProtonInstallation};
use crate::remote_desktop::{DesktopType, RemoteDesktopConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use uuid::Uuid;

/// GUI messages for Wine operations
#[derive(Debug, Clone)]
pub enum WineGuiMessage {
    /// Create a new Wine environment
    CreateEnvironment {
        project_path: PathBuf,
        name: String,
        config: WineEnvironmentConfig,
    },

    /// Environment created successfully
    EnvironmentCreated {
        env_id: String,
        info: crate::environment::WineEnvironmentInfo,
    },

    /// Failed to create environment
    EnvironmentCreationFailed { name: String, error: String },

    /// Spawn a Windows application
    SpawnApp {
        env_id: String,
        exe_path: PathBuf,
        args: Vec<String>,
        config: WineProcessConfig,
    },

    /// Process spawned successfully
    ProcessSpawned {
        process_id: Uuid,
        info: crate::process::WineProcessInfo,
    },

    /// Failed to spawn process
    ProcessSpawnFailed { exe_path: PathBuf, error: String },

    /// Create remote desktop session
    CreateRemoteDesktop {
        process_id: Option<Uuid>,
        desktop_type: DesktopType,
        resolution: Option<(u32, u32)>,
    },

    /// Remote desktop session created
    RemoteDesktopCreated {
        session_id: Uuid,
        connection_info: crate::remote_desktop::ConnectionInfo,
    },

    /// Failed to create remote desktop
    RemoteDesktopCreationFailed { error: String },

    /// Update process status
    ProcessStatusUpdate {
        process_id: Uuid,
        status: crate::process::ProcessStatus,
    },

    /// Close process
    CloseProcess { process_id: Uuid },

    /// Close remote desktop session
    CloseRemoteDesktop { session_id: Uuid },

    /// List environments
    ListEnvironments,

    /// List active processes
    ListProcesses,

    /// Get environment details
    GetEnvironmentDetails { env_id: String },

    /// Get process details
    GetProcessDetails { process_id: Uuid },

    // === Build-related messages ===
    /// Start MSBuild compilation
    StartBuild {
        target: MSBuildTarget,
        configuration: String,
        platform: String,
        env_id: String,
        action: MSBuildAction,
    },

    /// Build started
    BuildStarted {
        target: String,
        configuration: String,
        platform: String,
    },

    /// Build output event
    BuildOutput(MSBuildEvent),

    /// Build completed
    BuildCompleted {
        success: bool,
        duration: Duration,
        warning_count: u32,
        error_count: u32,
    },

    /// Cancel ongoing build
    CancelBuild,

    /// Build cancelled
    BuildCancelled,

    // === Environment discovery messages ===
    /// Refresh environment discovery
    RefreshEnvironments,

    /// Environment discovery completed
    EnvironmentsDiscovered(EnvironmentDiscovery),

    /// Select a Wine/Proton environment
    SelectEnvironment { env_id: String },

    /// Selected environment changed
    EnvironmentSelected { env_id: String },
}

/// GUI state for Wine integration
#[derive(Debug, Clone)]
pub struct WineGuiState {
    /// Available Wine environments
    pub environments: HashMap<String, crate::environment::WineEnvironmentInfo>,

    /// Active Wine processes
    pub processes: HashMap<Uuid, crate::process::WineProcessInfo>,

    /// Active remote desktop sessions
    pub remote_desktop_sessions: HashMap<Uuid, crate::remote_desktop::ConnectionInfo>,

    /// Current project path
    pub current_project_path: Option<PathBuf>,

    /// Status of Wine system
    pub wine_status: WineSystemStatus,

    /// Loading states
    pub loading_states: LoadingStates,

    /// Error states
    pub errors: Vec<WineError>,

    /// Discovered Wine/Proton environments
    pub discovered_environments: Option<EnvironmentDiscovery>,

    /// Currently selected environment ID
    pub selected_environment: Option<String>,

    /// Active build state
    pub active_build: Option<ActiveBuild>,

    /// Detected Proton installations (for quick access)
    pub proton_installations: Vec<ProtonInstallation>,
}

/// System status for Wine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WineSystemStatus {
    /// Whether Wine is available
    pub wine_available: bool,

    /// Whether running on NixOS
    pub is_nixos: bool,

    /// Wine version (if available)
    pub wine_version: Option<String>,

    /// Available runtimes
    pub available_runtimes: Vec<Runtime>,

    /// Available remote desktop types
    pub available_desktop_types: Vec<DesktopType>,
}

impl Default for WineSystemStatus {
    fn default() -> Self {
        Self {
            wine_available: crate::WineManager::is_wine_available(),
            is_nixos: crate::WineManager::is_nixos(),
            wine_version: Self::get_wine_version(),
            available_runtimes: vec![
                Runtime::DotNet48,
                Runtime::DotNet60,
                Runtime::DotNet80,
                Runtime::Vc2015_2022,
                Runtime::DirectX9,
                Runtime::DirectX11,
            ],
            available_desktop_types: vec![DesktopType::Vnc, DesktopType::Rdp, DesktopType::X11],
        }
    }
}

impl WineSystemStatus {
    /// Get Wine version
    fn get_wine_version() -> Option<String> {
        use std::process::Command;

        Command::new("wine")
            .arg("--version")
            .output()
            .ok()
            .and_then(|output| {
                String::from_utf8_lossy(&output.stdout)
                    .trim()
                    .split_whitespace()
                    .next()
                    .map(|s| s.to_string())
            })
    }
}

/// Loading states for GUI
#[derive(Debug, Clone, Default)]
pub struct LoadingStates {
    pub creating_environment: bool,
    pub spawning_process: bool,
    pub creating_remote_desktop: bool,
    pub listing_environments: bool,
    pub listing_processes: bool,
    pub building: bool,
    pub discovering_environments: bool,
}

/// State of an active build
#[derive(Debug, Clone)]
pub struct ActiveBuild {
    /// Target being built
    pub target: String,

    /// Build configuration
    pub configuration: String,

    /// Build platform
    pub platform: String,

    /// When the build started
    pub start_time: Instant,

    /// Build events received so far
    pub events: Vec<MSBuildEvent>,

    /// Current build status
    pub status: BuildStatus,

    /// Warning count
    pub warning_count: u32,

    /// Error count
    pub error_count: u32,
}

impl ActiveBuild {
    /// Create a new active build
    pub fn new(target: String, configuration: String, platform: String) -> Self {
        Self {
            target,
            configuration,
            platform,
            start_time: Instant::now(),
            events: Vec::new(),
            status: BuildStatus::Running,
            warning_count: 0,
            error_count: 0,
        }
    }

    /// Add an event to the build
    pub fn add_event(&mut self, event: MSBuildEvent) {
        match &event {
            MSBuildEvent::Warning { .. } => self.warning_count += 1,
            MSBuildEvent::Error { .. } => self.error_count += 1,
            MSBuildEvent::Completed { success, .. } => {
                self.status = if *success {
                    BuildStatus::Completed { success: true }
                } else {
                    BuildStatus::Completed { success: false }
                };
            }
            _ => {}
        }
        self.events.push(event);
    }

    /// Get elapsed time
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }
}

/// Build status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildStatus {
    Running,
    Completed { success: bool },
    Cancelled,
}

impl Default for WineGuiState {
    fn default() -> Self {
        Self {
            environments: HashMap::new(),
            processes: HashMap::new(),
            remote_desktop_sessions: HashMap::new(),
            current_project_path: None,
            wine_status: WineSystemStatus::default(),
            loading_states: LoadingStates::default(),
            errors: Vec::new(),
            discovered_environments: None,
            selected_environment: None,
            active_build: None,
            proton_installations: Vec::new(),
        }
    }
}

impl WineGuiState {
    /// Create new GUI state
    pub fn new() -> Self {
        Self::default()
    }

    /// Set current project path
    pub fn set_project_path(&mut self, path: PathBuf) {
        self.current_project_path = Some(path);
        self.refresh_environments();
    }

    /// Add environment
    pub fn add_environment(
        &mut self,
        env_id: String,
        info: crate::environment::WineEnvironmentInfo,
    ) {
        self.environments.insert(env_id, info);
    }

    /// Remove environment
    pub fn remove_environment(&mut self, env_id: &str) {
        self.environments.remove(env_id);
    }

    /// Add process
    pub fn add_process(&mut self, process_id: Uuid, info: crate::process::WineProcessInfo) {
        self.processes.insert(process_id, info);
    }

    /// Update process status
    pub fn update_process_status(
        &mut self,
        process_id: Uuid,
        status: crate::process::ProcessStatus,
    ) {
        if let Some(process) = self.processes.get_mut(&process_id) {
            process.status = status;
        }
    }

    /// Remove process
    pub fn remove_process(&mut self, process_id: &Uuid) {
        self.processes.remove(process_id);
    }

    /// Add remote desktop session
    pub fn add_remote_desktop_session(
        &mut self,
        session_id: Uuid,
        info: crate::remote_desktop::ConnectionInfo,
    ) {
        self.remote_desktop_sessions.insert(session_id, info);
    }

    /// Remove remote desktop session
    pub fn remove_remote_desktop_session(&mut self, session_id: &Uuid) {
        self.remote_desktop_sessions.remove(session_id);
    }

    /// Add error
    pub fn add_error(&mut self, error: WineError) {
        self.errors.push(error);
    }

    /// Clear errors
    pub fn clear_errors(&mut self) {
        self.errors.clear();
    }

    /// Refresh environments list
    pub fn refresh_environments(&mut self) {
        // This would typically trigger an async operation to load environments
        // For now, we'll just clear the list
        if self.current_project_path.is_none() {
            self.environments.clear();
        }
    }

    /// Get environments for current project
    pub fn project_environments(
        &self,
    ) -> impl Iterator<Item = (&String, &crate::environment::WineEnvironmentInfo)> {
        self.environments.iter()
    }

    /// Get running processes
    pub fn running_processes(
        &self,
    ) -> impl Iterator<Item = (&Uuid, &crate::process::WineProcessInfo)> {
        self.processes.iter().filter(|(_, info)| {
            matches!(
                info.status,
                crate::process::ProcessStatus::Starting | crate::process::ProcessStatus::Running
            )
        })
    }

    /// Get processes for a specific environment
    pub fn processes_for_environment(
        &self,
        env_id: &str,
    ) -> Vec<(&Uuid, &crate::process::WineProcessInfo)> {
        self.processes
            .iter()
            .filter(|(_, info)| info.environment_id == env_id)
            .collect()
    }

    /// Start a new build
    pub fn start_build(&mut self, target: String, configuration: String, platform: String) {
        self.active_build = Some(ActiveBuild::new(target, configuration, platform));
        self.loading_states.building = true;
    }

    /// Add build event
    pub fn add_build_event(&mut self, event: MSBuildEvent) {
        if let Some(build) = &mut self.active_build {
            build.add_event(event);
        }
    }

    /// Cancel current build
    pub fn cancel_build(&mut self) {
        if let Some(build) = &mut self.active_build {
            build.status = BuildStatus::Cancelled;
        }
        self.loading_states.building = false;
    }

    /// Complete the current build
    pub fn complete_build(&mut self, success: bool) {
        if let Some(build) = &mut self.active_build {
            build.status = BuildStatus::Completed { success };
        }
        self.loading_states.building = false;
    }

    /// Check if a build is currently running
    pub fn is_building(&self) -> bool {
        self.active_build
            .as_ref()
            .map(|b| b.status == BuildStatus::Running)
            .unwrap_or(false)
    }

    /// Set discovered environments
    pub fn set_discovered_environments(&mut self, discovery: EnvironmentDiscovery) {
        self.proton_installations = discovery.proton_installations.clone();
        self.discovered_environments = Some(discovery);
        self.loading_states.discovering_environments = false;
    }

    /// Select an environment
    pub fn select_environment(&mut self, env_id: String) {
        self.selected_environment = Some(env_id);
    }

    /// Get the selected Proton installation
    pub fn selected_proton(&self) -> Option<&ProtonInstallation> {
        let env_id = self.selected_environment.as_ref()?;
        self.proton_installations.iter().find(|p| &p.name == env_id)
    }

    /// Check if any Wine/Proton environment is available
    pub fn has_any_environment(&self) -> bool {
        self.wine_status.wine_available || !self.proton_installations.is_empty()
    }
}

/// Default configurations for GUI
pub struct DefaultConfigs;

impl DefaultConfigs {
    /// Get default environment configuration for GUI
    pub fn default_environment() -> WineEnvironmentConfig {
        WineEnvironmentConfig {
            wine_version: None,
            windows_version: WindowsVersion::Windows10,
            dll_overrides: {
                let mut overrides = HashMap::new();
                overrides.insert(
                    "mscoree".to_string(),
                    crate::environment::DllOverride::Disable,
                );
                overrides.insert(
                    "mshtml".to_string(),
                    crate::environment::DllOverride::Disable,
                );
                overrides
            },
            runtimes: vec![Runtime::Vc2015_2022],
            display: crate::environment::DisplayConfig::default(),
            audio: crate::environment::AudioConfig::default(),
            architecture: WineArchitecture::Win64,
            environment_type: crate::environment::WineEnvironmentType::Wine,
        }
    }

    /// Get default process configuration for GUI
    pub fn default_process() -> WineProcessConfig {
        WineProcessConfig {
            working_directory: None,
            args: Vec::new(),
            env_vars: HashMap::new(),
            capture_output: true,
            mode: ProcessMode::Integrated,
            remote_desktop: None,
            startup_timeout: std::time::Duration::from_secs(30),
        }
    }

    /// Get default remote desktop configuration for GUI
    pub fn default_remote_desktop() -> RemoteDesktopConfig {
        RemoteDesktopConfig::default()
    }

    /// Get common runtime presets
    pub fn runtime_presets() -> Vec<(String, Vec<Runtime>)> {
        vec![
            (
                "Development".to_string(),
                vec![Runtime::Vc2015_2022, Runtime::DotNet48],
            ),
            (
                "Gaming".to_string(),
                vec![Runtime::DirectX9, Runtime::DirectX11, Runtime::Vc2015_2022],
            ),
            (
                "Modern .NET".to_string(),
                vec![Runtime::DotNet60, Runtime::DotNet80, Runtime::Vc2015_2022],
            ),
            (
                "Legacy".to_string(),
                vec![Runtime::DotNet20, Runtime::DotNet35, Runtime::Vc2008],
            ),
        ]
    }
}

/// Utility functions for GUI integration
pub struct WineGuiUtils;

impl WineGuiUtils {
    /// Format duration for display
    pub fn format_duration(duration: std::time::Duration) -> String {
        let total_seconds = duration.as_secs();
        let hours = total_seconds / 3600;
        let minutes = (total_seconds % 3600) / 60;
        let seconds = total_seconds % 60;

        if hours > 0 {
            format!("{}h {}m {}s", hours, minutes, seconds)
        } else if minutes > 0 {
            format!("{}m {}s", minutes, seconds)
        } else {
            format!("{}s", seconds)
        }
    }

    /// Format process status for display
    pub fn format_status(status: &crate::process::ProcessStatus) -> String {
        match status {
            crate::process::ProcessStatus::Starting => "🟡 Starting".to_string(),
            crate::process::ProcessStatus::Running => "🟢 Running".to_string(),
            crate::process::ProcessStatus::Finished => "✅ Finished".to_string(),
            crate::process::ProcessStatus::Failed(error) => format!("❌ Failed: {}", error),
            crate::process::ProcessStatus::Killed => "🛑 Killed".to_string(),
        }
    }

    /// Get icon for process status
    pub fn status_icon(status: &crate::process::ProcessStatus) -> &'static str {
        match status {
            crate::process::ProcessStatus::Starting => "⏳",
            crate::process::ProcessStatus::Running => "🟢",
            crate::process::ProcessStatus::Finished => "✅",
            crate::process::ProcessStatus::Failed(_) => "❌",
            crate::process::ProcessStatus::Killed => "🛑",
        }
    }

    /// Get color for process status (for GUI theming)
    pub fn status_color(status: &crate::process::ProcessStatus) -> &'static str {
        match status {
            crate::process::ProcessStatus::Starting => "orange",
            crate::process::ProcessStatus::Running => "green",
            crate::process::ProcessStatus::Finished => "blue",
            crate::process::ProcessStatus::Failed(_) => "red",
            crate::process::ProcessStatus::Killed => "gray",
        }
    }

    /// Validate executable path
    pub fn validate_executable(path: &PathBuf) -> Result<(), String> {
        if !path.exists() {
            return Err("File does not exist".to_string());
        }

        if !path.is_file() {
            return Err("Path is not a file".to_string());
        }

        let extension = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");

        if !["exe", "com", "bat", "cmd"].contains(&extension.to_lowercase().as_str()) {
            return Err("File does not appear to be a Windows executable".to_string());
        }

        Ok(())
    }
}
