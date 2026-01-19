// Commands module - some async commands are API for future use
#![allow(dead_code)]

use crate::debugger::DebuggerType;
use rfd::FileDialog;
use std::fs;
use std::path::PathBuf;
use vedit_config::{WorkspaceConfig, WorkspaceMetadata};
use vedit_core::Document;
use vedit_debugger_gdb::{
    Breakpoint as DebuggerBreakpoint, GdbSession, LaunchConfig as DebuggerLaunchConfig,
};

#[derive(Debug, Clone)]
pub struct SaveDocumentRequest {
    pub path: Option<String>,
    pub contents: String,
    pub suggested_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SaveKeymapRequest {
    pub path: String,
    pub contents: String,
}

#[derive(Debug, Clone)]
pub struct WorkspaceData {
    pub root: String,
    pub config: WorkspaceConfig,
    pub metadata: WorkspaceMetadata,
}
#[derive(Debug, Clone)]
pub struct DebugSessionBreakpoint {
    pub file: String,
    pub line: u32,
    pub condition: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DebugSessionRequest {
    pub executable: String,
    pub working_directory: String,
    pub arguments: Vec<String>,
    pub breakpoints: Vec<DebugSessionBreakpoint>,
    pub launch_script: Option<String>,
    pub debugger_type: DebuggerType,
}

pub async fn pick_keymap_location(current: Option<String>) -> Result<Option<String>, String> {
    let mut dialog = FileDialog::new();

    if let Some(current) = current {
        let path = PathBuf::from(&current);
        if let Some(parent) = path.parent() {
            dialog = dialog.set_directory(parent);
        }
        if let Some(file_name) = path.file_name().and_then(|name| name.to_str()) {
            dialog = dialog.set_file_name(file_name);
        }
    }

    Ok(dialog
        .save_file()
        .map(|path| path.to_string_lossy().to_string()))
}

pub async fn pick_document() -> Result<Option<Document>, String> {
    if let Some(path) = FileDialog::new().pick_file() {
        // Use smart loading for better performance with large files
        let document = Document::from_path_smart(&path)
            .map_err(|err| format!("Failed to read file: {}", err))?;
        Ok(Some(document))
    } else {
        Ok(None)
    }
}

pub async fn load_document_from_path(path: String) -> Result<Document, String> {
    // Use smart loading - memory maps files >5MB for faster startup
    Document::from_path_smart(&path).map_err(|err| format!("Failed to read file: {}", err))
}

pub async fn pick_workspace() -> Result<Option<WorkspaceData>, String> {
    if let Some(path) = FileDialog::new().pick_folder() {
        let root_string = path.to_string_lossy().to_string();
        let mut config = WorkspaceConfig::load_or_default(&path)
            .map_err(|err| format!("Failed to load workspace config: {}", err))?;
        let metadata = WorkspaceMetadata::load_or_default(&path)
            .map_err(|err| format!("Failed to load workspace metadata: {}", err))?;
        if config.name.is_none() {
            if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
                config.name = Some(name.to_string());
            }
        }
        Ok(Some(WorkspaceData {
            root: root_string,
            config,
            metadata,
        }))
    } else {
        Ok(None)
    }
}

pub async fn load_workspace_from_path(path: PathBuf) -> Result<Option<WorkspaceData>, String> {
    if path.exists() && path.is_dir() {
        let root_string = path.to_string_lossy().to_string();
        let mut config = WorkspaceConfig::load_or_default(&path)
            .map_err(|err| format!("Failed to load workspace config: {}", err))?;
        let metadata = WorkspaceMetadata::load_or_default(&path)
            .map_err(|err| format!("Failed to load workspace metadata: {}", err))?;
        if config.name.is_none() {
            if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
                config.name = Some(name.to_string());
            }
        }
        Ok(Some(WorkspaceData {
            root: root_string,
            config,
            metadata,
        }))
    } else {
        Ok(None)
    }
}

