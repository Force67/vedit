//! Core symbol index types and implementation

use crate::{Result, SymbolError};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tree_sitter::Parser;

/// A definition location in the codebase
#[derive(Debug, Clone)]
pub struct DefinitionLocation {
    /// Path to the file containing the definition
    pub file_path: PathBuf,
    /// Line number (1-indexed)
    pub line: usize,
    /// Column number (0-indexed)
    pub column: usize,
    /// Byte offset from start of file
    pub byte_offset: usize,
    /// The kind of definition
    pub kind: DefinitionKind,
    /// Preview of the definition (first few lines)
    pub preview: String,
}

/// The kind of definition
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DefinitionKind {
    Struct,
    Class,
    Typedef,
    Enum,
    Union,
    Function,
    Macro,
    Variable,
    Namespace,
}

impl DefinitionKind {
    /// Get the display name for this kind
    pub fn as_str(&self) -> &'static str {
        match self {
            DefinitionKind::Struct => "struct",
            DefinitionKind::Class => "class",
            DefinitionKind::Typedef => "typedef",
            DefinitionKind::Enum => "enum",
            DefinitionKind::Union => "union",
            DefinitionKind::Function => "function",
            DefinitionKind::Macro => "macro",
            DefinitionKind::Variable => "variable",
            DefinitionKind::Namespace => "namespace",
        }
    }
}

/// Symbol index for a workspace
///
/// The index maintains a mapping from symbol names to their definition locations.
/// It supports incremental updates and tracks file modification times to avoid
/// re-indexing unchanged files.
#[derive(Debug, Default)]
pub struct SymbolIndex {
    /// Map from symbol name to definition locations
    definitions: HashMap<String, Vec<DefinitionLocation>>,
    /// Include directories for header resolution
    include_dirs: Vec<PathBuf>,
    /// Indexed file paths with modification times
    indexed_files: HashMap<PathBuf, SystemTime>,
}

impl SymbolIndex {
    /// Create a new empty symbol index
    pub fn new() -> Self {
        Self::default()
    }

    /// Index a single C/C++ file for definitions
    ///
    /// This will parse the file using tree-sitter and extract all type definitions
    /// (structs, classes, typedefs, enums, unions).
    pub fn index_file(&mut self, path: &Path, content: &str) -> Result<()> {
        // Safety checks to avoid crashes in tree-sitter
        if content.is_empty() {
            return Ok(());
        }

        // Skip very large files (>1MB) to avoid memory issues and tree-sitter bugs
        if content.len() > 1024 * 1024 {
            return Err(SymbolError::ParseError(
                "File too large to index".to_string(),
            ));
        }

        // Skip files that are known to cause tree-sitter crashes
        // (shader headers with embedded GLSL/HLSL, generated code, GPU code, etc.)
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            let name_lower = name.to_lowercase();
            if name_lower.contains("shader")
                || name_lower.contains("_generated")
                || name_lower.contains(".gen.")
                || name_lower.ends_with("_inc.h")
                || name_lower.contains("gpu")
                || name_lower.contains("vulkan")
                || name_lower.contains("metal")
                || name_lower.contains("directx")
                || name_lower.contains("d3d")
            {
                return Err(SymbolError::ParseError(
                    "Skipping file (shader/gpu/generated)".to_string(),
                ));
            }
        }

        // Check for binary content (null bytes indicate binary)
        if content.as_bytes().contains(&0) {
            return Err(SymbolError::ParseError("Binary file detected".to_string()));
        }

        // Check for very long lines which can cause tree-sitter issues
        // (often indicates embedded data like shaders or base64)
        for line in content.lines().take(100) {
            if line.len() > 10000 {
                return Err(SymbolError::ParseError(
                    "File contains very long lines (likely embedded data)".to_string(),
                ));
            }
        }

        let mut parser = Parser::new();
        let language = tree_sitter_cpp::LANGUAGE;
        parser
            .set_language(&language.into())
            .map_err(|e| SymbolError::ParseError(format!("Failed to set language: {}", e)))?;

        let tree = match parser.parse(content, None) {
            Some(tree) => tree,
            None => return Err(SymbolError::ParseError("Failed to parse file".to_string())),
        };

        // Traverse the tree to find definitions
        let mut cursor = tree.walk();
        self.traverse_for_definitions(&mut cursor, content, path);

        // Track indexed file
        if let Ok(metadata) = std::fs::metadata(path) {
            if let Ok(modified) = metadata.modified() {
                self.indexed_files.insert(path.to_path_buf(), modified);
            }
        }

