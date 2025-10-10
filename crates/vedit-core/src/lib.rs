pub mod editor;

/// Returns the startup banner presented when launching the editor.
pub fn startup_banner() -> String {
    "Welcome to vedit".to_string()
}

pub use editor::Editor;
pub use vedit_config::StickyNote;

// Re-export from new focused crates
pub use vedit_text::TextBuffer;
pub use vedit_workspace::{
    DirEntryMeta, FileMeta, FileNode, FilterState, FsWorkspaceProvider, GitStatus, LegacyNodeKind, Node, NodeId,
    NodeKind, WorkspaceProvider, WorkspaceTree,
};
pub use vedit_syntax::Language;
pub use vedit_keybinds::{KeyCombination, KeyEvent, Keymap, KeymapError, Key, QUICK_COMMAND_MENU_ACTION, SAVE_ACTION};
pub use vedit_config::{WorkspaceConfig, DebugTargetRecord};

// Re-export document types from vedit-document
pub use vedit_document::{Document, MappedDocument, Viewport, LineIndex};
