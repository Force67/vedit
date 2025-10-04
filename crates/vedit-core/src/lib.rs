pub mod document;
pub mod editor;
pub mod keybinds;
pub mod workspace;
pub mod language;

/// Returns the startup banner presented when launching the editor.
pub fn startup_banner() -> String {
    "Welcome to vedit".to_string()
}

pub use document::Document;
pub use editor::Editor;
pub use keybinds::{KeyCombination, KeyEvent, Keymap, KeymapError, Key, QUICK_COMMAND_MENU_ACTION, SAVE_ACTION};
pub use workspace::{FileNode, NodeKind};
pub use language::Language;
pub use vedit_config::WorkspaceConfig;
