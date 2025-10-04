pub mod document;
pub mod editor;
pub mod workspace;

/// Returns the startup banner presented when launching the editor.
pub fn startup_banner() -> String {
    "Welcome to vedit".to_string()
}

pub use document::Document;
pub use editor::Editor;
pub use workspace::FileNode;
