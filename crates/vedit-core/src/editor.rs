use crate::document::Document;
use crate::workspace::{self, FileNode};
use std::io;
use std::sync::Arc;

/// High-level editor session managing open documents and workspace state.
#[derive(Debug)]
pub struct Editor {
    open_documents: Vec<Document>,
    active_index: usize,
    workspace_root: Option<String>,
    workspace_tree: Arc<Vec<FileNode>>,
    workspace_generation: u64,
}

impl Default for Editor {
    fn default() -> Self {
        Self {
            open_documents: vec![Document::default()],
            active_index: 0,
            workspace_root: None,
            workspace_tree: Arc::new(Vec::new()),
            workspace_generation: 0,
        }
    }
}

impl Editor {
    /// Create a fresh editor instance with a single scratch buffer.
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open_documents(&self) -> &[Document] {
        &self.open_documents
    }

    pub fn active_document(&self) -> Option<&Document> {
        self.open_documents.get(self.active_index)
    }

    pub fn active_document_mut(&mut self) -> Option<&mut Document> {
        self.open_documents.get_mut(self.active_index)
    }

    pub fn active_index(&self) -> usize {
        self.active_index
    }

    pub fn set_active(&mut self, index: usize) {
        if index < self.open_documents.len() {
            self.active_index = index;
        }
    }

    pub fn document_count(&self) -> usize {
        self.open_documents.len()
    }

    pub fn open_document(&mut self, document: Document) -> usize {
        if let Some(fingerprint) = document.fingerprint {
            if let Some(index) = self
                .open_documents
                .iter()
                .position(|doc| doc.fingerprint == Some(fingerprint))
            {
                self.open_documents[index] = document;
                self.active_index = index;
                return index;
            }
        }

        self.open_documents.push(document);
        self.active_index = self.open_documents.len() - 1;
        self.active_index
    }

    pub fn update_active_buffer(&mut self, contents: String) {
        if let Some(doc) = self.active_document_mut() {
            doc.buffer = contents;
            doc.is_modified = true;
        }
    }

    pub fn clear_active_modified(&mut self) {
        if let Some(doc) = self.active_document_mut() {
            doc.mark_clean();
        }
    }

    pub fn mark_active_document_saved(&mut self, path: Option<String>) {
        if let Some(doc) = self.active_document_mut() {
            if let Some(path) = path {
                doc.set_path(path);
            }
            doc.mark_clean();
        }
    }

    pub fn workspace_root(&self) -> Option<&str> {
        self.workspace_root.as_deref()
    }

    pub fn workspace_tree(&self) -> Option<&[FileNode]> {
        if self.workspace_root.is_some() {
            Some(self.workspace_tree.as_slice())
        } else {
            None
        }
    }

    pub fn set_workspace(&mut self, root: String, tree: Vec<FileNode>) {
        self.workspace_root = Some(root);
        self.workspace_tree = Arc::new(tree);
        self.workspace_generation = self.workspace_generation.wrapping_add(1);
    }

    pub fn clear_workspace(&mut self) {
        self.workspace_root = None;
        self.workspace_tree = Arc::new(Vec::new());
        self.workspace_generation = self.workspace_generation.wrapping_add(1);
    }

    pub fn workspace_snapshot(&self) -> Option<(u64, Arc<Vec<FileNode>>)> {
        if self.workspace_root.is_some() {
            Some((
                self.workspace_generation,
                Arc::clone(&self.workspace_tree),
            ))
        } else {
            None
        }
    }

    /// Build a workspace tree for the provided directory.
    pub fn build_workspace_tree(root: impl AsRef<std::path::Path>) -> io::Result<Vec<FileNode>> {
        workspace::build_tree(root)
    }

    /// Returns a human-friendly status line reflecting the current editor state.
    pub fn status_line(&self) -> String {
        if let Some(doc) = self.active_document() {
            let name = if let Some(path) = &doc.path {
                path.as_str()
            } else {
                "(scratch)"
            };
            let dirty = if doc.is_modified { "*" } else { "" };
            format!("{}{}", name, dirty)
        } else {
            "No document".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn reopening_same_path_reuses_existing_document() {
        let mut editor = Editor::new();
        let unique = format!(
            "vedit_core_test_{}_{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );

        let temp_dir = std::env::temp_dir().join(unique);
        fs::create_dir_all(&temp_dir).unwrap();

        let file_path = temp_dir.join("sample.txt");
        fs::write(&file_path, "hello world").unwrap();

        let doc_one = Document::from_path(&file_path).unwrap();
        let base_count = editor.document_count();
        let first_index = editor.open_document(doc_one);

        assert_eq!(editor.document_count(), base_count + 1);

        let doc_two = Document::from_path(&file_path).unwrap();
        let count_before = editor.document_count();
        let second_index = editor.open_document(doc_two);

        assert_eq!(second_index, first_index);
        assert_eq!(editor.document_count(), count_before);
        assert!(editor.active_document().is_some());

        let _ = fs::remove_file(&file_path);
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