pub async fn load_workspace_from_path_with_files(
    path: PathBuf,
    _session_state: crate::session::SessionState,
) -> Result<Option<WorkspaceData>, String> {
    if path.exists() && path.is_dir() {
        println!("DEBUG: Loading workspace from: {}", path.display());

        let root_string = path.to_string_lossy().to_string();
        let mut config = WorkspaceConfig::load_or_default(&path)
            .map_err(|err| format!("Failed to load workspace config: {}", err))?;
        let metadata = WorkspaceMetadata::load_or_default(&path)
            .map_err(|err| format!("Failed to load workspace metadata: {}", err))?;
        if config.name.is_none() {
            if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
                config.name = Some(name.to_string());
            }
        }

        Ok(Some(WorkspaceData {
            root: root_string,
            config,
            metadata,
        }))
    } else {
        Ok(None)
    }
}

pub async fn pick_solution() -> Result<Option<WorkspaceData>, String> {
    if let Some(path) = FileDialog::new().pick_file() {
        let root_dir = path
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let root_string = root_dir.to_string_lossy().to_string();
        let mut config = WorkspaceConfig::load_or_default(&root_dir)
            .map_err(|err| format!("Failed to load workspace config: {}", err))?;
        let metadata = WorkspaceMetadata::load_or_default(&root_dir)
            .map_err(|err| format!("Failed to load workspace metadata: {}", err))?;
        if config.name.is_none() {
            if let Some(name) = path.file_stem().and_then(|stem| stem.to_str()) {
                config.name = Some(name.to_string());
            }
        }

        Ok(Some(WorkspaceData {
            root: root_string,
            config,
            metadata,
        }))
    } else {
        Ok(None)
    }
}

pub async fn save_document(request: SaveDocumentRequest) -> Result<Option<String>, String> {
    let SaveDocumentRequest {
        path,
        contents,
        suggested_name,
    } = request;

    if let Some(path) = path {
        let target = PathBuf::from(path);
        fs::write(&target, contents).map_err(|err| format!("Failed to write file: {}", err))?;
        return Ok(Some(target.to_string_lossy().to_string()));
    }

    let mut dialog = FileDialog::new();
    if let Some(name) = suggested_name.as_deref() {
        if !name.trim().is_empty() && name != "(scratch)" {
            dialog = dialog.set_file_name(name);
        }
    }

    if let Some(target) = dialog.save_file() {
        fs::write(&target, contents).map_err(|err| format!("Failed to write file: {}", err))?;
        Ok(Some(target.to_string_lossy().to_string()))
    } else {
        Ok(None)
    }
}

pub async fn save_keymap(request: SaveKeymapRequest) -> Result<String, String> {
    let SaveKeymapRequest { path, contents } = request;
    let target = PathBuf::from(&path);

    if let Some(parent) = target.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)
                .map_err(|err| format!("Failed to create keymap directory: {}", err))?;
        }
    }

    fs::write(&target, contents).map_err(|err| format!("Failed to write keymap: {}", err))?;

    Ok(target.to_string_lossy().to_string())
}

pub async fn save_workspace_config(
    root: String,
    config: WorkspaceConfig,
) -> Result<String, String> {
    config
        .save(&root)
        .map_err(|err| format!("Failed to save workspace config: {}", err))?;
    Ok(root)
}

pub async fn save_workspace_metadata(
    root: String,
    metadata: WorkspaceMetadata,
) -> Result<String, String> {
    metadata
        .save(&root)
        .map_err(|err| format!("Failed to save workspace metadata: {}", err))?;
    Ok(root)
}

#[derive(Debug, Clone)]
pub enum DebugSession {
    Gdb(GdbSession),
    Vedit(vedit_debugger::VeditSession),
}

pub async fn start_debug_session(request: DebugSessionRequest) -> Result<DebugSession, String> {
    let DebugSessionRequest {
        executable,
        working_directory,
        arguments,
        breakpoints,
        launch_script,
        debugger_type,
    } = request;

    match debugger_type {
        DebuggerType::Gdb => {
            let config = DebuggerLaunchConfig {
                executable: PathBuf::from(executable),
                working_directory: PathBuf::from(working_directory),
                arguments,
                breakpoints: breakpoints
                    .into_iter()
                    .map(|bp| DebuggerBreakpoint {
                        file: PathBuf::from(bp.file),
                        line: bp.line,
                        condition: bp.condition,
                    })
                    .collect(),
                launch_script,
                gdb_path: None,
            };

            vedit_debugger_gdb::spawn_session(config)
                .map(DebugSession::Gdb)
                .map_err(|err| err.to_string())
        }
        DebuggerType::Vedit => {
            let config = vedit_debugger::LaunchConfig {
                executable: PathBuf::from(executable),
                working_directory: PathBuf::from(working_directory),
                arguments,
                breakpoints: vec![], // For now, no breakpoints for vedit debugger
            };

            vedit_debugger::spawn_session(config)
                .map(DebugSession::Vedit)
                .map_err(|err| err.to_string())
        }
    }
}

