//! Wine prefix management
//!
//! A Wine prefix is a directory containing a Windows-like environment
//! (registry, C: drive, etc.) where applications can be installed and run.

use crate::error::{WineError, WineResult};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use tokio::sync::mpsc;

/// Check if running on NixOS
pub fn is_nixos() -> bool {
    std::path::Path::new("/etc/nixos").exists() || std::env::var("NIX_PATH").is_ok()
}

/// Check if steam-run is available (for NixOS FHS compatibility)
pub fn has_steam_run() -> bool {
    which::which("steam-run").is_ok()
}

/// Get the wine binary name to use inside steam-run
/// On NixOS, we ALWAYS use "wine" from steam-run's FHS environment
/// Both system wine and Proton wine have library issues outside their runtime environments
fn get_wine_binary_for_steam_run(_wine_binary: &PathBuf) -> String {
    // Always use steam-run's wine on NixOS
    // - System wine has hardcoded nix store paths
    // - Proton wine expects Steam's runtime environment
    // steam-run provides a proper FHS wine that just works
    "wine".to_string()
}

/// Get the command to run Wine (wrapped with steam-run on NixOS if needed)
/// On NixOS, uses steam-run if available, otherwise falls back to nix-shell
fn wine_command(wine_binary: &PathBuf) -> Command {
    if is_nixos() {
        if has_steam_run() {
            let wine_bin = get_wine_binary_for_steam_run(wine_binary);
            eprintln!(
                "DEBUG: wine_command: Using steam-run with wine={}",
                wine_bin
            );
            let mut cmd = Command::new("steam-run");
            cmd.arg(&wine_bin);
            cmd
        } else {
            // Fallback: use nix-shell to get steam-run
            eprintln!("DEBUG: wine_command: Using nix-shell -p steam-run fallback");
            let mut cmd = Command::new("nix-shell");
            cmd.arg("-p")
                .arg("steam-run")
                .arg("--run")
                .arg("steam-run wine");
            cmd
        }
    } else {
        Command::new(wine_binary)
    }
}

/// Run a command with steam-run on NixOS, with proper argument handling
/// This is for complex commands where we need to pass multiple arguments
#[allow(dead_code)]
fn run_wine_command(
    wine_binary: &PathBuf,
    args: &[&str],
    prefix_path: &PathBuf,
) -> std::io::Result<std::process::ExitStatus> {
    if is_nixos() {
        let wine_bin = get_wine_binary_for_steam_run(wine_binary);
        if has_steam_run() {
            eprintln!(
                "DEBUG: run_wine_command: Using steam-run with wine={}",
                wine_bin
            );
            let mut cmd = Command::new("steam-run");
            cmd.arg(&wine_bin);
            for arg in args {
                cmd.arg(arg);
            }
            cmd.env("WINEPREFIX", prefix_path)
                .env("WINEDEBUG", "-all")
                .env("DISPLAY", "")
                .status()
        } else {
            // Fallback: use nix-shell with steam-run
            eprintln!("DEBUG: run_wine_command: Using nix-shell -p steam-run fallback");
            let wine_cmd = format!("wine {}", args.join(" "));
            Command::new("nix-shell")
                .arg("-p")
                .arg("steam-run")
                .arg("--run")
                .arg(format!("steam-run {}", wine_cmd))
                .env("WINEPREFIX", prefix_path)
                .env("WINEDEBUG", "-all")
                .env("DISPLAY", "")
                .status()
        }
    } else {
        let mut cmd = Command::new(wine_binary);
        for arg in args {
            cmd.arg(arg);
        }
        cmd.env("WINEPREFIX", prefix_path)
            .env("WINEDEBUG", "-all")
            .env("DISPLAY", "")
            .status()
    }
}

/// Run wineboot to initialize a prefix
fn run_wine_boot(
    wine_binary: &PathBuf,
    prefix_path: &PathBuf,
    arch: &str,
) -> std::io::Result<std::process::ExitStatus> {
    if is_nixos() {
        let wine_bin = get_wine_binary_for_steam_run(wine_binary);
        if has_steam_run() {
            eprintln!(
                "DEBUG: run_wine_boot: Using steam-run with wine={}",
                wine_bin
            );
            Command::new("steam-run")
                .arg(&wine_bin)
                .arg("wineboot")
                .arg("--init")
                .env("WINEPREFIX", prefix_path)
                .env("WINEARCH", arch)
                .env("WINEDEBUG", "-all")
                .env("DISPLAY", "")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
        } else {
            // Fallback: use nix-shell with steam-run
            eprintln!("DEBUG: run_wine_boot: Using nix-shell -p steam-run fallback");
            let wine_cmd = format!(
                "WINEPREFIX='{}' WINEARCH='{}' WINEDEBUG='-all' DISPLAY='' wine wineboot --init",
                prefix_path.display(),
                arch
            );
            Command::new("nix-shell")
                .arg("-p")
                .arg("steam-run")
                .arg("--run")
                .arg(format!("steam-run sh -c \"{}\"", wine_cmd))
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
        }
    } else {
        Command::new(wine_binary)
            .arg("wineboot")
            .arg("--init")
            .env("WINEPREFIX", prefix_path)
            .env("WINEARCH", arch)
            .env("WINEDEBUG", "-all")
            .env("DISPLAY", "")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
    }
}

