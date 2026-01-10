//! Wine process management

use crate::environment::WineEnvironment;
use crate::error::{WineError, WineResult};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::{Child, Command};
use tokio::time::{Duration, timeout};
use uuid::Uuid;

/// Configuration for spawning a Wine process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WineProcessConfig {
    /// Working directory for the process
    pub working_directory: Option<PathBuf>,

    /// Command line arguments
    pub args: Vec<String>,

    /// Environment variables (in addition to Wine environment)
    pub env_vars: std::collections::HashMap<String, String>,

    /// Whether to capture stdout/stderr
    pub capture_output: bool,

    /// Process execution mode
    pub mode: ProcessMode,

    /// Remote desktop configuration
    pub remote_desktop: Option<RemoteDesktopConfig>,

    /// Timeout for process startup
    pub startup_timeout: Duration,
}

impl Default for WineProcessConfig {
    fn default() -> Self {
        Self {
            working_directory: None,
            args: Vec::new(),
            env_vars: std::collections::HashMap::new(),
            capture_output: true,
            mode: ProcessMode::Integrated,
            remote_desktop: None,
            startup_timeout: Duration::from_secs(30),
        }
    }
}

/// Process execution modes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProcessMode {
    /// Process runs integrated with vedit (appears as a panel)
    Integrated,
    /// Process runs in separate window
    Windowed,
    /// Process runs headless (background service)
    Headless,
    /// Process runs with remote desktop access
    RemoteDesktop,
}

/// Remote desktop configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteDesktopConfig {
    /// Type of remote desktop
    pub desktop_type: RemoteDesktopType,

    /// Port for VNC/RDP
    pub port: u16,

    /// Resolution
    pub resolution: (u32, u32),

    /// Password for remote access
    pub password: Option<String>,
}

/// Remote desktop types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RemoteDesktopType {
    /// VNC server
    Vnc,
    /// RDP server
    Rdp,
    /// X11 forwarding
    X11,
}

/// Status of a Wine process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProcessStatus {
    /// Process is starting
    Starting,
    /// Process is running
    Running,
    /// Process has finished successfully
    Finished,
    /// Process failed with error
    Failed(String),
    /// Process was killed
    Killed,
}

/// A managed Wine process
pub struct WineProcess {
    /// Unique identifier
    pub id: Uuid,

    /// Executable path
    pub exe_path: PathBuf,

    /// Command line arguments
    pub args: Vec<String>,

    /// Current status
    pub status: ProcessStatus,

    /// Process start time
    pub start_time: std::time::Instant,

    /// Wine environment this process belongs to
    pub environment_id: String,

    /// Process configuration
    pub config: WineProcessConfig,

    /// Child process handle (None if process has exited)
    pub child: Option<Child>,
}

impl WineProcess {
    /// Spawn a new Wine process
    pub async fn spawn(
        environment: &WineEnvironment,
        exe_path: &Path,
        args: &[String],
        config: WineProcessConfig,
    ) -> WineResult<Self> {
        if !exe_path.exists() {
            return Err(WineError::ExecutableNotFound(
                exe_path.to_string_lossy().to_string(),
            ));
        }

        let process_id = Uuid::new_v4();
        let mut cmd = Command::new("wine");

        // Configure Wine environment
        for (key, value) in &environment.env_vars {
            cmd.env(key, value);
        }

        // Add process-specific environment variables
        for (key, value) in &config.env_vars {
            cmd.env(key, value);
        }

        // Set working directory
        if let Some(working_dir) = &config.working_directory {
            cmd.current_dir(working_dir);
        } else {
            cmd.current_dir(exe_path.parent().unwrap_or_else(|| Path::new(".")));
        }

        // Configure based on mode
        match &config.mode {
            ProcessMode::Integrated => {
                // Set up for integration with vedit
                cmd.env("VEDIT_INTEGRATED", "1");
            }
            ProcessMode::Windowed => {
                // Standard windowed mode
                cmd.env("VEDIT_WINDOWED", "1");
            }
            ProcessMode::Headless => {
                // Headless mode (virtual display)
                cmd.env("DISPLAY", ":99"); // Assuming Xvfb is running
            }
            ProcessMode::RemoteDesktop => {
                // Set up remote desktop
                if let Some(remote_config) = &config.remote_desktop {
                    Self::configure_remote_desktop(&mut cmd, remote_config)?;
                }
            }
        }

        // Build arguments
        let mut wine_args = vec![exe_path.to_string_lossy().to_string()];
        wine_args.extend_from_slice(args);

        cmd.args(wine_args);

        // Configure output capture
        if config.capture_output {
            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::piped());
        }