pub async fn load_solution_from_path(path: String) -> Result<Option<WorkspaceData>, String> {
    let path_buf = PathBuf::from(&path);

    let root_dir = path_buf
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let root_string = root_dir.to_string_lossy().to_string();
    let mut config = WorkspaceConfig::load_or_default(&root_dir)
        .map_err(|err| format!("Failed to load workspace config: {}", err))?;
    let metadata = WorkspaceMetadata::load_or_default(&root_dir)
        .map_err(|err| format!("Failed to load workspace metadata: {}", err))?;
    if config.name.is_none() {
        if let Some(name) = path_buf.file_stem().and_then(|stem| stem.to_str()) {
            config.name = Some(name.to_string());
        }
    }

    Ok(Some(WorkspaceData {
        root: root_string,
        config,
        metadata,
    }))
}

/// Build action type
#[derive(Debug, Clone, Copy, Hash)]
pub enum BuildAction {
    Build,
    Rebuild,
    Clean,
}

/// Request to build a solution/project via Wine
#[derive(Debug, Clone, Hash)]
pub struct WineBuildRequest {
    /// Path to the solution or project file
    pub target_path: PathBuf,
    /// Wine prefix path
    pub prefix_path: PathBuf,
    /// Path to MSBuild.exe inside the prefix
    pub msbuild_path: PathBuf,
    /// Build configuration (e.g., "Release", "Debug")
    pub configuration: String,
    /// Platform (e.g., "x64", "Win32")
    pub platform: String,
    /// Build action
    pub action: BuildAction,
}

/// Result of a build operation
#[derive(Debug, Clone)]
pub struct WineBuildResult {
    pub success: bool,
    pub output: String,
    pub target: String,
}

/// Event emitted during a streaming build
#[derive(Debug, Clone)]
pub enum WineBuildEvent {
    /// A line of output from the build process
    Output(String),
    /// Build completed with success/failure status
    Completed { success: bool },
    /// Build failed to start
    Failed(String),
}

/// Run MSBuild via Wine with streaming output - returns a stream for use with Task::run
pub fn wine_build_stream(
    request: WineBuildRequest,
) -> impl iced::futures::Stream<Item = WineBuildEvent> {
    iced::stream::channel(100, move |mut output| {
        let request = request.clone();
        async move {
            use iced::futures::SinkExt;

            match run_wine_build_streaming(request, &mut output).await {
                Ok(success) => {
                    let _ = output.send(WineBuildEvent::Completed { success }).await;
                }
                Err(e) => {
                    let _ = output.send(WineBuildEvent::Failed(e)).await;
                }
            }
        }
    })
}

