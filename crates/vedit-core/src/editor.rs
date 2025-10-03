use std::path::Path;

/// Represents an open file in the editor workspace.
#[derive(Debug, Clone)]
pub struct Document {
    pub path: Option<String>,
    pub buffer: String,
    pub is_modified: bool,
}

impl Document {
    pub fn new(path: Option<String>, buffer: String) -> Self {
        Self {
            path,
            buffer,
            is_modified: false,
        }
    }

    pub fn display_name(&self) -> &str {
        if let Some(path) = &self.path {
            Path::new(path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(path)
        } else {
            "(scratch)"
        }
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new(None, String::new())
    }
}

/// High-level editor session managing open documents.
#[derive(Debug)]
pub struct Editor {
    open_documents: Vec<Document>,
    active_index: usize,
}

impl Default for Editor {
    fn default() -> Self {
        Self {
            open_documents: vec![Document::default()],
            active_index: 0,
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
            doc.is_modified = false;
        }
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
