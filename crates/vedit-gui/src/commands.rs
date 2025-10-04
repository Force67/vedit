use rfd::FileDialog;
use std::fs;
use std::path::PathBuf;
use vedit_core::{Document, Editor, FileNode};

#[derive(Debug, Clone)]
pub struct SaveDocumentRequest {
    pub path: Option<String>,
    pub contents: String,
    pub suggested_name: Option<String>,
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

pub async fn pick_workspace() -> Result<Option<(String, Vec<FileNode>)>, String> {
    if let Some(path) = FileDialog::new().pick_folder() {
        let tree = Editor::build_workspace_tree(&path)
            .map_err(|err| format!("Failed to read folder: {}", err))?;
        Ok(Some((path.to_string_lossy().to_string(), tree)))
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
