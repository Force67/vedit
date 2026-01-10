//! Wine environment management

use crate::error::{WineError, WineResult};
use crate::process::{WineProcess, WineProcessConfig, WineProcessInfo};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;
use uuid::Uuid;

/// Configuration for a Wine environment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WineEnvironmentConfig {
    /// Wine version to use
    pub wine_version: Option<String>,

    /// Windows version to emulate
    pub windows_version: WindowsVersion,

    /// DLL overrides configuration
    pub dll_overrides: std::collections::HashMap<String, DllOverride>,

    /// Required runtimes (.NET, Visual C++, etc.)
    pub runtimes: Vec<Runtime>,

    /// Display settings
    pub display: DisplayConfig,

    /// Audio settings
    pub audio: AudioConfig,

    /// Whether to create a 32-bit or 64-bit prefix
    pub architecture: WineArchitecture,
}

impl Default for WineEnvironmentConfig {
    fn default() -> Self {
        let mut dll_overrides = std::collections::HashMap::new();
        dll_overrides.insert("mscoree".to_string(), DllOverride::Disable);
        dll_overrides.insert("mshtml".to_string(), DllOverride::Disable);

        Self {
            wine_version: None,
            windows_version: WindowsVersion::Windows10,
            dll_overrides,
            runtimes: Vec::new(),
            display: DisplayConfig::default(),
            audio: AudioConfig::default(),
            architecture: WineArchitecture::Win64,
        }
    }
}

/// Windows version emulation options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WindowsVersion {
    WindowsXP,
    Windows7,
    Windows8,
    Windows81,
    Windows10,
    Windows11,
}

/// DLL override behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DllOverride {
    /// Use native DLL
    Native,
    /// Use built-in DLL
    Builtin,
    /// Disable the DLL
    Disable,
    /// Use native first, fallback to built-in
    NativeBuiltin,
    /// Use built-in first, fallback to native
    BuiltinNative,
}

/// Required runtime packages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Runtime {
    /// .NET Framework 2.0
    DotNet20,
    /// .NET Framework 3.5
    DotNet35,
    /// .NET Framework 4.0
    DotNet40,
    /// .NET Framework 4.5
    DotNet45,
    /// .NET Framework 4.8
    DotNet48,
    /// .NET 6.0
    DotNet60,
    /// .NET 7.0
    DotNet70,
    /// .NET 8.0
    DotNet80,
    /// Visual C++ 2005
    Vc2005,
    /// Visual C++ 2008
    Vc2008,
    /// Visual C++ 2010
    Vc2010,
    /// Visual C++ 2012
    Vc2012,
    /// Visual C++ 2013
    Vc2013,
    /// Visual C++ 2015-2022
    Vc2015_2022,
    /// DirectX 9.0
    DirectX9,
    /// DirectX 11
    DirectX11,
    /// Mono (for .NET support)
    Mono,
}

/// Display configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayConfig {
    /// Enable DPI awareness
    pub dpi_aware: bool,
    /// Windowed mode by default
    pub windowed: bool,
    /// Virtual desktop resolution
    pub virtual_desktop: Option<(u32, u32)>,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            dpi_aware: true,
            windowed: true,
            virtual_desktop: None,
        }
    }
}

/// Audio configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    /// Audio driver to use
    pub driver: AudioDriver,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            driver: AudioDriver::PulseAudio,
        }
    }
}

/// Audio driver options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AudioDriver {
    PulseAudio,
    Alsa,
    OSS,
}

/// Wine architecture
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WineArchitecture {
    Win32,
    Win64,
}

/// A managed Wine environment
pub struct WineEnvironment {
    /// Unique identifier for this environment
    pub id: String,

    /// Path to the Wine prefix
    pub prefix_path: PathBuf,

    /// Project path this environment belongs to
    pub project_path: PathBuf,

    /// Configuration for this environment
    pub config: WineEnvironmentConfig,

