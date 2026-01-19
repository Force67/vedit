//! MSBuild compilation support via Wine/Proton
//!
//! This module provides support for running MSBuild within Wine/Proton
//! environments to compile Visual Studio solutions and projects.

use crate::environment::WineEnvironment;
use crate::error::{WineError, WineResult};
use crate::prefix::{has_steam_run, is_nixos};
use crate::proton::ProtonInstallation;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc::{self, Receiver, Sender};

/// Request to run MSBuild
#[derive(Debug, Clone)]
pub struct MSBuildRequest {
    /// What to build (solution or project)
    pub target: MSBuildTarget,

    /// Build configuration (e.g., Debug|x64)
    pub configuration: String,

    /// Platform (e.g., x64, Win32)
    pub platform: String,

    /// Wine environment ID to use
    pub environment_id: String,

    /// Additional MSBuild arguments
    pub additional_args: Vec<String>,

    /// Working directory (defaults to target's parent directory)
    pub working_directory: Option<PathBuf>,

    /// Maximum parallel builds (-maxcpucount)
    pub max_cpu_count: Option<u32>,

    /// Verbosity level
    pub verbosity: MSBuildVerbosity,

    /// Build action (Build, Rebuild, Clean)
    pub action: MSBuildAction,
}

/// What to build
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MSBuildTarget {
    /// A Visual Studio solution file
    Solution(PathBuf),

    /// A Visual Studio project file
    Project(PathBuf),
}

impl MSBuildTarget {
    /// Get the path to the target file
    pub fn path(&self) -> &Path {
        match self {
            MSBuildTarget::Solution(p) => p,
            MSBuildTarget::Project(p) => p,
        }
    }
}

/// MSBuild verbosity levels
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub enum MSBuildVerbosity {
    Quiet,
    #[default]
    Minimal,
    Normal,
    Detailed,
    Diagnostic,
}

impl MSBuildVerbosity {
    fn as_arg(&self) -> &'static str {
        match self {
            MSBuildVerbosity::Quiet => "quiet",
            MSBuildVerbosity::Minimal => "minimal",
            MSBuildVerbosity::Normal => "normal",
            MSBuildVerbosity::Detailed => "detailed",
            MSBuildVerbosity::Diagnostic => "diagnostic",
        }
    }
}

/// MSBuild action to perform
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub enum MSBuildAction {
    #[default]
    Build,
    Rebuild,
    Clean,
}

impl MSBuildAction {
    fn as_target(&self) -> &'static str {
        match self {
            MSBuildAction::Build => "Build",
            MSBuildAction::Rebuild => "Rebuild",
            MSBuildAction::Clean => "Clean",
        }
    }
}

/// Events emitted during MSBuild execution
#[derive(Debug, Clone)]
pub enum MSBuildEvent {
    /// Build started
    Started {
        target: String,
        configuration: String,
        platform: String,
    },

    /// Raw output line
    Output(String),

    /// Parsed warning
    Warning {
        file: Option<String>,
        line: Option<u32>,
        column: Option<u32>,
        code: Option<String>,
        message: String,
    },

    /// Parsed error
    Error {
        file: Option<String>,
        line: Option<u32>,
        column: Option<u32>,
        code: Option<String>,
        message: String,
    },

    /// Project build started
    ProjectStarted { project: String },

    /// Project build completed
    ProjectCompleted { project: String, success: bool },

    /// Build completed
    Completed {
        success: bool,
        duration: Duration,
        warning_count: u32,
        error_count: u32,
    },
}

/// An active MSBuild session
pub struct MSBuildSession {
    request: MSBuildRequest,
    events_rx: Receiver<MSBuildEvent>,
    cancel_tx: Sender<()>,
    start_time: Instant,
}

impl MSBuildSession {
    /// Start a new MSBuild session using system Wine
    pub async fn start_with_wine(
        environment: &WineEnvironment,
        request: MSBuildRequest,
    ) -> WineResult<Self> {
        let msbuild_path = find_msbuild(&environment.prefix_path)?;
        let wine_executable = which::which("wine").map_err(|_| WineError::WineNotAvailable)?;

        Self::start_internal(
            &wine_executable,
            &msbuild_path,
            &environment.prefix_path,
            &environment.env_vars,
            request,
        )
        .await
    }

