//! Configuration management for Wine integration

use crate::environment::{Runtime, WindowsVersion, WineArchitecture};
use crate::error::{WineError, WineResult};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Main Wine configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WineConfig {
    /// Default Wine version to use
    pub default_wine_version: Option<String>,

    /// Default Windows version
    pub default_windows_version: WindowsVersion,

    /// Default architecture
    pub default_architecture: WineArchitecture,

    /// Default runtimes to install
    pub default_runtimes: Vec<Runtime>,

    /// Global DLL overrides
    pub global_dll_overrides: std::collections::HashMap<String, String>,

    /// Remote desktop settings
    pub remote_desktop: RemoteDesktopGlobalConfig,

    /// Build system integration
    pub build_system: BuildSystemConfig,

    /// Paths and directories
    pub paths: PathConfig,
}

impl Default for WineConfig {
    fn default() -> Self {
        let mut global_dll_overrides = std::collections::HashMap::new();
        global_dll_overrides.insert("mscoree".to_string(), "".to_string());
        global_dll_overrides.insert("mshtml".to_string(), "".to_string());

        Self {
            default_wine_version: None,
            default_windows_version: WindowsVersion::Windows10,
            default_architecture: WineArchitecture::Win64,
            default_runtimes: vec![Runtime::Vc2015_2022, Runtime::DotNet48],
            global_dll_overrides,
            remote_desktop: RemoteDesktopGlobalConfig::default(),
            build_system: BuildSystemConfig::default(),
            paths: PathConfig::default(),
        }
    }
}

impl WineConfig {
    /// Load configuration from default locations
    pub fn load_default() -> WineResult<Self> {
        // Try to load from user config directory
        let config_path = dirs::config_dir()
            .ok_or_else(|| WineError::ConfigError("Could not find config directory".to_string()))?
            .join("vedit")
            .join("wine.json");

        if config_path.exists() {
            Self::load_from_file(&config_path)
        } else {
            // Create default config
            let config = Self::default();
            config.save_to_file(&config_path)?;
            Ok(config)
        }
    }

    /// Load configuration from a file
    pub fn load_from_file(path: &PathBuf) -> WineResult<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| WineError::ConfigError(format!("Failed to read config file: {}", e)))?;

        serde_json::from_str(&content)
            .map_err(|e| WineError::ConfigError(format!("Failed to parse config file: {}", e)))
    }

    /// Save configuration to a file
    pub fn save_to_file(&self, path: &PathBuf) -> WineResult<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                WineError::ConfigError(format!("Failed to create config directory: {}", e))
            })?;
        }

        let content = serde_json::to_string_pretty(self)
            .map_err(|e| WineError::ConfigError(format!("Failed to serialize config: {}", e)))?;

        std::fs::write(path, content)
            .map_err(|e| WineError::ConfigError(format!("Failed to write config file: {}", e)))?;

        Ok(())
    }

    /// Get Wine executable path
    pub fn wine_executable(&self) -> Result<PathBuf, WineError> {
        if let Some(version) = &self.default_wine_version {
            // Try to find version-specific Wine binary
            let wine_bin = format!("wine{}", version);
            which::which(&wine_bin).map_err(|_| {
                WineError::ConfigError(format!("Wine version {} not found: {}", version, wine_bin))
            })
        } else {
            // Use system Wine
            which::which("wine").map_err(|_| WineError::WineNotAvailable)
        }
    }

    /// Get winetricks executable path
    pub fn winetricks_executable(&self) -> Result<PathBuf, WineError> {
        which::which("winetricks").map_err(|_| {
            WineError::ConfigError("winetricks not found. Please install winetricks.".to_string())
        })
    }

    /// Get winecfg executable path
    pub fn winecfg_executable(&self) -> Result<PathBuf, WineError> {
        which::which("winecfg").map_err(|_| {
            WineError::ConfigError("winecfg not found. Please install wine-winecfg.".to_string())
        })
    }
}

/// Global remote desktop configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteDesktopGlobalConfig {
    /// Default VNC port range
    pub vnc_port_range: (u16, u16),

    /// Default RDP port range
    pub rdp_port_range: (u16, u16),

    /// Default remote desktop type
    pub default_type: crate::remote_desktop::DesktopType,

    /// Enable remote desktop by default
    pub enabled_by_default: bool,

    /// Security settings
    pub security: RemoteDesktopSecurityConfig,
}

impl Default for RemoteDesktopGlobalConfig {
    fn default() -> Self {
        Self {
            vnc_port_range: (5900, 5999),
            rdp_port_range: (3389, 3499),
            default_type: crate::remote_desktop::DesktopType::Vnc,
            enabled_by_default: false,
            security: RemoteDesktopSecurityConfig::default(),
        }
    }
}

/// Remote desktop security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteDesktopSecurityConfig {
    /// Generate random passwords
    pub generate_random_passwords: bool,

    /// Password length for generated passwords
    pub password_length: usize,

    /// Allow password-less connections
    pub allow_no_password: bool,

    /// Connection timeout in seconds
    pub connection_timeout: u64,
}

impl Default for RemoteDesktopSecurityConfig {
    fn default() -> Self {
        Self {
            generate_random_passwords: true,
            password_length: 12,
            allow_no_password: false,
            connection_timeout: 300, // 5 minutes
        }
    }
}

