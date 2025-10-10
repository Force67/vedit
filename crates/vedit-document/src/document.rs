use std::cmp;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io;
use std::path::{Path, PathBuf};
use memmap2::MmapOptions;
use vedit_config::{StickyNote, StickyNoteRecord};
use vedit_text::TextBuffer;
use vedit_syntax::Language;
use crate::mapped::load_viewport_content;
use crate::mapped::count_lines_in_mmap;

/// Core document structure representing a file or buffer
#[derive(Debug, Clone)]
pub struct Document {
    /// File path if saved to disk
    pub path: Option<String>,
    /// Text content buffer
    pub buffer: TextBuffer,
    /// Whether the document has unsaved changes
    pub is_modified: bool,
    /// Fingerprint for file identification (computed from path)
    pub fingerprint: Option<u64>,
    /// Sticky notes attached to the document
    pub sticky_notes: Vec<StickyNote>,
}

impl Document {
    /// Create a new document with optional path and initial content
    pub fn new(path: Option<String>, content: impl Into<TextBuffer>) -> Self {
        let fingerprint = path.as_ref().map(|p| compute_fingerprint(p));
        Self {
            path,
            buffer: content.into(),
            is_modified: false,
            fingerprint,
            sticky_notes: Vec::new(),
        }
    }

    /// Create a new empty document
    pub fn empty() -> Self {
        Self::new(None, TextBuffer::new())
    }

    /// Get the document path if it has one
    pub fn path(&self) -> Option<&str> {
        self.path.as_deref()
    }

    /// Get the text buffer
    pub fn buffer(&self) -> &TextBuffer {
        &self.buffer
    }

    /// Get mutable reference to the text buffer
    pub fn buffer_mut(&mut self) -> &mut TextBuffer {
        self.is_modified = true;
        &mut self.buffer
    }

    /// Get the document content as a string
    pub fn content(&self) -> String {
        self.buffer.to_string()
    }

    /// Check if the document has unsaved changes
    pub fn is_modified(&self) -> bool {
        self.is_modified
    }

    /// Mark the document as unchanged relative to disk.
    pub fn mark_clean(&mut self) {
        self.is_modified = false;
    }

    /// Load a document from a file path
    pub fn from_path(path: impl AsRef<Path>) -> io::Result<Self> {
        let path_buf = path.as_ref().to_path_buf();
        let contents = fs::read_to_string(&path_buf)?;
        Ok(Self::new(
            Some(path_buf.to_string_lossy().to_string()),
            contents,
        ))
    }

    /// Open a document with automatic memory-mapping for large files
    pub fn from_path_smart(path: impl AsRef<Path>) -> io::Result<Self> {
        let path_buf = path.as_ref().to_path_buf();

        // Get file size to decide if we should use memory mapping
        let metadata = fs::metadata(&path_buf)?;
        let file_size = metadata.len();

        // Use memory mapping for files larger than 10MB
        if file_size > 10 * 1024 * 1024 {
            // Create a streaming document that loads content on demand
            let file = fs::File::open(&path_buf)?;
            let mmap = unsafe { MmapOptions::new().map(&file)? };

            // Load initial viewport content (first 1000 lines)
            let initial_content = load_viewport_content(&mmap, 0, 1000);

            Ok(Self::new(
                Some(path_buf.to_string_lossy().to_string()),
                initial_content,
            ))
        } else {
            // Use regular loading for smaller files
            Self::from_path(path_buf)
        }
    }

    /// Load content from a specific viewport of a memory-mapped file
    pub fn load_viewport(&self, start_line: usize, visible_lines: usize) -> Option<String> {
        // Check if this document is large enough to have a memory-mapped backing
        if let Some(path) = &self.path {
            if let Ok(file) = fs::File::open(path) {
                if let Ok(metadata) = file.metadata() {
                    if metadata.len() > 10 * 1024 * 1024 {
                        if let Ok(mmap) = unsafe { MmapOptions::new().map(&file) } {
                            return Some(load_viewport_content(&mmap, start_line, visible_lines));
                        }
                    }
                }
            }
        }
        None
    }

    /// Get total line count for the file
    pub fn total_lines(&self) -> Option<usize> {
        if let Some(path) = &self.path {
            if let Ok(file) = fs::File::open(path) {
                if let Ok(metadata) = file.metadata() {
                    if metadata.len() > 10 * 1024 * 1024 {
                        if let Ok(mmap) = unsafe { MmapOptions::new().map(&file) } {
                            return Some(count_lines_in_mmap(&mmap));
                        }
                    }
                }
            }
        }
        // For regular documents, count lines in the buffer
        Some(self.buffer.to_string().lines().count())
    }