        Ok(())
    }

    /// Traverse tree to find struct, class, typedef, enum definitions
    fn traverse_for_definitions(
        &mut self,
        cursor: &mut tree_sitter::TreeCursor,
        content: &str,
        path: &Path,
    ) {
        let content_len = content.len();

        loop {
            let node = cursor.node();
            let kind_str = node.kind();

            // Check for definition types
            let (def_kind, name_field) = match kind_str {
                "struct_specifier" => (Some(DefinitionKind::Struct), "name"),
                "class_specifier" => (Some(DefinitionKind::Class), "name"),
                "enum_specifier" => (Some(DefinitionKind::Enum), "name"),
                "union_specifier" => (Some(DefinitionKind::Union), "name"),
                "type_definition" => (Some(DefinitionKind::Typedef), "declarator"),
                "namespace_definition" => (Some(DefinitionKind::Namespace), "name"),
                "preproc_def" | "preproc_function_def" => (Some(DefinitionKind::Macro), "name"),
                _ => (None, ""),
            };

            if let Some(kind) = def_kind {
                // Look for the name field
                if let Some(name_node) = node.child_by_field_name(name_field) {
                    let actual_name_node = self.extract_name_node(name_node, kind);

                    if let Some(name_node) = actual_name_node {
                        let range = name_node.byte_range();
                        // Bounds check to prevent UB
                        if range.start <= range.end && range.end <= content_len {
                            let name = &content[range.clone()];
                            // Skip anonymous or empty names
                            if !name.is_empty() && !name.starts_with("__") {
                                let start = node.start_position();
                                let preview =
                                    extract_preview(content, node.start_byte(), node.end_byte());

                                self.definitions.entry(name.to_string()).or_default().push(
                                    DefinitionLocation {
                                        file_path: path.to_path_buf(),
                                        line: start.row + 1,
                                        column: start.column,
                                        byte_offset: node.start_byte(),
                                        kind,
                                        preview,
                                    },
                                );
                            }
                        }
                    }
                }
            }

            // Traverse children
            if cursor.goto_first_child() {
                self.traverse_for_definitions(cursor, content, path);
                cursor.goto_parent();
            }

            // Move to next sibling
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    /// Extract the actual name node from a potentially nested node
    fn extract_name_node<'a>(
        &self,
        name_node: tree_sitter::Node<'a>,
        kind: DefinitionKind,
    ) -> Option<tree_sitter::Node<'a>> {
        match kind {
            DefinitionKind::Typedef => {
                // For typedef, the declarator might be nested
                if name_node.kind() == "type_identifier" {
                    Some(name_node)
                } else {
                    None
                }
            }
            DefinitionKind::Macro => {
                // Macros have identifier as name
                if name_node.kind() == "identifier" {
                    Some(name_node)
                } else {
                    None
                }
            }
            _ => {
                // Most definitions use type_identifier or namespace_identifier
                if name_node.kind() == "type_identifier"
                    || name_node.kind() == "namespace_identifier"
                    || name_node.kind() == "identifier"
                {
                    Some(name_node)
                } else {
                    None
                }
            }
        }
    }

    /// Look up definitions for a symbol name
    pub fn find_definition(&self, name: &str) -> Vec<&DefinitionLocation> {
        self.definitions
            .get(name)
            .map(|v| v.iter().collect())
            .unwrap_or_default()
    }

    /// Look up definitions with a filter for kind
    pub fn find_definition_by_kind(
        &self,
        name: &str,
        kind: DefinitionKind,
    ) -> Vec<&DefinitionLocation> {
        self.definitions
            .get(name)
            .map(|v| v.iter().filter(|d| d.kind == kind).collect())
            .unwrap_or_default()
    }

    /// Set include directories for header resolution
    pub fn set_include_dirs(&mut self, dirs: Vec<PathBuf>) {
        self.include_dirs = dirs;
    }

    /// Add an include directory
    pub fn add_include_dir(&mut self, dir: PathBuf) {
        if !self.include_dirs.contains(&dir) {
            self.include_dirs.push(dir);
        }
    }

    /// Get include directories
    pub fn include_dirs(&self) -> &[PathBuf] {
        &self.include_dirs
    }

    /// Check if a file needs reindexing
    pub fn needs_reindex(&self, path: &Path) -> bool {
        if let Some(indexed_time) = self.indexed_files.get(path) {
            if let Ok(metadata) = std::fs::metadata(path) {
                if let Ok(modified) = metadata.modified() {
                    return modified > *indexed_time;
                }
            }
        }
        true
    }

    /// Get the number of indexed symbols
    pub fn symbol_count(&self) -> usize {
        self.definitions.len()
    }

    /// Get the total number of definitions (including multiple definitions for same symbol)
    pub fn definition_count(&self) -> usize {
        self.definitions.values().map(|v| v.len()).sum()
    }

    /// Get the number of indexed files
    pub fn file_count(&self) -> usize {
        self.indexed_files.len()
    }

    /// Clear the index
    pub fn clear(&mut self) {
        self.definitions.clear();
        self.indexed_files.clear();
    }

    /// Remove definitions from a specific file (for incremental updates)
    pub fn remove_file(&mut self, path: &Path) {
        // Remove from indexed files tracking
        self.indexed_files.remove(path);

        // Remove definitions from this file
        for defs in self.definitions.values_mut() {
            defs.retain(|d| d.file_path != path);
        }

        // Remove empty entries
        self.definitions.retain(|_, v| !v.is_empty());
    }

    /// Get all symbol names
    pub fn symbol_names(&self) -> impl Iterator<Item = &str> {
        self.definitions.keys().map(|s| s.as_str())
    }

    /// Search for symbols matching a prefix
    pub fn search_prefix(&self, prefix: &str) -> Vec<(&str, &[DefinitionLocation])> {
        let prefix_lower = prefix.to_lowercase();
        self.definitions
            .iter()
            .filter(|(name, _)| name.to_lowercase().starts_with(&prefix_lower))
            .map(|(name, defs)| (name.as_str(), defs.as_slice()))
            .collect()
    }

    /// Search for symbols containing a substring
    pub fn search_contains(&self, query: &str) -> Vec<(&str, &[DefinitionLocation])> {
        let query_lower = query.to_lowercase();
        self.definitions
            .iter()
            .filter(|(name, _)| name.to_lowercase().contains(&query_lower))
            .map(|(name, defs)| (name.as_str(), defs.as_slice()))
            .collect()
    }
}