/// Build system configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildSystemConfig {
    /// Cross-compilation toolchains
    pub toolchains: std::collections::HashMap<String, ToolchainConfig>,

    /// Default output directory for Windows builds
    pub default_output_dir: PathBuf,

    /// Automatic deployment to Wine prefix
    pub auto_deploy: bool,

    /// Build commands for different project types
    pub build_commands: std::collections::HashMap<String, Vec<String>>,
}

impl Default for BuildSystemConfig {
    fn default() -> Self {
        let mut toolchains = std::collections::HashMap::new();
        toolchains.insert("mingw-w64".to_string(), ToolchainConfig::mingw_w64());
        toolchains.insert("msvc".to_string(), ToolchainConfig::msvc());

        let mut build_commands = std::collections::HashMap::new();
        build_commands.insert(
            "rust".to_string(),
            vec!["cargo build --target x86_64-pc-windows-gnu".to_string()],
        );
        build_commands.insert(
            "cpp".to_string(),
            vec![
                "mkdir -p build-windows".to_string(),
                "cd build-windows && x86_64-w64-mingw32-cmake ..".to_string(),
                "cd build-windows && make -j$(nproc)".to_string(),
            ],
        );
        build_commands.insert(
            "csharp".to_string(),
            vec!["dotnet build --configuration Release --runtime win-x64".to_string()],
        );

        Self {
            toolchains,
            default_output_dir: PathBuf::from("target/windows"),
            auto_deploy: true,
            build_commands,
        }
    }
}

/// Toolchain configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolchainConfig {
    /// Toolchain name
    pub name: String,

    /// Compiler executable
    pub compiler: PathBuf,

    /// Linker executable
    pub linker: Option<PathBuf>,

    /// Additional tools
    pub tools: std::collections::HashMap<String, PathBuf>,

    /// Environment variables
    pub env_vars: std::collections::HashMap<String, String>,

    /// Build flags
    pub build_flags: Vec<String>,
}

impl ToolchainConfig {
    /// Create MinGW-w64 toolchain configuration
    pub fn mingw_w64() -> Self {
        let mut tools = std::collections::HashMap::new();
        tools.insert(
            "windres".to_string(),
            PathBuf::from("x86_64-w64-mingw32-windres"),
        );
        tools.insert(
            "dlltool".to_string(),
            PathBuf::from("x86_64-w64-mingw32-dlltool"),
        );

        Self {
            name: "mingw-w64".to_string(),
            compiler: PathBuf::from("x86_64-w64-mingw32-gcc"),
            linker: Some(PathBuf::from("x86_64-w64-mingw32-gcc")),
            tools,
            env_vars: std::collections::HashMap::new(),
            build_flags: vec![
                "-static-libgcc".to_string(),
                "-static-libstdc++".to_string(),
            ],
        }
    }

    /// Create MSVC toolchain configuration (for cross-compilation via clang-cl)
    pub fn msvc() -> Self {
        Self {
            name: "msvc".to_string(),
            compiler: PathBuf::from("clang-cl"),
            linker: Some(PathBuf::from("lld-link")),
            tools: std::collections::HashMap::new(),
            env_vars: std::collections::HashMap::new(),
            build_flags: vec!["/target:x86_64-pc-windows-msvc".to_string()],
        }
    }
}

/// Path configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathConfig {
    /// Base directory for Wine environments
    pub wine_environments_base: PathBuf,

    /// Cache directory for downloaded runtimes
    pub cache_dir: PathBuf,

    /// Temporary directory
    pub temp_dir: PathBuf,

    /// Log directory
    pub log_dir: PathBuf,
}

impl Default for PathConfig {
    fn default() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("~"));
        let cache = dirs::cache_dir().unwrap_or_else(|| home.join(".cache"));

        Self {
            wine_environments_base: home.join(".vedit").join("wine"),
            cache_dir: cache.join("vedit").join("wine"),
            temp_dir: std::env::temp_dir().join("vedit-wine"),
            log_dir: home.join(".local").join("share").join("vedit").join("logs"),
        }
    }
}

/// Runtime-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    /// Runtime name
    pub name: String,

    /// Version
    pub version: String,

    /// Installation URL (for downloading)
    pub install_url: Option<String>,

    /// Install command (if not using winetricks)
    pub install_command: Option<Vec<String>>,

    /// Verification command to check if runtime is installed
    pub verify_command: Option<Vec<String>>,
}

impl RuntimeConfig {
    /// Create configuration for common runtimes
    pub fn for_runtime(runtime: &Runtime) -> Self {
        match runtime {
            Runtime::DotNet48 => Self {
                name: ".NET Framework 4.8".to_string(),
                version: "4.8".to_string(),
                install_url: None,
                install_command: None,
                verify_command: Some(vec![
                    "wine".to_string(),
                    "reg".to_string(),
                    "query".to_string(),
                    "HKEY_LOCAL_MACHINE\\\\SOFTWARE\\\\Microsoft\\\\NET Framework Setup\\\\NDP\\\\v4\\\\Full".to_string(),
                    "/v".to_string(),
                    "Release".to_string(),
                ]),
            },
            Runtime::Vc2015_2022 => Self {
                name: "Visual C++ 2015-2022".to_string(),
                version: "14.0".to_string(),
                install_url: None,
                install_command: None,
                verify_command: None,
            },
            _ => Self {
                name: format!("{:?}", runtime),
                version: "latest".to_string(),
                install_url: None,
                install_command: None,
                verify_command: None,
            },
        }
    }
}