/// Represents a configured Wine prefix
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WinePrefix {
    /// User-friendly name for this prefix
    pub name: String,

    /// Path to the Wine prefix directory
    pub path: PathBuf,

    /// Path to the Wine/Proton binary to use with this prefix
    pub wine_binary: PathBuf,

    /// Optional: Name of the Proton installation (if using Proton)
    pub proton_name: Option<String>,

    /// Architecture (win32 or win64)
    pub arch: WinePrefixArch,

    /// Whether VS Build Tools are installed
    pub has_build_tools: bool,

    /// Path to MSBuild.exe if known
    pub msbuild_path: Option<PathBuf>,
}

/// Wine prefix architecture
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum WinePrefixArch {
    Win32,
    Win64,
}

impl Default for WinePrefixArch {
    fn default() -> Self {
        Self::Win64
    }
}

impl std::fmt::Display for WinePrefixArch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WinePrefixArch::Win32 => write!(f, "32-bit"),
            WinePrefixArch::Win64 => write!(f, "64-bit"),
        }
    }
}

impl WinePrefix {
    /// Create a new Wine prefix
    pub fn create(
        name: String,
        path: PathBuf,
        wine_binary: PathBuf,
        proton_name: Option<String>,
        arch: WinePrefixArch,
    ) -> WineResult<Self> {
        // Create the prefix directory if it doesn't exist
        if !path.exists() {
            std::fs::create_dir_all(&path).map_err(|e| {
                WineError::EnvironmentCreationFailed(format!(
                    "Failed to create prefix directory: {}",
                    e
                ))
            })?;
        }

        // Initialize the Wine prefix by running wineboot
        let arch_env = match arch {
            WinePrefixArch::Win32 => "win32",
            WinePrefixArch::Win64 => "win64",
        };

        // Run wineboot with environment variables to minimize GUI issues
        // WINEDLLOVERRIDES disables some problematic DLLs
        // DISPLAY="" prevents X11 connection attempts on headless systems
        eprintln!("DEBUG: Initializing Wine prefix with wineboot...");

        // Use wineboot to initialize the prefix
        // On NixOS, this needs steam-run for FHS compatibility
        let status = run_wine_boot(&wine_binary, &path, arch_env);

        // Check if wineboot ran - even if it "fails", the prefix might be usable
        match status {
            Ok(s) => {
                if !s.success() {
                    // wineboot returned non-zero, but check if prefix was created anyway
                    if !path.join("system.reg").exists() && !path.join("drive_c").exists() {
                        // Try creating minimal structure manually
                        Self::create_minimal_prefix(&path)?;
                    }
                }
            }
            Err(e) => {
                // wineboot couldn't run at all - try manual creation
                eprintln!(
                    "wineboot failed to run: {}. Attempting manual prefix creation.",
                    e
                );
                Self::create_minimal_prefix(&path)?;
            }
        }

        // Verify the prefix has at least the basic structure
        let drive_c = path.join("drive_c");
        if !drive_c.exists() {
            Self::create_minimal_prefix(&path)?;
        }

        Ok(Self {
            name,
            path,
            wine_binary,
            proton_name,
            arch,
            has_build_tools: false,
            msbuild_path: None,
        })
    }

    /// Create a minimal prefix structure manually (fallback if wineboot fails)
    fn create_minimal_prefix(path: &PathBuf) -> WineResult<()> {
        let drive_c = path.join("drive_c");

        // Create essential directories
        let dirs = [
            drive_c.join("windows"),
            drive_c.join("windows/system32"),
            drive_c.join("windows/syswow64"),
            drive_c.join("Program Files"),
            drive_c.join("Program Files (x86)"),
            drive_c.join("users/Public"),
        ];

        for dir in &dirs {
            std::fs::create_dir_all(dir).map_err(|e| {
                WineError::EnvironmentCreationFailed(format!(
                    "Failed to create directory {}: {}",
                    dir.display(),
                    e
                ))
            })?;
        }

        Ok(())
    }

    /// Check if the prefix exists and is valid
    pub fn is_valid(&self) -> bool {
        self.path.exists() && self.path.join("system.reg").exists()
    }

    /// Get the drive_c path
    pub fn drive_c(&self) -> PathBuf {
        self.path.join("drive_c")
    }