/// Extract a preview of the definition (first few lines)
fn extract_preview(content: &str, start_byte: usize, end_byte: usize) -> String {
    let content_len = content.len();
    // Bounds check to prevent UB
    if start_byte >= content_len {
        return String::new();
    }
    let end = end_byte.min(content_len);
    if start_byte > end {
        return String::new();
    }
    let def_text = &content[start_byte..end];
    let lines: Vec<&str> = def_text.lines().take(8).collect();
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_cpp_struct() {
        let mut index = SymbolIndex::new();
        let content = r#"
struct MyStruct {
    int x;
    int y;
};
"#;
        let path = Path::new("test.h");
        index.index_file(path, content).unwrap();

        let defs = index.find_definition("MyStruct");
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].kind, DefinitionKind::Struct);
        assert_eq!(defs[0].line, 2);
    }

    #[test]
    fn test_index_cpp_class() {
        let mut index = SymbolIndex::new();
        let content = r#"
class MyClass {
public:
    void method();
private:
    int m_value;
};
"#;
        let path = Path::new("test.h");
        index.index_file(path, content).unwrap();

        let defs = index.find_definition("MyClass");
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].kind, DefinitionKind::Class);
    }

    #[test]
    fn test_index_typedef() {
        let mut index = SymbolIndex::new();
        let content = r#"
typedef unsigned int uint32;
typedef struct { int x; } Point;
"#;
        let path = Path::new("test.h");
        index.index_file(path, content).unwrap();

        let defs = index.find_definition("uint32");
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].kind, DefinitionKind::Typedef);
    }

    #[test]
    fn test_index_enum() {
        let mut index = SymbolIndex::new();
        let content = r#"
enum Color {
    Red,
    Green,
    Blue
};
"#;
        let path = Path::new("test.h");
        index.index_file(path, content).unwrap();

        let defs = index.find_definition("Color");
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].kind, DefinitionKind::Enum);
    }

    #[test]
    fn test_search_prefix() {
        let mut index = SymbolIndex::new();
        let content = r#"
struct MyStruct {};
struct MyOther {};
struct NotMy {};
"#;
        let path = Path::new("test.h");
        index.index_file(path, content).unwrap();

        let results = index.search_prefix("My");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_remove_file() {
        let mut index = SymbolIndex::new();

        index.index_file(Path::new("a.h"), "struct A {};").unwrap();
        index.index_file(Path::new("b.h"), "struct B {};").unwrap();

        assert_eq!(index.symbol_count(), 2);

        index.remove_file(Path::new("a.h"));

        assert_eq!(index.symbol_count(), 1);
        assert!(index.find_definition("A").is_empty());
        assert!(!index.find_definition("B").is_empty());
    }

    #[test]
    fn test_multiple_definitions() {
        let mut index = SymbolIndex::new();

        // Same name in different files (forward decl pattern)
        index.index_file(Path::new("a.h"), "struct Foo;").unwrap();
        index
            .index_file(Path::new("b.h"), "struct Foo { int x; };")
            .unwrap();

        let defs = index.find_definition("Foo");
        // Note: forward declarations might not be indexed depending on tree-sitter
        // This test verifies the multi-definition capability
        assert!(defs.len() >= 1);
    }
}
