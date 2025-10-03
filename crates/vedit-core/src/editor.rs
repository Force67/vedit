use std::cmp::Ordering;
use std::fs;
use std::io;
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

    pub fn from_path(path: impl AsRef<Path>) -> io::Result<Self> {
        let path_buf = path.as_ref().to_path_buf();
        let contents = fs::read_to_string(&path_buf)?;
        Ok(Self::new(
            Some(path_buf.to_string_lossy().to_string()),
            contents,
        ))
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

/// Node of a workspace file tree.
#[derive(Debug, Clone)]
pub struct FileNode {
    pub name: String,
    pub path: String,
    pub is_directory: bool,
    pub children: Vec<FileNode>,
}

impl FileNode {
    fn from_path(path: &Path) -> io::Result<Self> {
        let metadata = fs::metadata(path)?;
        let is_directory = metadata.is_dir();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.to_string())
            .unwrap_or_else(|| path.to_string_lossy().to_string());
        let path_string = path.to_string_lossy().to_string();

        let children = if is_directory {
            Editor::collect_directory(path)?
        } else {
            Vec::new()
        };

        Ok(Self {
            name,
            path: path_string,
            is_directory,
            children,
        })
    }
}

/// High-level editor session managing open documents and workspace state.
#[derive(Debug)]
pub struct Editor {
    open_documents: Vec<Document>,
    active_index: usize,
    workspace_root: Option<String>,
    workspace_tree: Vec<FileNode>,
}

impl Default for Editor {
    fn default() -> Self {
        Self {
            open_documents: vec![Document::default()],
            active_index: 0,
            workspace_root: None,
            workspace_tree: Vec::new(),
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

    pub fn workspace_root(&self) -> Option<&str> {
        self.workspace_root.as_deref()
    }

    pub fn workspace_tree(&self) -> Option<&[FileNode]> {
        if self.workspace_root.is_some() {
            Some(&self.workspace_tree)
        } else {
            None
        }
    }

    pub fn set_workspace(&mut self, root: String, tree: Vec<FileNode>) {
        self.workspace_root = Some(root);
        self.workspace_tree = tree;
    }

    pub fn clear_workspace(&mut self) {
        self.workspace_root = None;
        self.workspace_tree.clear();
    }

    /// Build a workspace tree for the provided directory.
    pub fn build_workspace_tree(root: impl AsRef<Path>) -> io::Result<Vec<FileNode>> {
        Self::collect_directory(root.as_ref())
    }

    fn collect_directory(path: &Path) -> io::Result<Vec<FileNode>> {
        let mut children = Vec::new();

        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let entry_path = entry.path();

            // Skip directories we cannot access gracefully.
            match FileNode::from_path(&entry_path) {
                Ok(node) => children.push(node),
                Err(err) => {
                    if err.kind() == io::ErrorKind::PermissionDenied {
                        continue;
                    } else {
                        return Err(err);
                    }
                }
            }
        }

        children.sort_by(|a, b| match (a.is_directory, b.is_directory) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        });

        Ok(children)
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
