use vedit_syntax::Language;
use crate::sticky::StickyNote;
use vedit_text::TextBuffer;
use std::cmp;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::ops::Range;
use memmap2::{Mmap, MmapOptions};
use parking_lot::RwLock;
use crossbeam::channel::{self, Sender};
use std::thread;
use vedit_config::StickyNoteRecord;

/// Viewport configuration for rendering large files
#[derive(Debug, Clone)]
pub struct Viewport {
    pub start_line: usize,
    pub visible_lines: usize,
    pub line_height: f32,
    pub buffer_capacity: usize,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            start_line: 0,
            visible_lines: 100,
            line_height: 1.5,
            buffer_capacity: 1000, // Keep ~1000 lines in memory
        }
    }
}

/// Line index for fast byte offset -> line number mapping
#[derive(Debug, Clone)]
pub struct LineIndex {
    /// Maps line number to byte offset in the file
    line_to_offset: Vec<usize>,
    /// Maps byte offset to approximate line number (for fast lookups)
    offset_to_line: HashMap<usize, usize>,
    total_lines: usize,
}

impl LineIndex {
    pub fn new() -> Self {
        Self {
            line_to_offset: vec![0],
            offset_to_line: HashMap::new(),
            total_lines: 0,
        }
    }

    pub fn from_mmap(mmap: &Mmap) -> Self {
        let mut line_to_offset = vec![0];
        let mut offset_to_line = HashMap::new();
        let mut line_num = 1;

        for (offset, byte) in mmap.iter().enumerate() {
            if *byte == b'\n' {
                line_to_offset.push(offset + 1);
                offset_to_line.insert(offset + 1, line_num);
                line_num += 1;
            }
        }

        Self {
            line_to_offset,
            offset_to_line,
            total_lines: line_num - 1,
        }
    }

    pub fn line_to_offset(&self, line: usize) -> usize {
        self.line_to_offset.get(line).copied().unwrap_or(0)
    }

    pub fn offset_to_line(&self, offset: usize) -> usize {
        // Binary search to find the line containing this offset
        match self.line_to_offset.binary_search(&offset) {
            Ok(line) => line,
            Err(insert_pos) => insert_pos.saturating_sub(1),
        }
    }

    pub fn line_range(&self, start_line: usize, end_line: usize) -> Range<usize> {
        let start = self.line_to_offset(start_line);
        let end = if end_line >= self.line_to_offset.len() {
            start // Empty range
        } else {
            self.line_to_offset(end_line)
        };
        start..end
    }

    pub fn total_lines(&self) -> usize {
        self.total_lines
    }
}

/// Memory-mapped document for handling large files
#[derive(Debug)]
pub struct MappedDocument {
    path: Option<String>,
    mmap: Mmap,
    viewport: Viewport,
    line_index: Arc<RwLock<LineIndex>>,
    line_cache: Arc<RwLock<HashMap<usize, String>>>,
    cache_capacity: usize,
    fingerprint: Option<u64>,
    sticky_notes: Vec<StickyNote>,
    indexing_complete: Arc<RwLock<bool>>,
    index_sender: Option<Sender<usize>>,
}

impl MappedDocument {
    pub fn from_path(path: impl AsRef<Path>) -> io::Result<Self> {
        let path_buf = path.as_ref().to_path_buf();
        let file = fs::File::open(&path_buf)?;
        let mmap = unsafe { MmapOptions::new().map(&file)? };

        let fingerprint = Some(compute_fingerprint(&path_buf.to_string_lossy()));
        let path_str = Some(path_buf.to_string_lossy().to_string());

        // Start background line indexing
        let (index_sender, _index_receiver) = channel::unbounded::<usize>();
        let line_index = Arc::new(RwLock::new(LineIndex::new()));
        let indexing_complete = Arc::new(RwLock::new(false));

        // Clone mmap for background thread (Mmap is Send + Sync)
        let mmap_for_indexing = unsafe {
            MmapOptions::new().map(&fs::File::open(&path_buf)?)?
        };

        let line_index_bg = Arc::clone(&line_index);
        let indexing_complete_bg = Arc::clone(&indexing_complete);

        thread::spawn(move || {
            let index = LineIndex::from_mmap(&mmap_for_indexing);
            *line_index_bg.write() = index;
            *indexing_complete_bg.write() = true;
        });

        Ok(Self {
            path: path_str,
            mmap,
            viewport: Viewport::default(),
            line_index,
            line_cache: Arc::new(RwLock::new(HashMap::new())),
            cache_capacity: 1000,
            fingerprint,
            sticky_notes: Vec::new(),
            indexing_complete,
            index_sender: Some(index_sender),
        })
    }

    pub fn set_viewport(&mut self, start_line: usize, visible_lines: usize) {
        self.viewport.start_line = start_line;
        self.viewport.visible_lines = visible_lines;

        // Prefetch lines around the viewport
        self.prefetch_lines(start_line, visible_lines);
    }

