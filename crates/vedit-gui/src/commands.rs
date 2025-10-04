use rfd::FileDialog;
use std::fs;
use std::path::PathBuf;
use vedit_config::{WorkspaceConfig, WorkspaceMetadata};
use vedit_core::{Document, Editor, FileNode};

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

pub async fn pick_solution() -> Result<Option<WorkspaceData>, String> {
    if let Some(path) = FileDialog::new().pick_file() {
        let root_string = path.to_string_lossy().to_string();
        let tree = Editor::build_solution_tree(&path)
            .map_err(|err| format!("Failed to load solution: {}", err))?;

        let config_root = path
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let mut config = WorkspaceConfig::load_or_default(&config_root)
            .map_err(|err| format!("Failed to load workspace config: {}", err))?;
        let metadata = WorkspaceMetadata::load_or_default(&config_root)
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