    /// Start a new MSBuild session using Proton
    pub async fn start_with_proton(
        proton: &ProtonInstallation,
        prefix_path: &Path,
        request: MSBuildRequest,
    ) -> WineResult<Self> {
        let msbuild_path = find_msbuild(prefix_path)?;
        let env_vars = proton.get_env_vars(prefix_path);

        Self::start_internal(
            &proton.wine_executable,
            &msbuild_path,
            prefix_path,
            &env_vars,
            request,
        )
        .await
    }

    /// Internal start implementation
    async fn start_internal(
        wine_executable: &Path,
        msbuild_path: &Path,
        prefix_path: &Path,
        env_vars: &std::collections::HashMap<String, String>,
        request: MSBuildRequest,
    ) -> WineResult<Self> {
        let (events_tx, events_rx) = mpsc::channel::<MSBuildEvent>(1000);
        let (cancel_tx, mut cancel_rx) = mpsc::channel::<()>(1);

        let target_path = request.target.path().to_path_buf();
        let working_dir = request
            .working_directory
            .clone()
            .or_else(|| target_path.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."));

        // Convert Linux path to Windows path for MSBuild
        let windows_target_path = linux_to_wine_path(&target_path, prefix_path);

        // Build MSBuild arguments
        // VSInstallRoot is critical for MSBuild to find MSVC tools
        let vs_root = r"C:\Program Files\Microsoft Visual Studio\2022\BuildTools";
        // SDK version from msvc-wine installation
        let sdk_version = "10.0.26100.0";

        let mut msbuild_args = vec![
            windows_target_path,
            format!("/p:Configuration={}", request.configuration),
            format!("/p:Platform={}", request.platform),
            format!("/p:VSInstallRoot={}", vs_root),
            format!("/p:WindowsTargetPlatformVersion={}", sdk_version),
            format!("/t:{}", request.action.as_target()),
            format!("/v:{}", request.verbosity.as_arg()),
            "/nologo".to_string(),
            "/consoleloggerparameters:Summary;ForceNoAlign".to_string(),
        ];

        if let Some(max_cpu) = request.max_cpu_count {
            msbuild_args.push(format!("/maxcpucount:{}", max_cpu));
        }

        msbuild_args.extend(request.additional_args.clone());

        // Build the command - use steam-run on NixOS for FHS compatibility
        let mut cmd = if is_nixos() && has_steam_run() {
            tracing::info!("Using steam-run for MSBuild on NixOS");
            let mut c = Command::new("steam-run");
            c.arg("wine");
            c.arg(msbuild_path);
            c.args(&msbuild_args);
            c
        } else {
            let mut c = Command::new(wine_executable);
            c.arg(msbuild_path);
            c.args(&msbuild_args);
            c
        };

        cmd.current_dir(&working_dir);

        // Set environment
        cmd.env("WINEPREFIX", prefix_path);
        cmd.env("WINEDEBUG", "-all");
        for (key, value) in env_vars {
            cmd.env(key, value);
        }

        // Capture output
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let start_time = Instant::now();

        // Send started event
        let _ = events_tx
            .send(MSBuildEvent::Started {
                target: target_path.display().to_string(),
                configuration: request.configuration.clone(),
                platform: request.platform.clone(),
            })
            .await;

        tracing::info!(
            "Starting MSBuild: {:?} {} /p:Configuration={} /p:Platform={}",
            msbuild_path,
            target_path.display(),
            request.configuration,
            request.platform
        );

        // Spawn the process
        let mut child = cmd.spawn().map_err(|e| {
            WineError::MSBuildFailed(format!("Failed to spawn MSBuild process: {}", e))
        })?;

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        // Spawn task to process output
        let events_tx_clone = events_tx.clone();

        tokio::spawn(async move {
            let mut warning_count = 0u32;
            let mut error_count = 0u32;
            let mut cancelled = false;

            // Process stdout
            if let Some(stdout) = stdout {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();

                loop {
                    tokio::select! {
                        biased;
                        _ = cancel_rx.recv() => {
                            let _ = child.kill().await;
                            cancelled = true;
                            break;
                        }
                        result = lines.next_line() => {
                            match result {
                                Ok(Some(line)) => {
                                    if let Some(event) = parse_msbuild_line(&line) {
                                        match &event {
                                            MSBuildEvent::Warning { .. } => warning_count += 1,
                                            MSBuildEvent::Error { .. } => error_count += 1,
                                            _ => {}
                                        }
                                        let _ = events_tx_clone.send(event).await;
                                    } else {
                                        let _ = events_tx_clone.send(MSBuildEvent::Output(line)).await;
                                    }
                                }
                                Ok(None) => break,
                                Err(_) => break,
                            }
                        }
                    }
                }
            }

            if cancelled {
                return;
            }

            // Process stderr
            if let Some(stderr) = stderr {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();

                while let Ok(Some(line)) = lines.next_line().await {
                    if let Some(event) = parse_msbuild_line(&line) {
                        match &event {
                            MSBuildEvent::Warning { .. } => warning_count += 1,
                            MSBuildEvent::Error { .. } => error_count += 1,
                            _ => {}
                        }
                        let _ = events_tx_clone.send(event).await;
                    } else {
                        let _ = events_tx_clone.send(MSBuildEvent::Output(line)).await;
                    }
                }
            }

            // Wait for process to complete
            let status = child.wait().await;
            let success = status.map(|s| s.success()).unwrap_or(false);
            let duration = start_time.elapsed();

            let _ = events_tx_clone
                .send(MSBuildEvent::Completed {
                    success,
                    duration,
                    warning_count,
                    error_count,
                })
                .await;
        });

        Ok(Self {
            request,
            events_rx,
            cancel_tx,
            start_time,
        })
    }

    /// Get the event receiver
    pub fn event_receiver(&mut self) -> &mut Receiver<MSBuildEvent> {
        &mut self.events_rx
    }

    /// Try to receive the next event without blocking
    pub fn try_recv(&mut self) -> Option<MSBuildEvent> {
        self.events_rx.try_recv().ok()
    }

    /// Cancel the build
    pub async fn cancel(&self) {
        let _ = self.cancel_tx.send(()).await;
    }

    /// Get the original request
    pub fn request(&self) -> &MSBuildRequest {
        &self.request
    }

    /// Get elapsed time since build started
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }
}