    /// Environment variables
    pub env_vars: std::collections::HashMap<String, String>,

    /// Active processes in this environment
    pub active_processes: std::collections::HashMap<Uuid, WineProcessInfo>,
}

impl WineEnvironment {
    /// Create a new Wine environment
    pub async fn create(
        project_path: &Path,
        env_id: &str,
        config: WineEnvironmentConfig,
    ) -> WineResult<Self> {
        if !crate::WineManager::is_wine_available() {
            return Err(WineError::WineNotAvailable);
        }

        let prefix_path = project_path.join(".wine").join(env_id);
        std::fs::create_dir_all(&prefix_path)?;

        // Initialize Wine prefix
        Self::initialize_prefix(&prefix_path, &config).await?;

        // Apply configuration
        Self::apply_configuration(&prefix_path, &config).await?;

        let mut env_vars = std::collections::HashMap::new();
        env_vars.insert(
            "WINEPREFIX".to_string(),
            prefix_path.to_string_lossy().to_string(),
        );

        // Configure DLL overrides
        let dll_overrides = Self::build_dll_overrides(&config.dll_overrides);
        env_vars.insert("WINEDLLOVERRIDES".to_string(), dll_overrides);

        Ok(Self {
            id: env_id.to_string(),
            prefix_path,
            project_path: project_path.to_path_buf(),
            config,
            env_vars,
            active_processes: std::collections::HashMap::new(),
        })
    }

    /// Initialize a Wine prefix
    async fn initialize_prefix(
        prefix_path: &Path,
        config: &WineEnvironmentConfig,
    ) -> WineResult<()> {
        tracing::info!("Initializing Wine prefix at: {}", prefix_path.display());

        let mut cmd = Command::new("wineboot");

        // Configure architecture
        match config.architecture {
            WineArchitecture::Win32 => {
                cmd.arg("--init");
                cmd.env("WINEARCH", "win32");
            }
            WineArchitecture::Win64 => {
                cmd.arg("--init");
                cmd.env("WINEARCH", "win64");
            }
        }

        cmd.env("WINEPREFIX", prefix_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let output = cmd.output().await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(WineError::EnvironmentCreationFailed(stderr.to_string()));
        }

        tracing::info!("Wine prefix initialized successfully");
        Ok(())
    }

    /// Apply configuration to the Wine prefix
    async fn apply_configuration(
        prefix_path: &Path,
        config: &WineEnvironmentConfig,
    ) -> WineResult<()> {
        // Set Windows version
        Self::set_windows_version(prefix_path, &config.windows_version).await?;

        // Configure display settings
        if let Some((width, height)) = config.display.virtual_desktop {
            Self::enable_virtual_desktop(prefix_path, width, height).await?;
        }

        // Install required runtimes
        for runtime in &config.runtimes {
            Self::install_runtime(prefix_path, runtime).await?;
        }

        Ok(())
    }

    /// Set Windows version for the prefix
    async fn set_windows_version(prefix_path: &Path, version: &WindowsVersion) -> WineResult<()> {
        let version_str = match version {
            WindowsVersion::WindowsXP => "winxp",
            WindowsVersion::Windows7 => "win7",
            WindowsVersion::Windows8 => "win8",
            WindowsVersion::Windows81 => "win81",
            WindowsVersion::Windows10 => "win10",
            WindowsVersion::Windows11 => "win11",
        };

        let output = Command::new("winecfg")
            .arg("-v")
            .arg(version_str)
            .env("WINEPREFIX", prefix_path)
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::warn!("Failed to set Windows version: {}", stderr);
        }