    fn prefetch_lines(&self, start_line: usize, visible_lines: usize) {
        let prefetch_start = start_line.saturating_sub(self.viewport.buffer_capacity / 4);
        let prefetch_end = (start_line + visible_lines + self.viewport.buffer_capacity / 4)
            .min(self.line_index.read().total_lines());

        for line_num in prefetch_start..prefetch_end {
            if !self.line_cache.read().contains_key(&line_num) {
                self.load_line(line_num);
            }
        }

        // Evict lines far from viewport
        self.evict_distant_lines(start_line, visible_lines);
    }

    fn load_line(&self, line_num: usize) {
        let line_index = self.line_index.read();
        let next_line_offset = line_index.line_to_offset(line_num + 1);
        let line_start = line_index.line_to_offset(line_num);
        drop(line_index);

        if line_start < self.mmap.len() {
            let line_end = if next_line_offset > self.mmap.len() {
                self.mmap.len()
            } else {
                next_line_offset.saturating_sub(1) // Don't include newline
            };

            if line_end > line_start {
                let line_data = &self.mmap[line_start..line_end];
                if let Ok(line_str) = std::str::from_utf8(line_data) {
                    self.line_cache.write().insert(line_num, line_str.to_string());
                }
            } else {
                self.line_cache.write().insert(line_num, String::new());
            }
        }
    }

    fn evict_distant_lines(&self, viewport_start: usize, visible_lines: usize) {
        let cache = self.line_cache.read();
        if cache.len() <= self.cache_capacity {
            return;
        }

        let eviction_distance = self.viewport.buffer_capacity;
        let mut to_evict = Vec::new();

        for (&line_num, _) in cache.iter() {
            if line_num < viewport_start.saturating_sub(eviction_distance) ||
               line_num > viewport_start + visible_lines + eviction_distance {
                to_evict.push(line_num);
            }
        }

        drop(cache);

        let mut cache = self.line_cache.write();
        for line_num in to_evict.iter().take(cache.len() - self.cache_capacity) {
            cache.remove(line_num);
        }
    }

    pub fn get_line(&self, line_num: usize) -> Option<String> {
        // Check cache first
        if let Some(line) = self.line_cache.read().get(&line_num) {
            return Some(line.clone());
        }

        // Load the line
        self.load_line(line_num);
        self.line_cache.read().get(&line_num).cloned()
    }

    pub fn get_lines(&self, start_line: usize, count: usize) -> Vec<String> {
        let mut lines = Vec::with_capacity(count);
        let end_line = start_line + count;

        for line_num in start_line..end_line {
            if let Some(line) = self.get_line(line_num) {
                lines.push(line);
            } else {
                break;
            }
        }

        lines
    }

    pub fn total_lines(&self) -> usize {
        self.line_index.read().total_lines()
    }

    pub fn is_indexing_complete(&self) -> bool {
        *self.indexing_complete.read()
    }

    pub fn viewport(&self) -> &Viewport {
        &self.viewport
    }
}

/// Represents an open file in the editor workspace.
#[derive(Debug, Clone)]
pub struct Document {
    pub path: Option<String>,
    pub buffer: TextBuffer,
    pub is_modified: bool,
    pub fingerprint: Option<u64>,
    pub sticky_notes: Vec<StickyNote>,
}

impl Document {
    pub fn new(path: Option<String>, buffer: impl Into<TextBuffer>) -> Self {
        let fingerprint = path.as_ref().map(|path| compute_fingerprint(path));
        Self {
            path,
            buffer: buffer.into(),
            is_modified: false,
            fingerprint,
            sticky_notes: Vec::new(),
        }
    }

    /// Update the document path and refresh its fingerprint.
    pub fn set_path(&mut self, path: String) {
        self.fingerprint = Some(compute_fingerprint(&path));
        self.path = Some(path);
    }

    /// Mark the document as unchanged relative to disk.
    pub fn mark_clean(&mut self) {
        self.is_modified = false;
    }

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

    pub fn language(&self) -> Language {
        self.path
            .as_deref()
            .map(detect_language_from_path)
            .unwrap_or(Language::PlainText)
    }

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
            let offset = offset_for_line_column(contents, record.line, record.column);
            let clamped = cmp::min(offset, contents.len());
            let (line, column) = position_for_offset(contents, clamped);
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
            let (line, column) = position_for_offset(contents, clamped);
            note.update(line, column, clamped);
        }

        changed
    }

    pub fn offset_for_line_column(contents: &str, line: usize, column: usize) -> usize {
        offset_for_line_column(contents, line, column)
    }

    pub fn line_column_for_offset(contents: &str, offset: usize) -> (usize, usize) {
        position_for_offset(contents, offset)
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new(None, TextBuffer::new())
    }
}

fn compute_fingerprint(path: &str) -> u64 {
    let resolved = canonicalize_lossy(path);
    let mut hasher = DefaultHasher::new();
    resolved.hash(&mut hasher);
    hasher.finish()
}