/// Find MSBuild.exe in a Wine prefix
pub fn find_msbuild(prefix_path: &Path) -> WineResult<PathBuf> {
    let drive_c = prefix_path.join("drive_c");

    // Common MSBuild locations (newest first)
    // IMPORTANT: amd64 paths must come first - they work correctly with VSInstallRoot
    let candidates = [
        // VS 2022 Build Tools - amd64 (preferred, works with msvc-wine)
        "Program Files/Microsoft Visual Studio/2022/BuildTools/MSBuild/Current/Bin/amd64/MSBuild.exe",
        "Program Files/Microsoft Visual Studio/2022/BuildTools/MSBuild/Current/Bin/MSBuild.exe",
        // VS 2022 Community/Professional/Enterprise
        "Program Files/Microsoft Visual Studio/2022/Community/MSBuild/Current/Bin/MSBuild.exe",
        "Program Files/Microsoft Visual Studio/2022/Professional/MSBuild/Current/Bin/MSBuild.exe",
        "Program Files/Microsoft Visual Studio/2022/Enterprise/MSBuild/Current/Bin/MSBuild.exe",
        // VS 2019 Build Tools
        "Program Files (x86)/Microsoft Visual Studio/2019/BuildTools/MSBuild/Current/Bin/MSBuild.exe",
        "Program Files (x86)/Microsoft Visual Studio/2019/BuildTools/MSBuild/Current/Bin/amd64/MSBuild.exe",
        // VS 2019 Community/Professional/Enterprise
        "Program Files (x86)/Microsoft Visual Studio/2019/Community/MSBuild/Current/Bin/MSBuild.exe",
        "Program Files (x86)/Microsoft Visual Studio/2019/Professional/MSBuild/Current/Bin/MSBuild.exe",
        "Program Files (x86)/Microsoft Visual Studio/2019/Enterprise/MSBuild/Current/Bin/MSBuild.exe",
        // VS 2017
        "Program Files (x86)/Microsoft Visual Studio/2017/BuildTools/MSBuild/15.0/Bin/MSBuild.exe",
        "Program Files (x86)/Microsoft Visual Studio/2017/Community/MSBuild/15.0/Bin/MSBuild.exe",
        // Legacy MSBuild
        "Program Files (x86)/MSBuild/14.0/Bin/MSBuild.exe",
        "Program Files (x86)/MSBuild/12.0/Bin/MSBuild.exe",
        // .NET Framework MSBuild
        "Windows/Microsoft.NET/Framework64/v4.0.30319/MSBuild.exe",
        "Windows/Microsoft.NET/Framework/v4.0.30319/MSBuild.exe",
    ];

    for candidate in candidates {
        let path = drive_c.join(candidate);
        if path.exists() {
            tracing::info!("Found MSBuild at: {}", path.display());
            return Ok(path);
        }
    }

    Err(WineError::MSBuildNotFound)
}