        Ok(())
    }

    /// Enable virtual desktop
    async fn enable_virtual_desktop(prefix_path: &Path, width: u32, height: u32) -> WineResult<()> {
        // This would typically involve modifying the registry
        // For now, we'll set it via winecfg
        let resolution = format!("{}x{}", width, height);

        let output = Command::new("winecfg")
            .arg("-v")
            .arg(&resolution)
            .env("WINEPREFIX", prefix_path)
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::warn!("Failed to set virtual desktop: {}", stderr);
        }

        Ok(())
    }

    /// Install runtime using winetricks
    async fn install_runtime(prefix_path: &Path, runtime: &Runtime) -> WineResult<()> {
        let runtime_str = match runtime {
            Runtime::DotNet20 => "dotnet20",
            Runtime::DotNet35 => "dotnet35",
            Runtime::DotNet40 => "dotnet40",
            Runtime::DotNet45 => "dotnet45",
            Runtime::DotNet48 => "dotnet48",
            Runtime::DotNet60 => "dotnet60",
            Runtime::DotNet70 => "dotnet70",
            Runtime::DotNet80 => "dotnet80",
            Runtime::Vc2005 => "vc2005express",
            Runtime::Vc2008 => "vc2008express",
            Runtime::Vc2010 => "vc2010express",
            Runtime::Vc2012 => "vc2012express",
            Runtime::Vc2013 => "vc2013express",
            Runtime::Vc2015_2022 => "vc2015_2022",
            Runtime::DirectX9 => "d3dx9",
            Runtime::DirectX11 => "d3dcompiler_47",
            Runtime::Mono => "mono",
        };

        tracing::info!("Installing runtime: {}", runtime_str);

        let output = Command::new("winetricks")
            .arg("-q")
            .arg(runtime_str)
            .env("WINEPREFIX", prefix_path)
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::warn!("Failed to install runtime {}: {}", runtime_str, stderr);
            // Don't fail the entire environment creation for runtime installation failures
        } else {
            tracing::info!("Successfully installed runtime: {}", runtime_str);
        }

        Ok(())
    }

    /// Build WINEDLLOVERRIDES string
    fn build_dll_overrides(overrides: &std::collections::HashMap<String, DllOverride>) -> String {
        overrides
            .iter()
            .map(|(dll, override_type)| {
                let override_str = match override_type {
                    DllOverride::Native => "n",
                    DllOverride::Builtin => "b",
                    DllOverride::Disable => "",
                    DllOverride::NativeBuiltin => "n,b",
                    DllOverride::BuiltinNative => "b,n",
                };
                format!("{}={}", dll, override_str)
            })
            .collect::<Vec<_>>()
            .join(";")
    }

    /// Spawn a process in this Wine environment
    pub async fn spawn_process(
        &mut self,
        exe_path: &Path,
        args: &[String],
        config: WineProcessConfig,
    ) -> WineResult<WineProcess> {
        let process = WineProcess::spawn(self, exe_path, args, config).await?;
        let process_id = process.id();
        self.active_processes
            .insert(process_id, process.clone_info());
        Ok(process)
    }

    /// Get all active processes
    pub fn active_processes(&self) -> &std::collections::HashMap<Uuid, WineProcessInfo> {
        &self.active_processes
    }

    /// Remove a completed process
    pub fn remove_process(&mut self, process_id: Uuid) -> Option<WineProcessInfo> {
        self.active_processes.remove(&process_id)
    }

    /// Get information about the environment
    pub fn info(&self) -> WineEnvironmentInfo {
        WineEnvironmentInfo {
            id: self.id.clone(),
            prefix_path: self.prefix_path.clone(),
            architecture: self.config.architecture.clone(),
            windows_version: self.config.windows_version.clone(),
            installed_runtimes: self.config.runtimes.clone(),
            active_process_count: self.active_processes.len(),
        }
    }
}

/// Information about a Wine environment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WineEnvironmentInfo {
    pub id: String,
    pub prefix_path: PathBuf,
    pub architecture: WineArchitecture,
    pub windows_version: WindowsVersion,
    pub installed_runtimes: Vec<Runtime>,
    pub active_process_count: usize,
}