        tracing::info!(
            "Spawning Wine process: {:?} with args: {:?}",
            exe_path,
            args
        );

        let child = cmd.spawn().map_err(|e| {
            WineError::ProcessSpawnFailed(format!("Failed to spawn wine process: {}", e))
        })?;

        Ok(Self {
            id: process_id,
            exe_path: exe_path.to_path_buf(),
            args: args.to_vec(),
            status: ProcessStatus::Starting,
            start_time: std::time::Instant::now(),
            environment_id: environment.id.clone(),
            config,
            child: Some(child),
        })
    }

    /// Configure remote desktop for the process
    fn configure_remote_desktop(
        cmd: &mut Command,
        remote_config: &RemoteDesktopConfig,
    ) -> WineResult<()> {
        match remote_config.desktop_type {
            RemoteDesktopType::Vnc => {
                // Set up VNC server
                cmd.env("VEDIT_VNC_PORT", remote_config.port.to_string());
                cmd.env(
                    "VEDIT_VNC_RESOLUTION",
                    format!(
                        "{}x{}",
                        remote_config.resolution.0, remote_config.resolution.1
                    ),
                );
                if let Some(password) = &remote_config.password {
                    cmd.env("VEDIT_VNC_PASSWORD", password);
                }
            }
            RemoteDesktopType::Rdp => {
                // Set up RDP server
                cmd.env("VEDIT_RDP_PORT", remote_config.port.to_string());
                cmd.env(
                    "VEDIT_RDP_RESOLUTION",
                    format!(
                        "{}x{}",
                        remote_config.resolution.0, remote_config.resolution.1
                    ),
                );
            }
            RemoteDesktopType::X11 => {
                // Set up X11 forwarding
                cmd.env("DISPLAY", format!(":{}", remote_config.port));
            }
        }
        Ok(())
    }

    /// Wait for the process to complete
    pub async fn wait(&mut self) -> WineResult<i32> {
        let child = self
            .child
            .as_mut()
            .ok_or_else(|| WineError::ProcessNotFound(self.id))?;

        let status = timeout(self.config.startup_timeout, child.wait())
            .await
            .map_err(|_| WineError::ProcessSpawnFailed("Process startup timed out".to_string()))?
            .map_err(|e| {
                WineError::ProcessSpawnFailed(format!("Failed to wait for process: {}", e))
            })?;

        let exit_code = status.code().unwrap_or(-1);

        if status.success() {
            self.status = ProcessStatus::Finished;
        } else {
            self.status = ProcessStatus::Failed(format!("Process exited with code {}", exit_code));
        }

        self.child = None;
        Ok(exit_code)
    }

    /// Kill the process
    pub async fn kill(&mut self) -> WineResult<()> {
        if let Some(child) = &mut self.child {
            child.kill().await.map_err(|e| {
                WineError::ProcessSpawnFailed(format!("Failed to kill process: {}", e))
            })?;
            self.status = ProcessStatus::Killed;
            self.child = None;
        }
        Ok(())
    }

    /// Check if the process is still running
    pub async fn is_running(&mut self) -> bool {
        if let Some(child) = &mut self.child {
            match child.try_wait() {
                Ok(Some(status)) => {
                    // Process has exited
                    if status.success() {
                        self.status = ProcessStatus::Finished;
                    } else {
                        self.status = ProcessStatus::Failed(format!(
                            "Process exited with code {}",
                            status.code().unwrap_or(-1)
                        ));
                    }
                    self.child = None;
                    false
                }
                Ok(None) => {
                    // Process is still running
                    if matches!(self.status, ProcessStatus::Starting) {
                        self.status = ProcessStatus::Running;
                    }
                    true
                }
                Err(_) => {
                    // Error checking status, assume not running
                    self.status =
                        ProcessStatus::Failed("Error checking process status".to_string());
                    self.child = None;
                    false
                }
            }
        } else {
            false
        }
    }

    /// Get process information
    pub fn info(&self) -> WineProcessInfo {
        WineProcessInfo {
            id: self.id,
            exe_path: self.exe_path.clone(),
            args: self.args.clone(),
            status: self.status.clone(),
            start_time: self.start_time,
            environment_id: self.environment_id.clone(),
            mode: self.config.mode.clone(),
            is_running: self.child.is_some(),
            uptime: self.start_time.elapsed(),
        }
    }

    /// Get a clone of the process info that can be sent across threads
    pub fn clone_info(&self) -> WineProcessInfo {
        self.info()
    }

    /// Get process ID (unique identifier)
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// Try to get stdout from the process (if captured)
    pub async fn try_read_stdout(&mut self) -> WineResult<Option<String>> {
        // This would require implementing stdout reading from the child process
        // For now, return None
        Ok(None)
    }

    /// Try to get stderr from the process (if captured)
    pub async fn try_read_stderr(&mut self) -> WineResult<Option<String>> {
        // This would require implementing stderr reading from the child process
        // For now, return None
        Ok(None)
    }
}