    /// Find MSBuild.exe in this prefix
    pub fn find_msbuild(&self) -> Option<PathBuf> {
        let program_files = self.drive_c().join("Program Files (x86)");

        // Common MSBuild locations
        let candidates = [
            // VS 2022 Build Tools
            program_files
                .join("Microsoft Visual Studio/2022/BuildTools/MSBuild/Current/Bin/MSBuild.exe"),
            // VS 2019 Build Tools
            program_files
                .join("Microsoft Visual Studio/2019/BuildTools/MSBuild/Current/Bin/MSBuild.exe"),
            // VS 2017 Build Tools
            program_files
                .join("Microsoft Visual Studio/2017/BuildTools/MSBuild/15.0/Bin/MSBuild.exe"),
            // Standalone MSBuild
            program_files.join("MSBuild/Current/Bin/MSBuild.exe"),
            program_files.join("MSBuild/14.0/Bin/MSBuild.exe"),
        ];

        for candidate in candidates {
            if candidate.exists() {
                return Some(candidate);
            }
        }

        None
    }

    /// Run a Windows executable in this prefix
    pub fn run(&self, exe: &PathBuf, args: &[String]) -> WineResult<std::process::Child> {
        wine_command(&self.wine_binary)
            .arg(exe)
            .args(args)
            .env("WINEPREFIX", &self.path)
            .spawn()
            .map_err(|e| WineError::ProcessSpawnFailed(format!("Failed to spawn process: {}", e)))
    }

    /// Download VS Build Tools installer
    pub async fn download_vs_build_tools() -> WineResult<PathBuf> {
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("vedit");

        std::fs::create_dir_all(&cache_dir).map_err(|e| {
            WineError::EnvironmentCreationFailed(format!("Failed to create cache dir: {}", e))
        })?;

        let installer_path = cache_dir.join("vs_BuildTools.exe");

        // Check if already downloaded
        if installer_path.exists() {
            return Ok(installer_path);
        }

        // Download using curl or wget
        let url = "https://aka.ms/vs/17/release/vs_BuildTools.exe";

        let status = if which::which("curl").is_ok() {
            Command::new("curl")
                .arg("-L") // Follow redirects
                .arg("-o")
                .arg(&installer_path)
                .arg(url)
                .stdout(Stdio::null())
                .status()
        } else if which::which("wget").is_ok() {
            Command::new("wget")
                .arg("-O")
                .arg(&installer_path)
                .arg(url)
                .stdout(Stdio::null())
                .status()
        } else {
            return Err(WineError::EnvironmentCreationFailed(
                "Neither curl nor wget found. Please install one to download VS Build Tools."
                    .to_string(),
            ));
        };

        match status {
            Ok(s) if s.success() => Ok(installer_path),
            Ok(_) => Err(WineError::EnvironmentCreationFailed(
                "Failed to download VS Build Tools installer".to_string(),
            )),
            Err(e) => Err(WineError::EnvironmentCreationFailed(format!(
                "Failed to run download command: {}",
                e
            ))),
        }
    }

    /// Install VS Build Tools into this prefix
    /// Returns a channel that receives progress updates
    pub async fn install_vs_build_tools(
        &self,
        event_tx: mpsc::Sender<VsBuildToolsInstallEvent>,
    ) -> WineResult<()> {
        let _ = event_tx.send(VsBuildToolsInstallEvent::Downloading).await;

        // Download the installer
        let installer_path = Self::download_vs_build_tools().await?;

        let _ = event_tx.send(VsBuildToolsInstallEvent::Downloaded).await;
        let _ = event_tx.send(VsBuildToolsInstallEvent::Installing).await;

        // Run the installer with command-line arguments for workloads we need
        // --passive shows progress but doesn't require interaction
        // --wait waits for installation to complete
        // --add adds specific workloads
        let mut child = wine_command(&self.wine_binary)
            .arg(&installer_path)
            .arg("--passive")
            .arg("--wait")
            .arg("--norestart")
            // MSBuild tools
            .arg("--add")
            .arg("Microsoft.VisualStudio.Workload.MSBuildTools")
            // C++ build tools (includes MSBuild, compiler, linker)
            .arg("--add")
            .arg("Microsoft.VisualStudio.Workload.VCTools")
            // Include recommended components
            .arg("--includeRecommended")
            .env("WINEPREFIX", &self.path)
            .env("WINEDEBUG", "-all")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                WineError::EnvironmentCreationFailed(format!(
                    "Failed to start VS Build Tools installer: {}",
                    e
                ))
            })?;

        // Wait for the installer to complete
        let status = child.wait().map_err(|e| {
            WineError::EnvironmentCreationFailed(format!("VS Build Tools installer failed: {}", e))
        })?;

        if status.success() {
            let _ = event_tx.send(VsBuildToolsInstallEvent::Completed).await;
            Ok(())
        } else {
            let _ = event_tx
                .send(VsBuildToolsInstallEvent::Failed(
                    "Installer returned non-zero exit code".to_string(),
                ))
                .await;
            Err(WineError::EnvironmentCreationFailed(
                "VS Build Tools installation failed".to_string(),
            ))
        }
    }

    /// Start VS Build Tools installation in background (non-blocking)
    pub fn start_vs_build_tools_install(&self) -> WineResult<std::process::Child> {
        // First check if installer is cached
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("vedit");
        let installer_path = cache_dir.join("vs_BuildTools.exe");

        if !installer_path.exists() {
            return Err(WineError::EnvironmentCreationFailed(
                "VS Build Tools not downloaded yet. Call download_vs_build_tools() first."
                    .to_string(),
            ));
        }

        wine_command(&self.wine_binary)
            .arg(&installer_path)
            .arg("--passive")
            .arg("--wait")
            .arg("--norestart")
            .arg("--add")
            .arg("Microsoft.VisualStudio.Workload.MSBuildTools")
            .arg("--add")
            .arg("Microsoft.VisualStudio.Workload.VCTools")
            .arg("--includeRecommended")
            .env("WINEPREFIX", &self.path)
            .env("WINEDEBUG", "-all")
            .spawn()
            .map_err(|e| {
                WineError::EnvironmentCreationFailed(format!(
                    "Failed to start VS Build Tools installer: {}",
                    e
                ))
            })
    }
}

