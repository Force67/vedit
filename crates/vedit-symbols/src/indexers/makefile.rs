//! Makefile-based project indexer
//!
//! This module provides indexing support for Makefile-based projects.
//! It extracts source files from Makefiles and indexes them.

use crate::indexers::ProjectIndexer;
use crate::{Result, SymbolError, SymbolIndex};
use std::path::{Path, PathBuf};
use vedit_make::Makefile;

/// Indexer for Makefile-based projects
///
/// This indexer extracts source files referenced in Makefiles and indexes
/// header files found in the project.
pub struct MakefileIndexer {
    /// Path to the Makefile
    makefile_path: PathBuf,
    /// Parsed Makefile
    makefile: Makefile,
    /// Header files to index (extracted from Makefile + discovered)
    header_files: Vec<PathBuf>,
    /// Project root directory
    root_dir: PathBuf,
}

impl MakefileIndexer {
    /// Create a new indexer from a Makefile path
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        let makefile_path = path.as_ref().to_path_buf();
        let makefile = Makefile::from_path(&makefile_path)
            .map_err(|e| SymbolError::ProjectError(format!("Failed to parse Makefile: {}", e)))?;

        let root_dir = makefile_path
            .parent()
            .unwrap_or(Path::new("."))
            .to_path_buf();

        let mut indexer = Self {
            makefile_path,
            makefile,
            header_files: Vec::new(),
            root_dir,
        };

        indexer.collect_header_files();
        Ok(indexer)
    }

    /// Collect header files from Makefile references and by scanning the directory
    fn collect_header_files(&mut self) {
        // Get files from the Makefile
        for item in &self.makefile.files {
            if is_header_file(&item.full_path) && item.full_path.exists() {
                if !self.header_files.contains(&item.full_path) {
                    self.header_files.push(item.full_path.clone());
                }
            }
        }

        // Also scan for header files in the project directory
        self.scan_for_headers(&self.root_dir.clone());
    }

    /// Recursively scan directory for header files
    fn scan_for_headers(&mut self, dir: &Path) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();

                // Skip common non-source directories
                if path.is_dir() {
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if !matches!(
                        name,
                        "." | ".." | ".git" | "build" | "obj" | "bin" | "node_modules" | "target"
                    ) {
                        self.scan_for_headers(&path);
                    }
                } else if is_header_file(&path) && !self.header_files.contains(&path) {
                    self.header_files.push(path);
                }
            }
        }
    }

    /// Get the Makefile name
    pub fn makefile_name(&self) -> &str {
        &self.makefile.name
    }

    /// Get source files (non-headers) from the Makefile
    pub fn source_files_from_makefile(&self) -> Vec<&PathBuf> {
        self.makefile
            .files
            .iter()
            .filter(|item| is_source_file(&item.full_path))
            .map(|item| &item.full_path)
            .collect()
    }
}

impl ProjectIndexer for MakefileIndexer {
    fn index(&self, index: &mut SymbolIndex) -> Result<usize> {
        let mut indexed_count = 0;

        // Add root directory as include path
        index.add_include_dir(self.root_dir.clone());

        // Index all header files
        for header_path in &self.header_files {
            if index.needs_reindex(header_path) {
                match std::fs::read_to_string(header_path) {
                    Ok(content) => {
                        if let Err(e) = index.index_file(header_path, &content) {
                            eprintln!("Warning: Failed to index {}: {}", header_path.display(), e);
                        } else {
                            indexed_count += 1;
                        }
                    }
                    Err(e) => {
                        eprintln!("Warning: Failed to read {}: {}", header_path.display(), e);
                    }
                }
            }
        }

        Ok(indexed_count)
    }

    fn include_dirs(&self) -> Vec<PathBuf> {
        vec![self.root_dir.clone()]
    }

    fn source_files(&self) -> Vec<PathBuf> {
        self.header_files.clone()
    }

    fn name(&self) -> &str {
        &self.makefile.name
    }

    fn root_dir(&self) -> &Path {
        &self.root_dir
    }
}

/// Check if a path is a C/C++ header file
fn is_header_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| {
            matches!(
                ext.to_lowercase().as_str(),
                "h" | "hpp" | "hxx" | "h++" | "hh" | "inl"
            )
        })
        .unwrap_or(false)
}

/// Check if a path is a C/C++ source file
fn is_source_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| {
            matches!(
                ext.to_lowercase().as_str(),
                "c" | "cpp" | "cxx" | "c++" | "cc"
            )
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_makefile(dir: &Path) -> PathBuf {
        let makefile_content = r#"
CC = gcc
CFLAGS = -Wall

SRCS = main.c utils.c
HEADERS = utils.h types.h

all: program

program: $(SRCS)
	$(CC) $(CFLAGS) -o $@ $^
"#;
        let makefile_path = dir.join("Makefile");
        std::fs::write(&makefile_path, makefile_content).unwrap();

        // Create header files
        std::fs::write(
            dir.join("utils.h"),
            r#"
struct Utils {
    int id;
};
void init_utils(void);
"#,
        )
        .unwrap();

        std::fs::write(
            dir.join("types.h"),
            r#"
typedef int MyInt;
struct Point {
    int x, y;
};
"#,
        )
        .unwrap();

        // Create source files
        std::fs::write(
            dir.join("main.c"),
            r#"
#include "utils.h"
int main() { return 0; }
"#,
        )
        .unwrap();

        std::fs::write(
            dir.join("utils.c"),
            r#"
#include "utils.h"
void init_utils(void) {}
"#,
        )
        .unwrap();

        makefile_path
    }

    #[test]
    fn test_makefile_indexer_creation() {
        let temp_dir = TempDir::new().unwrap();
        let makefile_path = create_test_makefile(temp_dir.path());

        let indexer = MakefileIndexer::from_path(&makefile_path);
        assert!(indexer.is_ok());
    }

    #[test]
    fn test_makefile_indexer_finds_headers() {
        let temp_dir = TempDir::new().unwrap();
        let makefile_path = create_test_makefile(temp_dir.path());

        let indexer = MakefileIndexer::from_path(&makefile_path).unwrap();
        let headers = indexer.source_files();

        // Should find at least the two header files we created
        assert!(headers.len() >= 2);
    }

    #[test]
    fn test_makefile_indexer_index() {
        let temp_dir = TempDir::new().unwrap();
        let makefile_path = create_test_makefile(temp_dir.path());

        let indexer = MakefileIndexer::from_path(&makefile_path).unwrap();
        let mut index = SymbolIndex::new();

        let count = indexer.index(&mut index).unwrap();
        assert!(count >= 2);

        // Check if structs were indexed
        assert!(!index.find_definition("Utils").is_empty());
        assert!(!index.find_definition("Point").is_empty());
    }

    #[test]
    fn test_is_header_file() {
        assert!(is_header_file(Path::new("foo.h")));
        assert!(is_header_file(Path::new("foo.hpp")));
        assert!(is_header_file(Path::new("foo.hxx")));
        assert!(!is_header_file(Path::new("foo.c")));
        assert!(!is_header_file(Path::new("foo.cpp")));
    }

    #[test]
    fn test_is_source_file() {
        assert!(is_source_file(Path::new("foo.c")));
        assert!(is_source_file(Path::new("foo.cpp")));
        assert!(is_source_file(Path::new("foo.cxx")));
        assert!(!is_source_file(Path::new("foo.h")));
        assert!(!is_source_file(Path::new("foo.hpp")));
    }
}