fn canonicalize_lossy(path: &str) -> String {
    let path_buf = PathBuf::from(path);
    fs::canonicalize(&path_buf)
        .unwrap_or(path_buf)
        .to_string_lossy()
        .into_owned()
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

fn offset_for_line_column(contents: &str, line: usize, column: usize) -> usize {
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

fn position_for_offset(contents: &str, offset: usize) -> (usize, usize) {
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

    #[test]
    fn detects_language_from_special_file_name() {
        let doc = Document::new(Some("/tmp/Makefile".into()), String::new());
        assert_eq!(doc.language(), Language::Makefile);
    }

    #[test]
    fn defaults_to_plain_text_without_path() {
        let doc = Document::default();
        assert_eq!(doc.language(), Language::PlainText);
    }

    #[test]
    fn computes_line_column_offsets() {
        let text = "fn main()\nprintln!(\"hi\");";
        let offset = super::offset_for_line_column(text, 2, 5);
        assert_eq!(&text[offset..offset + 3], "ln!(");

        let pos = super::position_for_offset(text, offset);
        assert_eq!(pos, (2, 5));
    }

    #[test]
    fn test_memory_mapped_performance() {
        // Create a test file with 10,000 lines (~500KB)
        let test_file_path = "test_large_file.txt";
        let num_lines = 10_000;

        println!("Creating test file with {} lines...", num_lines);
        create_test_file(test_file_path, num_lines).expect("Failed to create test file");

        // Test regular document loading
        println!("Testing regular document loading...");
        let start = std::time::Instant::now();
        let regular_result = Document::from_path(test_file_path);
        let regular_time = start.elapsed();

        if let Ok(doc) = regular_result {
            println!("✓ Regular loading: {:?} ({} chars)", regular_time, doc.buffer.len());
        } else {
            println!("✗ Regular loading failed");
        }

        // Test smart document loading (with memory mapping for large files)
        println!("Testing smart document loading...");
        let start = std::time::Instant::now();
        let smart_result = Document::from_path_smart(test_file_path);
        let smart_time = start.elapsed();

        if let Ok(doc) = smart_result {
            println!("✓ Smart loading: {:?} ({} chars)", smart_time, doc.buffer.len());
        } else {
            println!("✗ Smart loading failed");
        }

        // Test memory-mapped document
        println!("Testing memory-mapped document...");
        let start = std::time::Instant::now();
        let mapped_result = MappedDocument::from_path(test_file_path);
        let mapped_time = start.elapsed();

        if let Ok(doc) = mapped_result {
            println!("✓ Memory-mapped loading: {:?} ({} total lines)", mapped_time, doc.total_lines());

            // Test viewport operations
            println!("Testing viewport operations...");
            let viewport_start = 1_000;
            let viewport_size = 100;

            let start = std::time::Instant::now();
            let lines = doc.get_lines(viewport_start, viewport_size);
            let viewport_time = start.elapsed();

            println!("✓ Viewport retrieval ({} lines starting at {}): {:?}",
                lines.len(), viewport_start, viewport_time);

            // Test random access
            println!("Testing random access...");
            let test_lines = [0, 1_000, 5_000, num_lines - 1];
            let mut random_access_time = std::time::Duration::ZERO;

            for &line_num in &test_lines {
                let start = std::time::Instant::now();
                if let Some(_line) = doc.get_line(line_num) {
                    random_access_time += start.elapsed();
                }
            }

            println!("✓ Random access (4 lines): {:?}", random_access_time);
        } else {
            println!("✗ Memory-mapped loading failed");
        }

        // Clean up
        std::fs::remove_file(test_file_path).ok();

        println!("Performance test completed!");
    }
}

/// Load content from a specific viewport of a memory-mapped file (optimized)
fn load_viewport_content(mmap: &Mmap, start_line: usize, visible_lines: usize) -> String {
    // Use line index for fast lookup if available, otherwise fall back to linear scan
    let line_index = LineIndex::from_mmap(mmap);

    let start_offset = line_index.line_to_offset(start_line);
    let end_line = (start_line + visible_lines).min(line_index.total_lines());
    let end_offset = line_index.line_to_offset(end_line);

    // Safety bounds check
    if start_offset >= mmap.len() {
        return String::new();
    }

    let actual_end = end_offset.min(mmap.len());

    // Extract the exact bytes we need
    let slice = &mmap[start_offset..actual_end];

    // Convert to string, handling potential UTF-8 errors gracefully
    let content = String::from_utf8_lossy(slice).to_string();

    // Remove trailing newline for cleaner display
    content.trim_end_matches('\n').to_string()
}

/// Count total lines in a memory-mapped file
fn count_lines_in_mmap(mmap: &Mmap) -> usize {
    mmap.iter().filter(|&&byte| byte == b'\n').count() + 1
}
