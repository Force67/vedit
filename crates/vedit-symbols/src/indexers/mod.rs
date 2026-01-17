//! Project indexers for different build systems
//!
//! This module provides the `ProjectIndexer` trait and implementations for
//! various project types (Visual Studio, Makefile, CMake, etc.).

#[cfg(feature = "vs")]
mod vs;

#[cfg(feature = "make")]
mod makefile;

use crate::{Result, SymbolIndex};
use std::path::Path;

/// Trait for indexing symbols from different project types
///
/// Implementations of this trait know how to extract source files and include
/// directories from their respective project formats and populate a `SymbolIndex`.
pub trait ProjectIndexer {
    /// Index the project and populate the symbol index
    ///
    /// # Arguments
    /// * `index` - The symbol index to populate
    ///
    /// # Returns
    /// The number of files indexed
    fn index(&self, index: &mut SymbolIndex) -> Result<usize>;

    /// Get include directories from the project
    fn include_dirs(&self) -> Vec<std::path::PathBuf>;

    /// Get source files to index (typically headers)
    fn source_files(&self) -> Vec<std::path::PathBuf>;

    /// Get the project name for display
    fn name(&self) -> &str;

    /// Get the project root directory
    fn root_dir(&self) -> &Path;
}

#[cfg(feature = "vs")]
pub use vs::VsSolutionIndexer;

#[cfg(feature = "make")]
pub use makefile::MakefileIndexer;
