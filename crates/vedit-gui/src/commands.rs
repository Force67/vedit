use rfd::FileDialog;
use vedit_core::{Document, Editor, FileNode};

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