/// Convert a Linux path to a Wine/Windows path
pub fn linux_to_wine_path(linux_path: &Path, prefix_path: &Path) -> String {
    let path_str = linux_path.to_string_lossy();

    // If it's already a Windows-style path, return as-is
    if path_str.contains(':') && path_str.contains('\\') {
        return path_str.to_string();
    }

    // Check if path is within the Wine prefix's drive_c
    let drive_c = prefix_path.join("drive_c");
    if let Ok(relative) = linux_path.strip_prefix(&drive_c) {
        // Convert to C:\path\to\file
        let windows_path = format!("C:\\{}", relative.to_string_lossy().replace('/', "\\"));
        return windows_path;
    }

    // For paths outside the prefix, use Z: drive (root filesystem mapping)
    format!("Z:{}", path_str.replace('/', "\\"))
}

/// Convert a Wine/Windows path to a Linux path
pub fn wine_to_linux_path(wine_path: &str, prefix_path: &Path) -> PathBuf {
    let normalized = wine_path.replace('\\', "/");

    // Handle drive letters
    if normalized.len() >= 2 && normalized.chars().nth(1) == Some(':') {
        let drive_letter = normalized.chars().next().unwrap().to_ascii_lowercase();
        let rest = &normalized[2..].trim_start_matches('/');

        match drive_letter {
            'c' => prefix_path.join("drive_c").join(rest),
            'z' => PathBuf::from("/").join(rest),
            other => {
                // Check for dosdevices symlink
                let dosdevice = prefix_path.join("dosdevices").join(format!("{}:", other));
                if dosdevice.exists() {
                    if let Ok(target) = std::fs::read_link(&dosdevice) {
                        return target.join(rest);
                    }
                }
                // Fallback to drive_c
                prefix_path.join("drive_c").join(rest)
            }
        }
    } else {
        // No drive letter, assume relative path
        PathBuf::from(normalized)
    }
}

/// Parse an MSBuild output line into a structured event
fn parse_msbuild_line(line: &str) -> Option<MSBuildEvent> {
    let line = line.trim();

    if line.is_empty() {
        return None;
    }

    // Check for error pattern: file(line,col): error CODE: message
    // or: file(line): error CODE: message
    if let Some(event) = parse_diagnostic(line, "error") {
        return Some(event);
    }

    if let Some(event) = parse_diagnostic(line, "warning") {
        return Some(event);
    }

    // Check for project started pattern
    if line.contains("Build started") || line.starts_with("Project \"") {
        if let Some(start) = line.find('"') {
            if let Some(end) = line[start + 1..].find('"') {
                let project = &line[start + 1..start + 1 + end];
                return Some(MSBuildEvent::ProjectStarted {
                    project: project.to_string(),
                });
            }
        }
    }

    // Check for build succeeded/failed patterns
    if line.contains("Build succeeded") {
        // This will be captured by the Completed event
        return None;
    }

    if line.contains("Build FAILED") {
        // This will be captured by the Completed event
        return None;
    }

    None
}

