//! Symbol indexing for C/C++ codebases
//!
//! This crate provides symbol indexing and lookup capabilities for go-to-definition
//! functionality. It supports multiple project types through the `ProjectIndexer` trait:
//!
//! - Visual Studio solutions (`.sln`, `.vcxproj`) via the `vs` feature
//! - Makefiles via the `make` feature
//! - CMake (planned)
//!
//! # Example
//!
//! ```no_run
//! use vedit_symbols::{SymbolIndex, DefinitionKind};
//! use std::path::Path;
//!
//! let mut index = SymbolIndex::new();
//!
//! // Index a C++ header file
//! let content = r#"
//! struct MyStruct {
//!     int x;
//!     int y;
//! };
//! "#;
//! index.index_file(Path::new("test.h"), content).unwrap();
//!
//! // Look up the definition
//! let defs = index.find_definition("MyStruct");
//! assert_eq!(defs.len(), 1);
//! assert_eq!(defs[0].kind, DefinitionKind::Struct);
//! ```

mod hover;
mod index;
mod indexers;

pub use hover::{HoverSymbol, HoverSymbolKind, line_column_to_byte_offset, symbol_at_offset};
pub use index::{DefinitionKind, DefinitionLocation, SymbolIndex};
pub use indexers::ProjectIndexer;

#[cfg(feature = "vs")]
pub use indexers::VsSolutionIndexer;

#[cfg(feature = "make")]
pub use indexers::MakefileIndexer;

/// Error type for symbol indexing operations
#[derive(Debug, thiserror::Error)]
pub enum SymbolError {
    #[error("Failed to parse file: {0}")]
    ParseError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Project error: {0}")]
    ProjectError(String),
}

pub type Result<T> = std::result::Result<T, SymbolError>;
