//! Hover detection for C++ symbols
//!
//! This module provides functionality to identify the symbol under the cursor
//! in C++ source code using tree-sitter.

use tree_sitter::{Parser, Point, TreeCursor};

/// Information about the symbol under cursor
#[derive(Debug, Clone)]
pub struct HoverSymbol {
    /// The symbol name (e.g., "vector" from "std::vector")
    pub name: String,
    /// The full text of the symbol (e.g., "std::vector")
    pub full_text: String,
    /// Byte range of the symbol in the source
    pub byte_range: std::ops::Range<usize>,
    /// The kind of symbol found
    pub kind: HoverSymbolKind,
}

/// The kind of symbol found at hover position
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoverSymbolKind {
    /// A type identifier (e.g., "MyClass")
    TypeIdentifier,
    /// A qualified type (e.g., "std::vector")
    QualifiedType,
    /// A namespace (e.g., "std")
    Namespace,
    /// A generic identifier
    Identifier,
    /// A field/member identifier
    FieldIdentifier,
}

impl HoverSymbolKind {
    /// Get the display name for this kind
    pub fn as_str(&self) -> &'static str {
        match self {
            HoverSymbolKind::TypeIdentifier => "type",
            HoverSymbolKind::QualifiedType => "qualified type",
            HoverSymbolKind::Namespace => "namespace",
            HoverSymbolKind::Identifier => "identifier",
            HoverSymbolKind::FieldIdentifier => "field",
        }
    }
}

/// Safely extract a string slice from content using a byte range
fn safe_slice(content: &str, range: std::ops::Range<usize>) -> Option<&str> {
    if range.start <= range.end && range.end <= content.len() {
        Some(&content[range])
    } else {
        None
    }
}

/// Find the symbol at a given byte offset in C++ code
///
/// This function parses the code with tree-sitter and finds the most specific
/// symbol at the given position. It handles type identifiers, qualified names,
/// and namespaces.
///
/// # Arguments
/// * `content` - The source code content
/// * `byte_offset` - The byte offset to look up
///
/// # Returns
/// `Some(HoverSymbol)` if a symbol is found, `None` otherwise
pub fn symbol_at_offset(content: &str, byte_offset: usize) -> Option<HoverSymbol> {
    // Safety checks
    if content.is_empty() || byte_offset >= content.len() {
        return None;
    }

    // Skip binary content
    if content.as_bytes().contains(&0) {
        return None;
    }

    let mut parser = Parser::new();
    let language = tree_sitter_cpp::LANGUAGE;
    parser.set_language(&language.into()).ok()?;

    let tree = parser.parse(content, None)?;
    let point = offset_to_point(content, byte_offset);

    // Find the deepest node at this position
    let mut cursor = tree.walk();
    descend_to_point(&mut cursor, point);

    // Walk up to find a meaningful symbol
    loop {
        let node = cursor.node();
        let kind = node.kind();

        match kind {
            "type_identifier" => {
                let name = safe_slice(content, node.byte_range())?.to_string();
                return Some(HoverSymbol {
                    name: name.clone(),
                    full_text: name,
                    byte_range: node.byte_range(),
                    kind: HoverSymbolKind::TypeIdentifier,
                });
            }
            "namespace_identifier" => {
                let name = safe_slice(content, node.byte_range())?.to_string();
                return Some(HoverSymbol {
                    name: name.clone(),
                    full_text: name,
                    byte_range: node.byte_range(),
                    kind: HoverSymbolKind::Namespace,
                });
            }
            "qualified_identifier" => {
                // Extract the full qualified name and the rightmost identifier
                let full_text = safe_slice(content, node.byte_range())?.to_string();
                let name = if let Some(name_node) = node.child_by_field_name("name") {
                    safe_slice(content, name_node.byte_range())
                        .unwrap_or(&full_text)
                        .to_string()
                } else {
                    // Fallback: use the last part after ::
                    full_text
                        .rsplit("::")
                        .next()
                        .unwrap_or(&full_text)
                        .to_string()
                };
                return Some(HoverSymbol {
                    name,
                    full_text,
                    byte_range: node.byte_range(),
                    kind: HoverSymbolKind::QualifiedType,
                });
            }
            "template_type" => {
                // For template types like vector<int>, get the template name
                if let Some(name_node) = node.child_by_field_name("name") {
                    if let Some(name) = safe_slice(content, name_node.byte_range()) {
                        let full_text = safe_slice(content, node.byte_range())
                            .unwrap_or(name)
                            .to_string();
                        return Some(HoverSymbol {
                            name: name.to_string(),
                            full_text,
                            byte_range: node.byte_range(),
                            kind: HoverSymbolKind::TypeIdentifier,
                        });
                    }
                }
            }
            "identifier" => {
                // Check parent to determine context
                let parent_kind = cursor.node().parent().map(|p| p.kind()).unwrap_or("");

                // Check if this is in a type context
                if matches!(
                    parent_kind,
                    "declaration"
                        | "parameter_declaration"
                        | "field_declaration"
                        | "function_declarator"
                        | "call_expression"
                        | "type_descriptor"
                ) {
                    let name = safe_slice(content, node.byte_range())?.to_string();
                    return Some(HoverSymbol {
                        name: name.clone(),
                        full_text: name,
                        byte_range: node.byte_range(),
                        kind: HoverSymbolKind::Identifier,
                    });
                }
            }
            "field_identifier" => {
                let name = safe_slice(content, node.byte_range())?.to_string();
                return Some(HoverSymbol {
                    name: name.clone(),
                    full_text: name,
                    byte_range: node.byte_range(),
                    kind: HoverSymbolKind::FieldIdentifier,
                });
            }
            "primitive_type" => {
                // Don't return hover info for primitive types like int, float, etc.
                return None;
            }
            _ => {}
        }

        if !cursor.goto_parent() {
            break;
        }
    }

    None
}

