pub mod editor;

/// Returns the startup banner presented when launching the editor.
pub fn startup_banner() -> String {
    "Welcome to vedit".to_string()
}

pub use editor::{Document, Editor, FileNode};