/// Internal function to run the build with streaming output
async fn run_wine_build_streaming(
    request: WineBuildRequest,
    output: &mut iced::futures::channel::mpsc::Sender<WineBuildEvent>,
) -> Result<bool, String> {
    use iced::futures::SinkExt;
    use std::process::Stdio;
    use tokio::io::{AsyncBufReadExt, BufReader};
    use tokio::process::Command;

    let WineBuildRequest {
        target_path,
        prefix_path,
        msbuild_path,
        configuration,
        platform,
        action,
    } = request;

    // Convert Linux path to Wine path (Z: drive)
    let wine_target = format!("Z:{}", target_path.to_string_lossy().replace('/', "\\"));

    // Convert msbuild_path (which is relative to drive_c) to Windows path
    let msbuild_windows = if msbuild_path.starts_with(prefix_path.join("drive_c")) {
        let relative = msbuild_path
            .strip_prefix(prefix_path.join("drive_c"))
            .map_err(|_| "Invalid MSBuild path".to_string())?;
        format!("C:{}", relative.to_string_lossy().replace('/', "\\"))
    } else {
        format!("Z:{}", msbuild_path.to_string_lossy().replace('/', "\\"))
    };

    // Build action target
    let action_target = match action {
        BuildAction::Build => "Build",
        BuildAction::Rebuild => "Rebuild",
        BuildAction::Clean => "Clean",
    };

    // VS root for MSBuild to find tools
    let vs_root = r"C:\Program Files\Microsoft Visual Studio\2022\BuildTools";
    let sdk_version = "10.0.26100.0";

    // Check if we're on NixOS and need steam-run
    let is_nixos = std::path::Path::new("/etc/nixos").exists() || std::env::var("NIX_PATH").is_ok();
    let has_steam_run = which::which("steam-run").is_ok();

    // Ensure temp directory exists for MSVC compiler
    let temp_dir = prefix_path.join("drive_c/Temp");
    let _ = std::fs::create_dir_all(&temp_dir);
    let windows_temp_path = r"C:\Temp";

    // Get essential environment variables (uppercase only to avoid .NET case collisions)
    let home = std::env::var("HOME").unwrap_or_default();
    let path = std::env::var("PATH").unwrap_or_default();
    let display = std::env::var("DISPLAY").unwrap_or_default();
    let xdg_runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_default();

    // Build the command
    let mut cmd = if is_nixos && has_steam_run {
        let mut c = Command::new("steam-run");
        c.arg("wine");
        c
    } else {
        Command::new("wine")
    };

    cmd.arg(&msbuild_windows)
        .arg(&wine_target)
        .arg(format!("/p:Configuration={}", configuration))
        .arg(format!("/p:Platform={}", platform))
        .arg(format!("/p:VSInstallRoot={}", vs_root))
        .arg(format!("/p:WindowsTargetPlatformVersion={}", sdk_version))
        .arg(format!("/t:{}", action_target))
        .arg("/nologo")
        .arg("/verbosity:minimal")
        .env_clear()
        .env("HOME", &home)
        .env("PATH", &path)
        .env("DISPLAY", &display)
        .env("XDG_RUNTIME_DIR", &xdg_runtime_dir)
        .env("WINEPREFIX", &prefix_path)
        .env("WINEDEBUG", "-all")
        .env("TMP", &windows_temp_path)
        .env("TEMP", &windows_temp_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // Spawn the process
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to start MSBuild: {}", e))?;

    // Get stdout and stderr
    let stdout = child.stdout.take().ok_or("Failed to capture stdout")?;
    let stderr = child.stderr.take().ok_or("Failed to capture stderr")?;

    let mut stdout_reader = BufReader::new(stdout).lines();
    let mut stderr_reader = BufReader::new(stderr).lines();

    // Read output lines concurrently
    loop {
        tokio::select! {
            line = stdout_reader.next_line() => {
                match line {
                    Ok(Some(text)) => {
                        let _ = output.send(WineBuildEvent::Output(text)).await;
                    }
                    Ok(None) => break, // stdout closed
                    Err(e) => {
                        let _ = output.send(WineBuildEvent::Output(format!("[read error: {}]", e))).await;
                        break;
                    }
                }
            }
            line = stderr_reader.next_line() => {
                match line {
                    Ok(Some(text)) => {
                        let _ = output.send(WineBuildEvent::Output(text)).await;
                    }
                    Ok(None) => {} // stderr closed, continue reading stdout
                    Err(e) => {
                        let _ = output.send(WineBuildEvent::Output(format!("[read error: {}]", e))).await;
                    }
                }
            }
        }
    }

    // Drain any remaining stderr
    while let Ok(Some(text)) = stderr_reader.next_line().await {
        let _ = output.send(WineBuildEvent::Output(text)).await;
    }

    // Wait for process to complete
    let status = child
        .wait()
        .await
        .map_err(|e| format!("Failed to wait for process: {}", e))?;

    Ok(status.success())
}

/// Run MSBuild via Wine (non-streaming version for compatibility)
pub async fn run_wine_build(request: WineBuildRequest) -> Result<WineBuildResult, String> {
    use std::process::Stdio;
    use tokio::process::Command;

    let WineBuildRequest {
        target_path,
        prefix_path,
        msbuild_path,
        configuration,
        platform,
        action,
    } = request;

    // Convert Linux path to Wine path (Z: drive)
    let wine_target = format!("Z:{}", target_path.to_string_lossy().replace('/', "\\"));

    // Convert msbuild_path (which is relative to drive_c) to Windows path
    let msbuild_windows = if msbuild_path.starts_with(prefix_path.join("drive_c")) {
        let relative = msbuild_path
            .strip_prefix(prefix_path.join("drive_c"))
            .map_err(|_| "Invalid MSBuild path")?;
        format!("C:{}", relative.to_string_lossy().replace('/', "\\"))
    } else {
        format!("Z:{}", msbuild_path.to_string_lossy().replace('/', "\\"))
    };

    // Build action target
    let action_target = match action {
        BuildAction::Build => "Build",
        BuildAction::Rebuild => "Rebuild",
        BuildAction::Clean => "Clean",
    };

    // VS root for MSBuild to find tools
    let vs_root = r"C:\Program Files\Microsoft Visual Studio\2022\BuildTools";
    let sdk_version = "10.0.26100.0";

    // Check if we're on NixOS and need steam-run
    let is_nixos = std::path::Path::new("/etc/nixos").exists() || std::env::var("NIX_PATH").is_ok();
    let has_steam_run = which::which("steam-run").is_ok();

    // Ensure temp directory exists for MSVC compiler - use simple C:\Temp for Wine compatibility
    let temp_dir = prefix_path.join("drive_c/Temp");
    let _ = std::fs::create_dir_all(&temp_dir);

    // Use simple Windows temp path
    let windows_temp_path = r"C:\Temp";

    // Get essential environment variables we need to preserve (uppercase only to avoid .NET case collisions)
    let home = std::env::var("HOME").unwrap_or_default();
    let path = std::env::var("PATH").unwrap_or_default();
    let display = std::env::var("DISPLAY").unwrap_or_default();
    let xdg_runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_default();

    let output = if is_nixos && has_steam_run {
        Command::new("steam-run")
            .arg("wine")
            .arg(&msbuild_windows)
            .arg(&wine_target)
            .arg(format!("/p:Configuration={}", configuration))
            .arg(format!("/p:Platform={}", platform))
            .arg(format!("/p:VSInstallRoot={}", vs_root))
            .arg(format!("/p:WindowsTargetPlatformVersion={}", sdk_version))
            .arg(format!("/t:{}", action_target))
            .arg("/nologo")
            .arg("/verbosity:minimal")
            .env_clear() // Clear all inherited env vars to avoid case collisions in .NET
            .env("HOME", &home)
            .env("PATH", &path)
            .env("DISPLAY", &display)
            .env("XDG_RUNTIME_DIR", &xdg_runtime_dir)
            .env("WINEPREFIX", &prefix_path)
            .env("WINEDEBUG", "-all")
            .env("TMP", &windows_temp_path)
            .env("TEMP", &windows_temp_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| format!("Failed to start MSBuild: {}", e))?
    } else {
        Command::new("wine")
            .arg(&msbuild_windows)
            .arg(&wine_target)
            .arg(format!("/p:Configuration={}", configuration))
            .arg(format!("/p:Platform={}", platform))
            .arg(format!("/p:VSInstallRoot={}", vs_root))
            .arg(format!("/p:WindowsTargetPlatformVersion={}", sdk_version))
            .arg(format!("/t:{}", action_target))
            .arg("/nologo")
            .arg("/verbosity:minimal")
            .env_clear() // Clear all inherited env vars to avoid case collisions in .NET
            .env("HOME", &home)
            .env("PATH", &path)
            .env("DISPLAY", &display)
            .env("XDG_RUNTIME_DIR", &xdg_runtime_dir)
            .env("WINEPREFIX", &prefix_path)
            .env("WINEDEBUG", "-all")
            .env("TMP", &windows_temp_path)
            .env("TEMP", &windows_temp_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| format!("Failed to start MSBuild: {}", e))?
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined_output = format!("{}\n{}", stdout, stderr);

    Ok(WineBuildResult {
        success: output.status.success(),
        output: combined_output,
        target: target_path.display().to_string(),
    })
}
