use crate::document::Document;
use crate::text_buffer::TextBuffer;
use crate::workspace::{self, FileNode};
use std::io;
use std::sync::Arc;
use vedit_config::WorkspaceConfig;

/// High-level editor session managing open documents and workspace state.
#[derive(Debug)]
pub struct Editor {
    open_documents: Vec<Document>,
    active_index: usize,
    workspace_root: Option<String>,
    workspace_tree: Arc<Vec<FileNode>>,
    workspace_generation: u64,
    workspace_config: Option<WorkspaceConfig>,
}

impl Default for Editor {
    fn default() -> Self {
        Self {
            open_documents: vec![Document::default()],
            active_index: 0,
            workspace_root: None,
            workspace_tree: Arc::new(Vec::new()),
            workspace_generation: 0,
            workspace_config: None,
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
            let current = doc.buffer.to_string();
            if current == contents {
                return;
            }

            apply_text_diff(&mut doc.buffer, &current, &contents);
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

    pub fn workspace_config(&self) -> Option<&WorkspaceConfig> {
        self.workspace_config.as_ref()
    }

    pub fn workspace_config_mut(&mut self) -> Option<&mut WorkspaceConfig> {
        self.workspace_config.as_mut()
    }

    pub fn set_workspace(&mut self, root: String, tree: Vec<FileNode>, config: WorkspaceConfig) {
        self.workspace_root = Some(root);
        self.workspace_tree = Arc::new(tree);
        self.workspace_config = Some(config);
        self.workspace_generation = self.workspace_generation.wrapping_add(1);
    }

    pub fn clear_workspace(&mut self) {
        self.workspace_root = None;
        self.workspace_tree = Arc::new(Vec::new());
        self.workspace_config = None;
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
    pub fn build_workspace_tree(
        root: impl AsRef<std::path::Path>,
        config: Option<&WorkspaceConfig>,
    ) -> io::Result<Vec<FileNode>> {
        if let Some(config) = config {
            let ignored: Vec<String> = config
                .ignored_directories()
                .map(|entry| entry.to_string())
                .collect();
            workspace::build_tree_with_ignored(root, &ignored)
        } else {
            workspace::build_tree(root)
        }
    }

    pub fn build_solution_tree(path: impl AsRef<std::path::Path>) -> io::Result<Vec<FileNode>> {
        workspace::build_solution_tree(path)
            .map_err(|err| io::Error::new(io::ErrorKind::Other, err.to_string()))
    }

    pub fn load_workspace_directory(&mut self, path: &str) -> io::Result<Vec<String>> {
        if self.workspace_root.is_none() {
            return Ok(Vec::new());
        }

        let ignored: Vec<String> = self
            .workspace_config
            .as_ref()
            .map(|config| {
                config
                    .ignored_directories()
                    .map(|entry| entry.to_ascii_lowercase())
                    .collect()
            })
            .unwrap_or_default();

        let tree = Arc::make_mut(&mut self.workspace_tree);
        if let Some(node) = workspace::find_node_mut(tree.as_mut_slice(), path) {
            if workspace::load_directory_children(node, &ignored)? {
                self.workspace_generation = self.workspace_generation.wrapping_add(1);
                let directories = node
                    .children
                    .iter()
                    .filter(|child| child.is_directory)
                    .map(|child| child.path.clone())
                    .collect();
                return Ok(directories);
            }
        }

        Ok(Vec::new())
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

    pub fn workspace_name(&self) -> Option<&str> {
        self.workspace_config
            .as_ref()
            .and_then(|config| config.name.as_deref())
    }
}

fn apply_text_diff(buffer: &mut TextBuffer, old_text: &str, new_text: &str) {
    if old_text == new_text {
        return;
    }

    let old_bytes = old_text.as_bytes();
    let new_bytes = new_text.as_bytes();

    let mut prefix = 0usize;
    let max_prefix = old_bytes.len().min(new_bytes.len());
    while prefix < max_prefix && old_bytes[prefix] == new_bytes[prefix] {
        prefix += 1;
    }

    while prefix > 0
        && (!old_text.is_char_boundary(prefix) || !new_text.is_char_boundary(prefix))
    {
        prefix -= 1;
    }

    let mut suffix = 0usize;
    let max_suffix = old_bytes.len().min(new_bytes.len()).saturating_sub(prefix);
    while suffix < max_suffix
        && old_bytes[old_bytes.len() - 1 - suffix]
            == new_bytes[new_bytes.len() - 1 - suffix]
    {
        suffix += 1;
    }

    while suffix > 0 {
        let old_index = old_bytes.len() - suffix;
        let new_index = new_bytes.len() - suffix;
        if old_index < prefix || new_index < prefix {
            suffix = 0;
            break;
        }
        if old_text.is_char_boundary(old_index) && new_text.is_char_boundary(new_index) {
            break;
        }
        suffix -= 1;
    }

    let delete_start = prefix;
    let delete_end = old_bytes.len().saturating_sub(suffix);
    if delete_end > delete_start {
        buffer.delete(delete_start..delete_end);
    }

    let insert_start = prefix;
    let insert_end = new_bytes.len().saturating_sub(suffix);
    if insert_end > insert_start {
        let inserted = &new_text[insert_start..insert_end];
        buffer.insert(insert_start, inserted);
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn apply_text_diff_handles_inserts_and_deletes() {
        let original = "hello";
        let mut buffer = TextBuffer::from_text(original);
        let expanded = "hello world";
        super::apply_text_diff(&mut buffer, original, expanded);
        assert_eq!(buffer.to_string(), expanded);

        let shortened = "hello";
        super::apply_text_diff(&mut buffer, expanded, shortened);
        assert_eq!(buffer.to_string(), shortened);
    }

    #[test]
    fn apply_text_diff_preserves_unicode_boundaries() {
        let original = "café";
        let mut buffer = TextBuffer::from_text(original);
        let extended = "cafés";
        super::apply_text_diff(&mut buffer, original, extended);
        assert_eq!(buffer.to_string(), extended);

        let emoji_old = "🙂🙂";
        let emoji_new = "🙂";
        let mut emoji_buffer = TextBuffer::from_text(emoji_old);
        super::apply_text_diff(&mut emoji_buffer, emoji_old, emoji_new);
        assert_eq!(emoji_buffer.to_string(), emoji_new);
    }

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