    /// Update document content for a new viewport (for large files)
    pub fn update_viewport(&mut self, start_line: usize, visible_lines: usize) -> bool {
        if let Some(new_content) = self.load_viewport(start_line, visible_lines) {
            // Update the buffer with new content
            self.buffer = TextBuffer::from_text(&new_content);
            true
        } else {
            false
        }
    }

    /// Check if this document is using streaming mode (large file)
    pub fn is_streaming(&self) -> bool {
        if let Some(path) = &self.path {
            if let Ok(file) = fs::File::open(path) {
                if let Ok(metadata) = file.metadata() {
                    return metadata.len() > 10 * 1024 * 1024;
                }
            }
        }
        false
    }

    /// Update the document path and refresh its fingerprint.
    pub fn set_path(&mut self, path: String) {
        self.fingerprint = Some(compute_fingerprint(&path));
        self.path = Some(path);
    }

    /// Get the display name of the document
    pub fn display_name(&self) -> &str {
        if let Some(path) = &self.path {
            Path::new(path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(path)
        } else {
            "(scratch)"
        }
    }

    /// Get the detected language for this document
    pub fn language(&self) -> Language {
        self.path
            .as_deref()
            .map(detect_language_from_path)
            .unwrap_or(Language::PlainText)
    }

    // Sticky notes management
    pub fn sticky_notes(&self) -> &[StickyNote] {
        &self.sticky_notes
    }

    pub fn sticky_notes_mut(&mut self) -> &mut [StickyNote] {
        self.sticky_notes.as_mut_slice()
    }

    pub fn has_sticky_notes(&self) -> bool {
        !self.sticky_notes.is_empty()
    }

    pub fn clear_sticky_notes(&mut self) {
        self.sticky_notes.clear();
    }

    pub fn insert_sticky_note(&mut self, note: StickyNote) {
        self.sticky_notes.push(note);
    }

    pub fn find_sticky_note_mut(&mut self, id: u64) -> Option<&mut StickyNote> {
        self.sticky_notes.iter_mut().find(|note| note.id == id)
    }

    pub fn remove_sticky_note(&mut self, id: u64) -> Option<StickyNote> {
        if let Some(index) = self.sticky_notes.iter().position(|note| note.id == id) {
            Some(self.sticky_notes.remove(index))
        } else {
            None
        }
    }

    pub fn set_sticky_notes_from_records(
        &mut self,
        records: &[StickyNoteRecord],
        contents: &str,
    ) {
        self.sticky_notes.clear();
        for record in records {
            let offset = Self::offset_for_line_column(contents, record.line, record.column);
            let clamped = cmp::min(offset, contents.len());
            let (line, column) = Self::line_column_for_offset(contents, clamped);
            self.sticky_notes.push(StickyNote::new(
                record.id,
                line,
                column,
                record.content.clone(),
                clamped,
            ));
        }
    }

    pub fn to_sticky_records(&self, file: &str) -> Vec<StickyNoteRecord> {
        self.sticky_notes
            .iter()
            .map(|note| {
                StickyNoteRecord::new(
                    note.id,
                    file.to_string(),
                    note.line,
                    note.column,
                    note.content.clone(),
                )
            })
            .collect()
    }

    pub fn apply_sticky_offset_delta(
        &mut self,
        delete: Option<(usize, usize)>,
        insert: Option<(usize, usize)>,
        contents: &str,
    ) -> bool {
        if self.sticky_notes.is_empty() {
            return false;
        }

        let mut changed = false;

        for note in &mut self.sticky_notes {
            if let Some((start, len)) = delete {
                let end = start.saturating_add(len);
                if note.offset >= start && note.offset < end {
                    note.offset = start;
                    changed = true;
                } else if note.offset >= end {
                    note.offset = note.offset.saturating_sub(len);
                    changed = true;
                }
            }

            if let Some((start, len)) = insert {
                if len > 0 && note.offset >= start {
                    note.offset = note.offset.saturating_add(len);
                    changed = true;
                }
            }

            let clamped = cmp::min(note.offset, contents.len());
            let (line, column) = Self::line_column_for_offset(contents, clamped);
            note.update(line, column, clamped);
        }

        changed
    }

    // Utility functions
    pub fn offset_for_line_column(contents: &str, line: usize, column: usize) -> usize {
        Self::offset_for_line_column_internal(contents, line, column)
    }

    pub fn line_column_for_offset(contents: &str, offset: usize) -> (usize, usize) {
        Self::position_for_offset_internal(contents, offset)
    }

    // Internal implementations
    fn offset_for_line_column_internal(contents: &str, line: usize, column: usize) -> usize {
        if contents.is_empty() {
            return 0;
        }

        let mut current_line = 1usize;
        let mut offset = 0usize;
        let target_line = line.max(1);

        for segment in contents.split_inclusive('\n') {
            if current_line == target_line {
                let trimmed = if segment.ends_with('\n') {
                    &segment[..segment.len() - 1]
                } else {
                    segment
                };

                let mut char_column = 1usize;
                for (idx, _) in trimmed.char_indices() {
                    if char_column == column.max(1) {
                        return offset + idx;
                    }
                    char_column += 1;
                }

                return offset + trimmed.len();
            }

            offset += segment.len();
            current_line += 1;
        }

        contents.len()
    }

    fn position_for_offset_internal(contents: &str, offset: usize) -> (usize, usize) {
        let clamped = offset.min(contents.len());
        let mut line = 1usize;
        let mut column = 1usize;

        for (idx, ch) in contents.char_indices() {
            if idx >= clamped {
                break;
            }

            if ch == '\n' {
                line += 1;
                column = 1;
            } else {
                column += 1;
            }
        }

        (line, column)
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new(None, TextBuffer::new())
    }
}

fn compute_fingerprint(path: &str) -> u64 {
    let resolved = canonicalize_lossy(path);
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    resolved.hash(&mut hasher);
    hasher.finish()
}

fn canonicalize_lossy(path: &str) -> String {
    let path_buf = PathBuf::from(path);
    std::fs::canonicalize(&path_buf)
        .unwrap_or(path_buf)
        .to_string_lossy()
        .to_string()
}

fn detect_language_from_path(path: &str) -> Language {
    let path = Path::new(path);

    if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
        let lower = name.to_ascii_lowercase();
        match lower.as_str() {
            "makefile" => return Language::Makefile,
            "dockerfile" => return Language::Dockerfile,
            "cmakelists.txt" => return Language::CMake,
            _ => {}
        }
    }

    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
    {
        Some(ext) => match ext.as_str() {
            "rs" => Language::Rust,
            "c" => Language::C,
            "h" => Language::CHeader,
            "hh" | "hpp" | "hxx" | "h++" => Language::CppHeader,
            "cpp" | "cc" | "cxx" | "c++" => Language::Cpp,
            "m" => Language::ObjectiveC,
            "mm" => Language::ObjectiveCpp,
            "swift" => Language::Swift,
            "java" => Language::Java,
            "kt" | "kts" => Language::Kotlin,
            "cs" => Language::CSharp,
            "go" => Language::Go,
            "py" => Language::Python,
            "rb" => Language::Ruby,
            "php" => Language::Php,
            "hs" => Language::Haskell,
            "erl" | "hrl" => Language::Erlang,
            "ex" | "exs" => Language::Elixir,
            "js" => Language::JavaScript,
            "jsx" => Language::Jsx,
            "ts" => Language::TypeScript,
            "tsx" => Language::Tsx,
            "json" => Language::Json,
            "toml" => Language::Toml,
            "yaml" | "yml" => Language::Yaml,
            "ini" => Language::Ini,
            "md" | "markdown" => Language::Markdown,
            "sql" => Language::Sql,
            "html" | "htm" => Language::Html,
            "css" => Language::Css,
            "scss" | "sass" => Language::Scss,
            "less" => Language::Less,
            "lua" => Language::Lua,
            "zig" => Language::Zig,
            "dart" => Language::Dart,
            "scala" => Language::Scala,
            "sh" | "bash" => Language::Shell,
            "fish" => Language::Fish,
            "ps1" => Language::PowerShell,
            "bat" => Language::Batch,
            "vue" => Language::Vue,
            "svelte" => Language::Svelte,
            "nix" => Language::Nix,
            _ => Language::PlainText,
        },
        None => Language::PlainText,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::{Write, BufWriter};

    fn create_test_file(path: &str, lines: usize) -> std::io::Result<()> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);

        for i in 0..lines {
            writeln!(writer, "Line {}: This is a test line with some content to simulate a real file. Line number {} contains some sample text.", i + 1, i + 1)?;
        }

        writer.flush()?;
        Ok(())
    }

    #[test]
    fn detects_language_from_extension() {
        let doc = Document::new(Some("/tmp/test.rs".into()), String::new());
        assert_eq!(doc.language(), Language::Rust);
    }
}