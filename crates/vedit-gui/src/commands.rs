use rfd::FileDialog;
use std::fs;
use std::path::PathBuf;
use vedit_config::{WorkspaceConfig, WorkspaceMetadata};
use vedit_core::{Document, Editor, FileNode};
use vedit_debugger_gdb::{Breakpoint as DebuggerBreakpoint, GdbSession, LaunchConfig as DebuggerLaunchConfig};
use crate::debugger::DebuggerType;

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
    pub tree: Vec<FileNode>,
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

    Ok(dialog.save_file().map(|path| path.to_string_lossy().to_string()))
}

pub async fn pick_document() -> Result<Option<Document>, String> {
    if let Some(path) = FileDialog::new().pick_file() {
        let document = Document::from_path(&path)
            .map_err(|err| format!("Failed to read file: {}", err))?;
        Ok(Some(document))
    } else {
        Ok(None)
    }
}

pub async fn load_document_from_path(path: String) -> Result<Document, String> {
    Document::from_path(&path).map_err(|err| format!("Failed to read file: {}", err))
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
        let tree = Editor::build_workspace_tree(&path, Some(&config))
            .map_err(|err| format!("Failed to read folder: {}", err))?;
        Ok(Some(WorkspaceData {
            root: root_string,
            tree,
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
        let tree = Editor::build_workspace_tree(&path, Some(&config))
            .map_err(|err| format!("Failed to read folder: {}", err))?;
        Ok(Some(WorkspaceData {
            root: root_string,
            tree,
            config,
            metadata,
        }))
    } else {
        Ok(None)
    }
}

pub async fn load_workspace_from_path_with_files(
    path: PathBuf,
    _session_state: crate::session::SessionState
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
        let tree = Editor::build_workspace_tree(&path, Some(&config))
            .map_err(|err| format!("Failed to read folder: {}", err))?;

        Ok(Some(WorkspaceData {
            root: root_string,
            tree,
            config,
            metadata,
        }))
    } else {
        Ok(None)
    }
}


pub async fn pick_solution() -> Result<Option<WorkspaceData>, String> {
    if let Some(path) = FileDialog::new().pick_file() {
        let tree = Editor::build_solution_tree(&path)
            .map_err(|err| format!("Failed to load solution: {}", err))?;

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
            tree,
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
        fs::write(&target, contents)
            .map_err(|err| format!("Failed to write file: {}", err))?;
        return Ok(Some(target.to_string_lossy().to_string()));
    }

    let mut dialog = FileDialog::new();
    if let Some(name) = suggested_name.as_deref() {
        if !name.trim().is_empty() && name != "(scratch)" {
            dialog = dialog.set_file_name(name);
        }
    }

    if let Some(target) = dialog.save_file() {
        fs::write(&target, contents)
            .map_err(|err| format!("Failed to write file: {}", err))?;
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

    fs::write(&target, contents)
        .map_err(|err| format!("Failed to write keymap: {}", err))?;

    Ok(target.to_string_lossy().to_string())
}

pub async fn save_workspace_config(root: String, config: WorkspaceConfig) -> Result<String, String> {
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
    let tree = Editor::build_solution_tree(&path_buf)
        .map_err(|err| format!("Failed to load solution: {}", err))?;

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
        tree,
        config,
        metadata,
    }))
}
