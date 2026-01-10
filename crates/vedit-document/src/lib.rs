//! Document handling for vedit
//!
//! This crate provides core document functionality including:
//! - File-backed and in-memory documents
//! - Memory-mapped file support for large files
//! - Line indexing and navigation
//! - Viewport management for rendering
//! - Background content indexing

pub mod document;
pub mod indexing;
pub mod line_index;
pub mod mapped;
pub mod search;
pub mod viewport;

// Re-export main types for convenience
pub use document::Document;
pub use line_index::LineIndex;
pub use mapped::MappedDocument;
pub use search::{BoyerMooreSearcher, contains_pattern, find_pattern, search_pattern};
pub use viewport::Viewport;