/// Information about a Wine process
#[derive(Debug, Clone, Serialize)]
pub struct WineProcessInfo {
    pub id: Uuid,
    pub exe_path: PathBuf,
    pub args: Vec<String>,
    pub status: ProcessStatus,
    #[serde(serialize_with = "serialize_instant")]
    pub start_time: std::time::Instant,
    pub environment_id: String,
    pub mode: ProcessMode,
    pub is_running: bool,
    pub uptime: std::time::Duration,
}

impl<'de> Deserialize<'de> for WineProcessInfo {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct WineProcessInfoData {
            id: Uuid,
            exe_path: PathBuf,
            args: Vec<String>,
            status: ProcessStatus,
            #[serde(deserialize_with = "deserialize_instant")]
            start_time: std::time::Instant,
            environment_id: String,
            mode: ProcessMode,
            is_running: bool,
            uptime: std::time::Duration,
        }

        let data = WineProcessInfoData::deserialize(deserializer)?;
        Ok(Self {
            id: data.id,
            exe_path: data.exe_path,
            args: data.args,
            status: data.status,
            start_time: data.start_time,
            environment_id: data.environment_id,
            mode: data.mode,
            is_running: data.is_running,
            uptime: data.uptime,
        })
    }
}

fn serialize_instant<S>(instant: &std::time::Instant, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    // Instant can't be directly serialized; we store elapsed time from now
    // so it can be reconstructed on deserialize. This loses absolute time reference
    // but preserves relative timing information.
    // TODO(Vince): Consider storing SystemTime alongside Instant for better serialization
    let elapsed = instant.elapsed();
    elapsed.serialize(serializer)
}

fn deserialize_instant<'de, D>(deserializer: D) -> Result<std::time::Instant, D::Error>
where
    D: Deserializer<'de>,
{
    use std::time::{Instant, SystemTime};

    let duration = std::time::Duration::deserialize(deserializer)?;

    // We'll use system time as the basis for creating a new instant
    let system_time = SystemTime::UNIX_EPOCH + duration;

    // Convert to instant by measuring from now
    match system_time.elapsed() {
        Ok(elapsed) => Ok(Instant::now() - elapsed),
        Err(_) => Ok(Instant::now()),
    }
}