/// Convert a byte offset to a tree-sitter Point (row, column)
fn offset_to_point(content: &str, byte_offset: usize) -> Point {
    let prefix = &content[..byte_offset.min(content.len())];
    let row = prefix.bytes().filter(|&b| b == b'\n').count();
    let last_newline = prefix.rfind('\n').map(|i| i + 1).unwrap_or(0);
    let column = byte_offset.saturating_sub(last_newline);
    Point::new(row, column)
}

/// Descend the tree cursor to the deepest node containing the given point
fn descend_to_point(cursor: &mut TreeCursor, point: Point) {
    loop {
        let mut found_child = false;

        if cursor.goto_first_child() {
            loop {
                let node = cursor.node();
                let start = node.start_position();
                let end = node.end_position();

                // Check if this node contains the point
                let contains = (start.row < point.row
                    || (start.row == point.row && start.column <= point.column))
                    && (end.row > point.row
                        || (end.row == point.row && end.column >= point.column));

                if contains {
                    found_child = true;
                    break;
                }

                if !cursor.goto_next_sibling() {
                    break;
                }
            }

            if !found_child {
                cursor.goto_parent();
            }
        }

        if !found_child {
            break;
        }
    }
}

/// Get the byte offset for a given line and column in content
///
/// # Arguments
/// * `content` - The source code content
/// * `line` - 1-indexed line number
/// * `column` - 0-indexed column number
///
/// # Returns
/// `Some(offset)` if the position is valid, `None` otherwise
pub fn line_column_to_byte_offset(content: &str, line: usize, column: usize) -> Option<usize> {
    if line == 0 {
        return None;
    }

    let mut current_line = 1;
    let mut line_start = 0;

    for (i, c) in content.char_indices() {
        if current_line == line {
            // Found the line, now count columns
            let line_content = &content[line_start..];
            let mut col = 0;
            for (j, _) in line_content.char_indices() {
                if col == column {
                    return Some(line_start + j);
                }
                col += 1;
            }
            // Column is at or past end of line
            return Some(line_start + line_content.len().min(column));
        }
        if c == '\n' {
            current_line += 1;
            line_start = i + 1;
        }
    }

    // If we're looking for a line past the end, return None
    if current_line < line {
        return None;
    }

    // Line found but maybe column is at or past end
    Some(line_start + column.min(content.len() - line_start))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_type_identifier() {
        let content = "MyStruct foo;";
        // Position at 'M' of MyStruct
        let result = symbol_at_offset(content, 0);
        assert!(result.is_some());
        let symbol = result.unwrap();
        assert_eq!(symbol.name, "MyStruct");
        assert_eq!(symbol.kind, HoverSymbolKind::TypeIdentifier);
    }

    #[test]
    fn test_find_qualified_type() {
        let content = "std::vector<int> v;";
        // Position in the middle of "vector"
        let result = symbol_at_offset(content, 6);
        assert!(result.is_some());
        let symbol = result.unwrap();
        assert!(symbol.full_text.contains("vector"));
    }

    #[test]
    fn test_primitive_type_returns_none() {
        let content = "int x;";
        // Position at 'i' of int
        let result = symbol_at_offset(content, 0);
        assert!(result.is_none());
    }

    #[test]
    fn test_line_column_to_offset_first_line() {
        let content = "line1\nline2\nline3";
        assert_eq!(line_column_to_byte_offset(content, 1, 0), Some(0));
        assert_eq!(line_column_to_byte_offset(content, 1, 2), Some(2));
    }

    #[test]
    fn test_line_column_to_offset_middle_line() {
        let content = "line1\nline2\nline3";
        assert_eq!(line_column_to_byte_offset(content, 2, 0), Some(6));
        assert_eq!(line_column_to_byte_offset(content, 2, 2), Some(8));
    }

    #[test]
    fn test_line_column_to_offset_last_line() {
        let content = "line1\nline2\nline3";
        assert_eq!(line_column_to_byte_offset(content, 3, 0), Some(12));
    }

    #[test]
    fn test_line_column_invalid_line() {
        let content = "line1\nline2";
        assert_eq!(line_column_to_byte_offset(content, 0, 0), None);
        assert_eq!(line_column_to_byte_offset(content, 5, 0), None);
    }

    #[test]
    fn test_offset_to_point() {
        let content = "line1\nline2\nline3";
        let point = offset_to_point(content, 6);
        assert_eq!(point.row, 1);
        assert_eq!(point.column, 0);
    }

    #[test]
    fn test_method_impl_class_name() {
        // Test: hovering over "foo" in "void foo::bar()"
        //       positions:      01234567890
        let content = "void foo::bar() { }";

        // Position 5 is 'f' in foo
        let result = symbol_at_offset(content, 5);
        println!("Result at offset 5: {:?}", result);
        assert!(result.is_some(), "Should find symbol at 'foo' position");
        let symbol = result.unwrap();
        // foo should be recognized - either as the name itself or part of qualified
        assert!(
            symbol.name == "foo" || symbol.full_text.contains("foo"),
            "Expected 'foo' but got name={}, full_text={}",
            symbol.name,
            symbol.full_text
        );
    }

    #[test]
    fn test_method_impl_all_positions() {
        let content = "void foo::bar() { }";
        println!("Content: {}", content);
        println!("Positions: 0123456789012345678");

        for offset in 0..content.len() {
            let result = symbol_at_offset(content, offset);
            if let Some(sym) = result {
                println!(
                    "Offset {:2}: '{}' -> name={:?}, kind={:?}",
                    offset,
                    content.chars().nth(offset).unwrap_or(' '),
                    sym.name,
                    sym.kind
                );
            }
        }
    }

    #[test]
    fn test_end_to_end_hover_lookup() {
        use crate::SymbolIndex;
        use std::path::Path;

        // 1. Create and populate symbol index with a class definition
        let mut index = SymbolIndex::new();
        let header = r#"
class foo {
public:
    void bar();
    int value;
};
"#;
        index.index_file(Path::new("foo.h"), header).unwrap();

        // Verify class is indexed
        let defs = index.find_definition("foo");
        println!("Definitions for 'foo': {:?}", defs.len());
        assert!(!defs.is_empty(), "Class 'foo' should be indexed");
        for def in &defs {
            println!(
                "  - kind={:?}, line={}, preview:\n{}",
                def.kind, def.line, def.preview
            );
        }

        // 2. Simulate hover in implementation file
        let impl_content = "void foo::bar() { }";
        let hover_result = symbol_at_offset(impl_content, 5); // 'f' in foo
        assert!(hover_result.is_some());
        let symbol = hover_result.unwrap();
        println!("Hover found: name={}, kind={:?}", symbol.name, symbol.kind);

        // 3. Look up the hovered symbol in the index
        let found_defs = index.find_definition(&symbol.name);
        println!(
            "Found {} definitions for '{}'",
            found_defs.len(),
            symbol.name
        );
        assert!(!found_defs.is_empty(), "Should find class 'foo' in index");
    }
}