/// Events during VS Build Tools installation
#[derive(Debug, Clone)]
pub enum VsBuildToolsInstallEvent {
    Downloading,
    Downloaded,
    Installing,
    Progress(u8), // 0-100 percentage
    Completed,
    Failed(String),
}

/// Manager for Wine prefixes
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WinePrefixManager {
    /// List of configured prefixes
    pub prefixes: Vec<WinePrefix>,

    /// Currently selected prefix index
    pub selected: Option<usize>,
}

impl WinePrefixManager {
    /// Create a new prefix manager
    pub fn new() -> Self {
        Self::default()
    }

    /// Load prefixes from config file
    pub fn load() -> WineResult<Self> {
        let config_path = Self::config_path()?;

        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path).map_err(|e| {
                WineError::ConfigError(format!("Failed to read prefix config: {}", e))
            })?;

            serde_json::from_str(&content).map_err(|e| {
                WineError::ConfigError(format!("Failed to parse prefix config: {}", e))
            })
        } else {
            Ok(Self::new())
        }
    }

    /// Save prefixes to config file
    pub fn save(&self) -> WineResult<()> {
        let config_path = Self::config_path()?;

        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                WineError::ConfigError(format!("Failed to create config directory: {}", e))
            })?;
        }

        let content = serde_json::to_string_pretty(self)
            .map_err(|e| WineError::ConfigError(format!("Failed to serialize config: {}", e)))?;

        std::fs::write(&config_path, content)
            .map_err(|e| WineError::ConfigError(format!("Failed to write config: {}", e)))?;

        Ok(())
    }

    /// Get config file path
    fn config_path() -> WineResult<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| WineError::ConfigError("Could not find config directory".to_string()))?;

        Ok(config_dir.join("vedit").join("wine-prefixes.json"))
    }

    /// Add a new prefix
    pub fn add_prefix(&mut self, prefix: WinePrefix) {
        self.prefixes.push(prefix);
        if self.selected.is_none() {
            self.selected = Some(0);
        }
    }

    /// Remove a prefix by index
    pub fn remove_prefix(&mut self, index: usize) -> Option<WinePrefix> {
        if index < self.prefixes.len() {
            let prefix = self.prefixes.remove(index);

            // Adjust selected index
            if let Some(sel) = self.selected {
                if sel >= self.prefixes.len() {
                    self.selected = if self.prefixes.is_empty() {
                        None
                    } else {
                        Some(self.prefixes.len() - 1)
                    };
                } else if sel > index {
                    self.selected = Some(sel - 1);
                }
            }

            Some(prefix)
        } else {
            None
        }
    }

    /// Get the currently selected prefix
    pub fn selected_prefix(&self) -> Option<&WinePrefix> {
        self.selected.and_then(|i| self.prefixes.get(i))
    }

    /// Get mutable reference to selected prefix
    pub fn selected_prefix_mut(&mut self) -> Option<&mut WinePrefix> {
        self.selected.and_then(|i| self.prefixes.get_mut(i))
    }

    /// Select a prefix by index
    pub fn select(&mut self, index: usize) {
        if index < self.prefixes.len() {
            self.selected = Some(index);
        }
    }

    /// Check if any prefix has build tools
    pub fn has_build_tools(&self) -> bool {
        self.prefixes.iter().any(|p| p.has_build_tools)
    }
}
