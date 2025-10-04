use crate::document::Document;
use crate::sticky::StickyNote;
use crate::text_buffer::TextBuffer;
use crate::workspace::{self, FileNode};
use std::io;
use std::sync::Arc;
use vedit_config::{WorkspaceConfig, WorkspaceMetadata};

/// High-level editor session managing open documents and workspace state.
#[derive(Debug)]
pub struct Editor {
    open_documents: Vec<Document>,
    active_index: usize,
    workspace_root: Option<String>,
    workspace_tree: Arc<Vec<FileNode>>,
    workspace_generation: u64,
    workspace_config: Option<WorkspaceConfig>,
    workspace_metadata: Option<WorkspaceMetadata>,
    workspace_metadata_dirty: bool,
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
            workspace_metadata: None,
            workspace_metadata_dirty: false,
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
                self.apply_metadata_to_document(index);
                return index;
            }
        }

        self.open_documents.push(document);
        self.active_index = self.open_documents.len() - 1;
        self.apply_metadata_to_document(self.active_index);
        self.active_index
    }

    pub fn update_active_buffer(&mut self, contents: String) {
        if self.open_documents.is_empty() {
            return;
        }

        let current_index = self.active_index;
        if let Some(doc) = self.open_documents.get_mut(current_index) {
            let current = doc.buffer.to_string();
            if current == contents {
                return;
            }

            if let Some(change) = TextChange::between(&current, &contents) {
                change.apply(&mut doc.buffer);
                doc.is_modified = true;

                if doc.has_sticky_notes() {
                    doc.apply_sticky_offset_delta(
                        change.deletion_range(),
                        change.insertion_range(),
                        &contents,
                    );
                    self.sync_metadata_for_document(current_index);
                }
            }
        }
    }

    pub fn add_sticky_note(
        &mut self,
        line: usize,
        column: usize,
        content: String,
    ) -> Option<u64> {
        let id = self.workspace_metadata.as_ref()?.next_sticky_id();
        let (path, records) = {
            let doc = self.active_document_mut()?;
            let path = doc.path.clone()?;
            let snapshot = doc.buffer.to_string();
            let offset = Document::offset_for_line_column(&snapshot, line, column);
            let (resolved_line, resolved_column) =
                Document::line_column_for_offset(&snapshot, offset);
            let note = StickyNote::new(id, resolved_line, resolved_column, content, offset);
            doc.insert_sticky_note(note);
            let records = doc.to_sticky_records(&path);
            (path, records)
        };

        if let Some(metadata) = self.workspace_metadata.as_mut() {
            if metadata.set_notes_for_file(&path, records) {
                self.workspace_metadata_dirty = true;
            }
        }

        Some(id)
    }

    pub fn update_sticky_note_content(&mut self, id: u64, content: String) -> bool {
        if self.open_documents.is_empty() {
            return false;
        }

        let index = self.active_index;
        let Some(doc) = self.open_documents.get_mut(index) else {
            return false;
        };

        let path = match doc.path.clone() {
            Some(path) => path,
            None => return false,
        };

        let Some(note) = doc.find_sticky_note_mut(id) else {
            return false;
        };

        if note.content == content {
            return false;
        }

        note.content = content;

        if let Some(metadata) = self.workspace_metadata.as_mut() {
            let records = doc.to_sticky_records(&path);
            if metadata.set_notes_for_file(&path, records) {
                self.workspace_metadata_dirty = true;
            }
        }

        true
    }

    pub fn remove_sticky_note(&mut self, id: u64) -> bool {
        if self.open_documents.is_empty() {
            return false;
        }

        let index = self.active_index;
        let Some(doc) = self.open_documents.get_mut(index) else {
            return false;
        };

        let path = match doc.path.clone() {
            Some(path) => path,
            None => return false,
        };

        if doc.remove_sticky_note(id).is_none() {
            return false;
        }

        if let Some(metadata) = self.workspace_metadata.as_mut() {
            let records = doc.to_sticky_records(&path);
            if metadata.set_notes_for_file(&path, records) {
                self.workspace_metadata_dirty = true;
            }
        }

        true
    }

    pub fn clear_active_modified(&mut self) {
        if let Some(doc) = self.active_document_mut() {
            doc.mark_clean();
        }
    }

    pub fn mark_active_document_saved(&mut self, path: Option<String>) {
        if self.open_documents.is_empty() {
            return;
        }

        let index = self.active_index;
        if let Some(doc) = self.open_documents.get_mut(index) {
            if let Some(path) = path {
                let previous = doc.path.clone();
                doc.set_path(path.clone());
                if let Some(metadata) = self.workspace_metadata.as_mut() {
                    if let Some(old_path) = previous {
                        if metadata.remove_file(&old_path) {
                            self.workspace_metadata_dirty = true;
                        }
                    }
                    let records = doc.to_sticky_records(&path);
                    if metadata.set_notes_for_file(&path, records) {
                        self.workspace_metadata_dirty = true;
                    }
                }
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

    pub fn workspace_metadata(&self) -> Option<&WorkspaceMetadata> {
        self.workspace_metadata.as_ref()
    }

    pub fn workspace_metadata_mut(&mut self) -> Option<&mut WorkspaceMetadata> {
        self.workspace_metadata.as_mut()
    }

    pub fn active_sticky_notes(&self) -> Option<&[StickyNote]> {
        self.active_document().map(|doc| doc.sticky_notes())
    }

    pub fn set_workspace(
        &mut self,
        root: String,
        tree: Vec<FileNode>,
        config: WorkspaceConfig,
        metadata: WorkspaceMetadata,
    ) {
        self.workspace_root = Some(root);
        self.workspace_tree = Arc::new(tree);
        self.workspace_config = Some(config);
        self.workspace_generation = self.workspace_generation.wrapping_add(1);
        self.workspace_metadata = Some(metadata);
        self.workspace_metadata_dirty = false;
        self.apply_metadata_to_documents();
    }

    pub fn clear_workspace(&mut self) {
        self.workspace_root = None;
        self.workspace_tree = Arc::new(Vec::new());
        self.workspace_config = None;
        self.workspace_metadata = None;
        self.workspace_metadata_dirty = false;
        for doc in &mut self.open_documents {
            doc.clear_sticky_notes();
        }
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

    pub fn take_workspace_metadata_payload(&mut self) -> Option<(String, WorkspaceMetadata)> {
        if !self.workspace_metadata_dirty {
            return None;
        }

        let root = self.workspace_root.clone()?;
        let metadata = self.workspace_metadata.clone()?;
        self.workspace_metadata_dirty = false;
        Some((root, metadata))
    }

    fn apply_metadata_to_document(&mut self, index: usize) {
        let Some(doc) = self.open_documents.get_mut(index) else {
            return;
        };

        let Some(metadata) = self.workspace_metadata.as_ref() else {
            doc.clear_sticky_notes();
            return;
        };

        let Some(path) = doc.path.clone() else {
            doc.clear_sticky_notes();
            return;
        };

        let records = metadata.notes_for_file(&path);
        let contents = doc.buffer.to_string();
        doc.set_sticky_notes_from_records(&records, &contents);
    }

    fn apply_metadata_to_documents(&mut self) {
        for index in 0..self.open_documents.len() {
            self.apply_metadata_to_document(index);
        }
    }

    fn sync_metadata_for_document(&mut self, index: usize) {
        let Some(doc) = self.open_documents.get(index) else {
            return;
        };

        let Some(path) = doc.path.as_deref() else {
            return;
        };

        let Some(metadata) = self.workspace_metadata.as_mut() else {
            return;
        };

        if metadata.set_notes_for_file(path, doc.to_sticky_records(path)) {
            self.workspace_metadata_dirty = true;
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

#[derive(Debug, Clone)]
struct TextChange {
    delete: Option<Deletion>,
    insert: Option<Insertion>,
}

#[derive(Debug, Clone)]
struct Deletion {
    start: usize,
    len: usize,
}

#[derive(Debug, Clone)]
struct Insertion {
    start: usize,
    text: String,
}

impl TextChange {
    fn between(old_text: &str, new_text: &str) -> Option<Self> {
        if old_text == new_text {
            return None;
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
        let delete_len = delete_end.saturating_sub(delete_start);

        let insert_start = prefix;
        let insert_end = new_bytes.len().saturating_sub(suffix);
        let insert_len = insert_end.saturating_sub(insert_start);

        let delete = if delete_len > 0 {
            Some(Deletion {
                start: delete_start,
                len: delete_len,
            })
        } else {
            None
        };

        let insert = if insert_len > 0 {
            Some(Insertion {
                start: insert_start,
                text: new_text[insert_start..insert_end].to_string(),
            })
        } else {
            None
        };

        if delete.is_none() && insert.is_none() {
            None
        } else {
            Some(Self { delete, insert })
        }
    }

    fn apply(&self, buffer: &mut TextBuffer) {
        if let Some(delete) = &self.delete {
            buffer.delete(delete.start..delete.start + delete.len);
        }

        if let Some(insert) = &self.insert {
            buffer.insert(insert.start, &insert.text);
        }
    }

    fn deletion_range(&self) -> Option<(usize, usize)> {
        self.delete.as_ref().map(|deletion| (deletion.start, deletion.len))
    }

    fn insertion_range(&self) -> Option<(usize, usize)> {
        self.insert
            .as_ref()
            .map(|insert| (insert.start, insert.text.len()))
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn text_change_handles_inserts_and_deletes() {
        let original = "hello";
        let mut buffer = TextBuffer::from_text(original);
        let expanded = "hello world";
        let change = super::TextChange::between(original, expanded).unwrap();
        change.apply(&mut buffer);
        assert_eq!(buffer.to_string(), expanded);

        let shortened = "hello";
        let shrink = super::TextChange::between(expanded, shortened).unwrap();
        shrink.apply(&mut buffer);
        assert_eq!(buffer.to_string(), shortened);
    }

    #[test]
    fn text_change_preserves_unicode_boundaries() {
        let original = "café";
        let mut buffer = TextBuffer::from_text(original);
        let extended = "cafés";
        let insertion = super::TextChange::between(original, extended).unwrap();
        insertion.apply(&mut buffer);
        assert_eq!(buffer.to_string(), extended);

        let emoji_old = "🙂🙂";
        let emoji_new = "🙂";
        let mut emoji_buffer = TextBuffer::from_text(emoji_old);
        let removal = super::TextChange::between(emoji_old, emoji_new).unwrap();
        removal.apply(&mut emoji_buffer);
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
