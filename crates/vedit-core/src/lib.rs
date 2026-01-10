pub mod editor;

/// Returns the startup banner presented when launching the editor.
pub fn startup_banner() -> String {
    "Welcome to vedit".to_string()
}

pub use editor::Editor;
pub use vedit_config::StickyNote;

// Re-export from new focused crates
pub use vedit_config::{DebugTargetRecord, WorkspaceConfig};
pub use vedit_keybinds::{
    Key, KeyCombination, KeyEvent, Keymap, KeymapError, QUICK_COMMAND_MENU_ACTION, SAVE_ACTION,
};
pub use vedit_syntax::Language;
pub use vedit_text::TextBuffer;
pub use vedit_workspace::{
    DirEntryMeta, FileMeta, FileNode, FilterState, FsWorkspaceProvider, GitStatus, LegacyNodeKind,
    Node, NodeId, NodeKind, WorkspaceProvider, WorkspaceTree,
};

// Re-export document types from vedit-document
pub use vedit_document::{Document, LineIndex, MappedDocument, Viewport};
