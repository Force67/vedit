//! Wine integration support for vedit
//!
//! This crate provides seamless integration with Wine applications,
//! including environment management, process control, and remote desktop
//! capabilities for running Windows applications within vedit.

pub mod environment;
pub mod process;
pub mod nix_integration;
pub mod remote_desktop;
pub mod config;
pub mod error;
pub mod gui_integration;

pub use environment::{WineEnvironment, WineEnvironmentConfig};
pub use process::{WineProcess, WineProcessConfig};
pub use nix_integration::{NixWineManager, NixEnvironment};
pub use remote_desktop::{RemoteDesktop, DesktopType};
pub use config::{WineConfig, RuntimeConfig};
pub use error::{WineError, WineResult};
pub use gui_integration::{WineGuiMessage, WineGuiState, WineSystemStatus, DefaultConfigs, WineGuiUtils};

/// Main Wine manager that coordinates all Wine-related functionality
pub struct WineManager {
    environments: std::collections::HashMap<String, WineEnvironment>,
    active_processes: std::collections::HashMap<uuid::Uuid, WineProcess>,
    config: WineConfig,
    #[cfg(feature = "nix-support")]
    nix_manager: Option<NixWineManager>,
}

impl WineManager {
    /// Create a new Wine manager with default configuration
    pub fn new() -> WineResult<Self> {
        let config = WineConfig::load_default()?;

        #[cfg(feature = "nix-support")]
        let nix_manager = NixWineManager::detect().ok();

        Ok(Self {
            environments: std::collections::HashMap::new(),
            active_processes: std::collections::HashMap::new(),
            config,
            #[cfg(feature = "nix-support")]
            nix_manager,
        })
    }

    /// Create a new Wine environment for a project
    pub async fn create_environment(
        &mut self,
        project_path: &std::path::Path,
        name: &str,
        config: WineEnvironmentConfig,
    ) -> WineResult<String> {
        let env_id = format!("{}-{}", name, uuid::Uuid::new_v4());

        #[cfg(feature = "nix-support")]
        let environment = if let Some(nix_manager) = &self.nix_manager {
            nix_manager.create_wine_environment(project_path, &env_id, config).await?
        } else {
            WineEnvironment::create(project_path, &env_id, config).await?
        };

        #[cfg(not(feature = "nix-support"))]
        let environment = WineEnvironment::create(project_path, &env_id, config).await?;

        self.environments.insert(env_id.clone(), environment);
        Ok(env_id)
    }

    /// Spawn a Windows application in the specified environment
    pub async fn spawn_app(
        &mut self,
        env_id: &str,
        exe_path: &std::path::Path,
        args: &[String],
        config: WineProcessConfig,
    ) -> WineResult<uuid::Uuid> {
        let environment = self.environments.get_mut(env_id)
            .ok_or_else(|| WineError::EnvironmentNotFound(env_id.to_string()))?;

        let process = environment.spawn_process(exe_path, args, config).await?;
        let process_id = process.id();

        self.active_processes.insert(process_id, process);
        Ok(process_id)
    }

    /// Get information about all active Wine processes
    pub fn active_processes(&self) -> &std::collections::HashMap<uuid::Uuid, WineProcess> {
        &self.active_processes
    }

    /// Get all managed Wine environments
    pub fn environments(&self) -> &std::collections::HashMap<String, WineEnvironment> {
        &self.environments
    }

    /// Check if Wine is available on the system
    pub fn is_wine_available() -> bool {
        which::which("wine").is_ok()
    }

    /// Detect if running on NixOS
    pub fn is_nixos() -> bool {
        std::path::Path::new("/etc/nixos").exists()
    }
}

impl Default for WineManager {
    fn default() -> Self {
        Self::new().expect("Failed to create Wine manager")
    }
}