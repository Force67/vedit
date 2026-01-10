use crate::mapped::count_lines_in_mmap;
use crate::mapped::load_viewport_content;
use memmap2::MmapOptions;
use std::cmp;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io;
use std::path::{Path, PathBuf};
use vedit_config::{StickyNote, StickyNoteRecord};
use vedit_syntax::Language;
use vedit_text::TextBuffer;

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
        let contents = fs::read(&path_buf)?;

        // Try to decode as UTF-8, handling invalid UTF-8 sequences gracefully
        let contents = String::from_utf8(contents).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("File contains invalid UTF-8: {}", e),
            )
        })?;

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

        // Use memory mapping for files larger than 5MB (reduced for tests)
        if file_size > 5 * 1024 * 1024 {
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
                    if metadata.len() > 5 * 1024 * 1024 {
                        if let Ok(mmap) = unsafe { MmapOptions::new().map(&file) } {
                            return Some(load_viewport_content(&mmap, start_line, visible_lines));
                        }
                    }
                }
            }
        }

        // For small files, extract from current buffer
        let content = self.content();
        let lines: Vec<&str> = content
            .lines()
            .skip(start_line)
            .take(visible_lines)
            .collect();
        Some(lines.join("\n"))
    }

    /// Get total line count for the file
    pub fn total_lines(&self) -> Option<usize> {
        if let Some(path) = &self.path {
            if let Ok(file) = fs::File::open(path) {
                if let Ok(metadata) = file.metadata() {
                    if metadata.len() > 5 * 1024 * 1024 {
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
                    return metadata.len() > 5 * 1024 * 1024;
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

    pub fn set_sticky_notes_from_records(&mut self, records: &[StickyNoteRecord], contents: &str) {
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
    use std::io::{BufWriter, Write};
    use std::path::PathBuf;
    use std::time::Instant;
    use tempfile::tempdir;

    fn create_test_file(path: &str, lines: usize) -> std::io::Result<()> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);

        for i in 0..lines {
            writeln!(
                writer,
                "Line {}: This is a test line with some content to simulate a real file. Line number {} contains some sample text.",
                i + 1,
                i + 1
            )?;
        }

        writer.flush()?;
        Ok(())
    }

    fn create_large_test_file(path: &str, size_mb: usize) -> std::io::Result<()> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);
        let line_content = "This is a test line with some content to simulate a real file with reasonable line length.\n";
        let line_size = line_content.len();
        let total_lines = (size_mb * 1024 * 1024) / line_size;

        for i in 0..total_lines {
            writeln!(writer, "Line {}: {}", i + 1, line_content.trim())?;
        }

        writer.flush()?;
        Ok(())
    }

    fn estimate_memory_usage(content: &str) -> usize {
        content.len() + std::mem::size_of::<TextBuffer>()
    }

    fn cleanup_test_file(path: &str) {
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn detects_language_from_extension() {
        let doc = Document::new(Some("/tmp/test.rs".into()), String::new());
        assert_eq!(doc.language(), Language::Rust);
    }

    #[test]
    fn test_small_file_uses_regular_loading() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("small_test.txt");
        let path_str = file_path.to_str().unwrap();

        create_test_file(path_str, 100).unwrap();

        let doc = Document::from_path_smart(path_str).unwrap();

        assert!(!doc.is_streaming());
        assert_eq!(doc.total_lines(), Some(100));
        assert!(doc.content().lines().count() >= 100);
    }

    #[test]
    fn test_large_file_uses_mmap() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("large_test.txt");
        let path_str = file_path.to_str().unwrap();

        create_large_test_file(path_str, 15).unwrap(); // 15MB file

        let doc = Document::from_path_smart(path_str).unwrap();

        assert!(doc.is_streaming());
        assert!(doc.total_lines().is_some());
        assert!(doc.total_lines().unwrap() > 1000); // Should have more than 1000 lines

        // Initial buffer should contain only ~1000 lines, not entire file
        let loaded_lines = doc.content().lines().count();
        assert!(
            loaded_lines <= 1000,
            "Loaded {} lines, expected <= 1000",
            loaded_lines
        );
    }

    #[test]
    fn test_viewport_loads_only_requested_range() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("viewport_test.txt");
        let path_str = file_path.to_str().unwrap();

        create_large_test_file(path_str, 6).unwrap(); // 6MB file

        let doc = Document::from_path_smart(path_str).unwrap();
        assert!(doc.is_streaming());

        // Request viewport around line 5000
        let viewport_content = doc.load_viewport(5000, 100).unwrap();
        let viewport_lines: Vec<&str> = viewport_content.lines().collect();

        assert_eq!(viewport_lines.len(), 100);
        assert!(viewport_lines[0].contains("5001")); // Should start around line 5001
        assert!(viewport_lines[99].contains("5100")); // Should end around line 5100

        // Verify we're not loading the entire file
        let estimated_usage = estimate_memory_usage(&viewport_content);
        assert!(
            estimated_usage < 1024 * 1024,
            "Memory usage {} exceeds 1MB",
            estimated_usage
        );
    }

    #[test]
    fn test_viewport_switching_loads_correct_content() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("switching_test.txt");
        let path_str = file_path.to_str().unwrap();

        create_large_test_file(path_str, 6).unwrap(); // 6MB file

        let doc = Document::from_path_smart(path_str).unwrap();
        assert!(doc.is_streaming());

        // Load first viewport
        let viewport1 = doc.load_viewport(1000, 50).unwrap();
        let lines1: Vec<&str> = viewport1.lines().collect();
        assert!(lines1[0].contains("1001"));

        // Load different viewport
        let viewport2 = doc.load_viewport(8000, 50).unwrap();
        let lines2: Vec<&str> = viewport2.lines().collect();
        assert!(lines2[0].contains("8001"));

        // Verify content is different
        assert_ne!(lines1[0], lines2[0]);

        // Verify memory usage stays reasonable for both viewports
        assert!(estimate_memory_usage(&viewport1) < 100 * 1024);
        assert!(estimate_memory_usage(&viewport2) < 100 * 1024);
    }

    #[test]
    fn test_update_viewport() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("update_test.txt");
        let path_str = file_path.to_str().unwrap();

        create_large_test_file(path_str, 6).unwrap(); // 6MB file

        let mut doc = Document::from_path_smart(path_str).unwrap();
        assert!(doc.is_streaming());

        // Initial content should be first ~1000 lines
        let initial_content = doc.content();
        assert!(initial_content.lines().count() <= 1000);

        // Update to different viewport
        let updated = doc.update_viewport(5000, 100);
        assert!(updated);

        let new_content = doc.content();
        let new_lines: Vec<&str> = new_content.lines().collect();

        assert_eq!(new_lines.len(), 100);
        assert!(new_lines[0].contains("5001"));
    }

    #[test]
    fn test_viewport_edge_cases() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("edge_case_test.txt");
        let path_str = file_path.to_str().unwrap();

        create_large_test_file(path_str, 6).unwrap(); // 6MB file for streaming
        let doc = Document::from_path_smart(path_str).unwrap();
        assert!(doc.is_streaming());

        // Test loading beyond file bounds
        let beyond_end = doc.load_viewport(200000, 100);
        assert!(beyond_end.is_some());
        assert!(beyond_end.unwrap().is_empty());

        // Test empty viewport
        let empty = doc.load_viewport(5000, 0);
        assert!(empty.is_some());
        assert!(empty.unwrap().is_empty());

        // Test single line viewport
        let single = doc.load_viewport(5000, 1).unwrap();
        assert_eq!(single.lines().count(), 1);
    }

    #[test]
    fn test_total_lines_accuracy() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("lines_test.txt");
        let path_str = file_path.to_str().unwrap();

        create_test_file(path_str, 5000).unwrap();
        let doc = Document::from_path_smart(path_str).unwrap();

        let total_lines = doc.total_lines().unwrap();
        assert_eq!(total_lines, 5000);
    }

    #[test]
    fn test_memory_usage_doesnt_grow_with_viewport_changes() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("memory_test.txt");
        let path_str = file_path.to_str().unwrap();

        create_large_test_file(path_str, 10).unwrap(); // 10MB file

        let mut doc = Document::from_path_smart(path_str).unwrap();
        let mut max_memory = 0;

        // Perform multiple viewport changes
        for i in 0..10 {
            let line = (i + 1) * 1000;
            doc.update_viewport(line, 100);
            let current_memory = estimate_memory_usage(&doc.content());
            max_memory = max_memory.max(current_memory);
        }

        // Memory usage should stay bounded (not grow with each viewport change)
        assert!(
            max_memory < 500 * 1024,
            "Max memory {} exceeded 500KB",
            max_memory
        );
    }

    #[test]
    fn test_large_file_initialization_time() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("performance_test.txt");
        let path_str = file_path.to_str().unwrap();

        create_large_test_file(path_str, 15).unwrap(); // 15MB file

        let start = Instant::now();
        let _doc = Document::from_path_smart(path_str).unwrap();
        let elapsed = start.elapsed();

        // Large file initialization should be fast (< 2s)
        assert!(
            elapsed.as_millis() < 2000,
            "Initialization took {}ms, expected < 2000ms",
            elapsed.as_millis()
        );
    }

    #[test]
    fn test_is_streaming_detection() {
        let temp_dir = tempdir().unwrap();

        // Small file
        let small_file = temp_dir.path().join("small.txt");
        let small_path = small_file.to_str().unwrap();
        create_test_file(small_path, 100).unwrap();
        let small_doc = Document::from_path_smart(small_path).unwrap();
        assert!(!small_doc.is_streaming());

        // Large file
        let large_file = temp_dir.path().join("large.txt");
        let large_path = large_file.to_str().unwrap();
        create_large_test_file(large_path, 6).unwrap(); // 6MB
        let large_doc = Document::from_path_smart(large_path).unwrap();
        assert!(large_doc.is_streaming());
    }

    #[test]
    fn test_viewport_content_boundary_conditions() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("boundary_test.txt");
        let path_str = file_path.to_str().unwrap();

        create_test_file(path_str, 100).unwrap();
        let doc = Document::from_path_smart(path_str).unwrap();

        // Test viewport at end of file
        let end_viewport = doc.load_viewport(90, 20).unwrap();
        let end_lines: Vec<&str> = end_viewport.lines().collect();
        assert!(end_lines.len() <= 10); // Should be truncated to file length
        assert!(end_lines.last().unwrap().contains("Line 100"));

        // Test viewport exactly at file start
        let start_viewport = doc.load_viewport(0, 10).unwrap();
        let start_lines: Vec<&str> = start_viewport.lines().collect();
        assert!(start_lines[0].contains("Line 1"));
    }

    #[test]
    fn test_empty_file_handling() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("empty.txt");
        let path_str = file_path.to_str().unwrap();

        File::create(&file_path).unwrap();
        let doc = Document::from_path_smart(path_str).unwrap();

        assert!(!doc.is_streaming());
        assert_eq!(doc.total_lines(), Some(0)); // Empty file has 0 lines
        assert!(doc.content().is_empty());
    }

    #[test]
    fn test_unicode_file_loading() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("unicode.txt");
        let path_str = file_path.to_str().unwrap();

        // Create a test file with various Unicode content
        let file = File::create(&file_path).unwrap();
        let mut writer = BufWriter::new(file);
        writeln!(writer, "ASCII: Hello World").unwrap();
        writeln!(writer, "Emojis: 🚀🎉🦀😀😃😄😁").unwrap();
        writeln!(writer, "Accents: café résumé naïve").unwrap();
        writeln!(writer, "Chinese: 你好世界").unwrap();
        writeln!(writer, "Japanese: こんにちは世界").unwrap();
        writeln!(writer, "Arabic: مرحبا بالعالم").unwrap();
        writeln!(writer, "Mixed: Hello 世界 🌍 café").unwrap();
        writeln!(writer, "ZWNJ sequences: 👨‍👩‍👧‍👦 🏳️‍🌈").unwrap();
        writer.flush().unwrap();

        let doc = Document::from_path(path_str).unwrap();
        let content = doc.content();

        // Test that all Unicode content is preserved correctly
        assert!(content.contains("Hello World"));
        assert!(content.contains("🚀🎉🦀"));
        assert!(content.contains("café résumé"));
        assert!(content.contains("你好世界"));
        assert!(content.contains("こんにちは世界"));
        assert!(content.contains("مرحبا بالعالم"));
        assert!(content.contains("Hello 世界 🌍 café"));

        // Test that the content length is correct (should be more bytes than ASCII characters)
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 8);

        // Verify Unicode character counting works
        let emoji_line = lines.iter().find(|line| line.contains("Emojis")).unwrap();
        let emoji_count = emoji_line
            .chars()
            .filter(|c| {
                let cp = *c as u32;
                (cp >= 0x1F600 && cp <= 0x1F64F) || // Emoticons
            (cp >= 0x1F300 && cp <= 0x1F5FF) || // Misc Symbols
            (cp >= 0x1F680 && cp <= 0x1F6FF) || // Transport
            (cp >= 0x1F1E0 && cp <= 0x1F1FF) // Flags
            })
            .count();
        assert!(emoji_count >= 6); // Should have at least 6 emojis
    }

    #[test]
    fn test_invalid_utf8_handling() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("invalid_utf8.txt");
        let path_str = file_path.to_str().unwrap();

        // Create a file with invalid UTF-8 sequences
        let file = File::create(&file_path).unwrap();
        let mut writer = BufWriter::new(file);
        writer.write_all(b"Valid text: Hello World\n").unwrap();
        writer.write_all(&[0xFF, 0xFE, 0xFD]).unwrap(); // Invalid UTF-8
        writer.write_all(b"\nMore valid text\n").unwrap();
        writer.flush().unwrap();

        let result = Document::from_path(path_str);
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        // Check for the actual error message from String::from_utf8
        let error_msg = error.to_string();
        assert!(
            error_msg.contains("utf8")
                || error_msg.contains("UTF-8")
                || error_msg.contains("invalid utf8")
        );
        println!("Error message: {}", error_msg);
    }
}
