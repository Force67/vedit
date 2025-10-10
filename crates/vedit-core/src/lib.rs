pub mod document;
pub mod editor;
pub mod sticky;

/// Returns the startup banner presented when launching the editor.
pub fn startup_banner() -> String {
    "Welcome to vedit".to_string()
}

pub use document::{Document, MappedDocument, Viewport, LineIndex};
pub use editor::Editor;
pub use sticky::StickyNote;

// Re-export from new focused crates
pub use vedit_text::TextBuffer;
pub use vedit_workspace::{
    DirEntryMeta, FileMeta, FileNode, FilterState, FsWorkspaceProvider, GitStatus, LegacyNodeKind, Node, NodeId,
    NodeKind, WorkspaceProvider, WorkspaceTree,
};
pub use vedit_syntax::Language;
pub use vedit_keybinds::{KeyCombination, KeyEvent, Keymap, KeymapError, Key, QUICK_COMMAND_MENU_ACTION, SAVE_ACTION};
pub use vedit_config::{WorkspaceConfig, DebugTargetRecord};
