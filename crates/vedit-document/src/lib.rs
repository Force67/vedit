//! Document handling for vedit
//!
//! This crate provides core document functionality including:
//! - File-backed and in-memory documents
//! - Memory-mapped file support for large files
//! - Line indexing and navigation
//! - Viewport management for rendering
//! - Background content indexing

pub mod document;
pub mod mapped;
pub mod viewport;
pub mod line_index;
pub mod indexing;
pub mod search;

// Re-export main types for convenience
pub use document::Document;
pub use mapped::MappedDocument;
pub use viewport::Viewport;
pub use line_index::LineIndex;
pub use search::{BoyerMooreSearcher, search_pattern, find_pattern, contains_pattern};