use iced::widget::text_editor::Action as TextEditorAction;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use vedit_core::{Document, FileNode};

#[derive(Debug, Clone)]
pub enum Message {
    OpenFileRequested,
    FileLoaded(Result<Option<Document>, String>),
    DocumentSelected(usize),
    WorkspaceOpenRequested,
    WorkspaceLoaded(Result<Option<(String, Vec<FileNode>)>, String>),
    WorkspaceFileActivated(String),
    BufferAction(TextEditorAction),
}

#[derive(Clone)]
pub struct WorkspaceSnapshot {
    pub version: u64,
    pub tree: Arc<Vec<FileNode>>,
}

impl WorkspaceSnapshot {
    pub fn new(version: u64, tree: Arc<Vec<FileNode>>) -> Self {
        Self { version, tree }
    }
}

impl fmt::Debug for WorkspaceSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WorkspaceSnapshot")
            .field("version", &self.version)
            .field("tree_entries", &self.tree.len())
            .finish()
    }
}

impl Hash for WorkspaceSnapshot {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.version.hash(state);
        (Arc::as_ptr(&self.tree) as usize).hash(state);
    }
}