/// Parse a diagnostic (error or warning) line
fn parse_diagnostic(line: &str, kind: &str) -> Option<MSBuildEvent> {
    // Pattern: file(line,col): error/warning CODE: message
    // or: file(line): error/warning CODE: message
    // or: error/warning CODE: message

    let kind_marker = format!(": {} ", kind);
    let kind_pos = line.to_lowercase().find(&kind_marker)?;

    // Extract file and location (everything before the kind marker)
    let prefix = &line[..kind_pos];
    let rest = &line[kind_pos + kind_marker.len()..];

    // Parse file(line,col) or file(line) pattern
    let (file, line_num, col_num) = if let Some(paren_start) = prefix.rfind('(') {
        if let Some(paren_end) = prefix[paren_start..].find(')') {
            let file = prefix[..paren_start].trim().to_string();
            let location = &prefix[paren_start + 1..paren_start + paren_end];

            let parts: Vec<&str> = location.split(',').collect();
            let line_num = parts.first().and_then(|s| s.trim().parse().ok());
            let col_num = parts.get(1).and_then(|s| s.trim().parse().ok());

            (Some(file), line_num, col_num)
        } else {
            (Some(prefix.trim().to_string()), None, None)
        }
    } else {
        (None, None, None)
    };

    // Parse CODE: message
    let (code, message) = if let Some(colon_pos) = rest.find(':') {
        let code = rest[..colon_pos].trim();
        let message = rest[colon_pos + 1..].trim();
        (
            if code.is_empty() {
                None
            } else {
                Some(code.to_string())
            },
            message.to_string(),
        )
    } else {
        (None, rest.to_string())
    };

    if kind == "error" {
        Some(MSBuildEvent::Error {
            file,
            line: line_num,
            column: col_num,
            code,
            message,
        })
    } else {
        Some(MSBuildEvent::Warning {
            file,
            line: line_num,
            column: col_num,
            code,
            message,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_error_line() {
        let line = r#"C:\src\main.cpp(42,15): error C2065: 'foo': undeclared identifier"#;
        if let Some(MSBuildEvent::Error {
            file,
            line,
            column,
            code,
            message,
        }) = parse_msbuild_line(line)
        {
            assert_eq!(file.as_deref(), Some(r#"C:\src\main.cpp"#));
            assert_eq!(line, Some(42));
            assert_eq!(column, Some(15));
            assert_eq!(code.as_deref(), Some("C2065"));
            assert!(message.contains("undeclared identifier"));
        } else {
            panic!("Expected error event");
        }
    }

    #[test]
    fn test_parse_warning_line() {
        let line = r#"C:\src\main.cpp(10): warning C4996: 'strcpy': This function or variable may be unsafe."#;
        if let Some(MSBuildEvent::Warning {
            file,
            line,
            column,
            code,
            message,
        }) = parse_msbuild_line(line)
        {
            assert_eq!(file.as_deref(), Some(r#"C:\src\main.cpp"#));
            assert_eq!(line, Some(10));
            assert_eq!(column, None);
            assert_eq!(code.as_deref(), Some("C4996"));
            assert!(message.contains("unsafe"));
        } else {
            panic!("Expected warning event");
        }
    }

    #[test]
    fn test_linux_to_wine_path() {
        let prefix = PathBuf::from("/home/user/.wine/prefix");
        let linux_path = prefix.join("drive_c").join("src").join("main.cpp");

        let wine_path = linux_to_wine_path(&linux_path, &prefix);
        assert_eq!(wine_path, r#"C:\src\main.cpp"#);
    }

    #[test]
    fn test_wine_to_linux_path() {
        let prefix = PathBuf::from("/home/user/.wine/prefix");
        let wine_path = r#"C:\src\main.cpp"#;

        let linux_path = wine_to_linux_path(wine_path, &prefix);
        assert_eq!(
            linux_path,
            prefix.join("drive_c").join("src").join("main.cpp")
        );
    }
}
