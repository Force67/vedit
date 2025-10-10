use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use memmap2::Mmap;
use memmap2::MmapOptions;
use crate::line_index::LineIndex;
use crate::viewport::Viewport;

/// Memory-mapped document for large files
#[derive(Debug)]
pub struct MappedDocument {
    pub path: PathBuf,
    mmap: Mmap,
    line_index: LineIndex,
    file_size: u64,
}

impl MappedDocument {
    /// Create a new memory-mapped document from a file path
    pub fn from_path(path: impl AsRef<Path>) -> io::Result<Self> {
        let path_buf = path.as_ref().to_path_buf();
        let file = fs::File::open(&path_buf)?;
        let file_size = file.metadata()?.len();
        let mmap = unsafe { MmapOptions::new().map(&file)? };
        let line_index = LineIndex::from_mmap(&mmap);

        Ok(Self {
            path: path_buf,
            mmap,
            line_index,
            file_size,
        })
    }

    /// Get the file path
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Get the file size in bytes
    pub fn file_size(&self) -> u64 {
        self.file_size
    }

    /// Get the total number of lines
    pub fn total_lines(&self) -> usize {
        self.line_index.total_lines()
    }

    /// Get content for a specific viewport
    pub fn get_viewport_content(&self, viewport: &Viewport) -> String {
        let start_line = viewport.start_line;
        let end_line = (start_line + viewport.visible_lines).min(self.total_lines());

        let range = self.line_index.line_range(start_line, end_line);

        if range.start >= self.mmap.len() {
            return String::new();
        }

        let end = range.end.min(self.mmap.len());
        String::from_utf8_lossy(&self.mmap[range.start..end]).to_string()
    }

    /// Get content for a specific line range
    pub fn get_line_range(&self, start_line: usize, end_line: usize) -> Option<String> {
        if start_line >= self.total_lines() {
            return None;
        }

        let end_line = end_line.min(self.total_lines());
        let range = self.line_index.line_range(start_line, end_line);

        if range.start >= self.mmap.len() {
            return None;
        }

        let end = range.end.min(self.mmap.len());
        Some(String::from_utf8_lossy(&self.mmap[range.start..end]).to_string())
    }

    /// Get a single line by line number
    pub fn get_line(&self, line_num: usize) -> Option<String> {
        if line_num >= self.total_lines() {
            return None;
        }

        self.get_line_range(line_num, line_num + 1)
            .map(|s| s.trim_end_matches('\n').to_string())
    }

    /// Convert byte offset to line number
    pub fn offset_to_line(&self, offset: usize) -> usize {
        self.line_index.offset_to_line(offset)
    }

    /// Convert line number to byte offset
    pub fn line_to_offset(&self, line: usize) -> usize {
        self.line_index.line_to_offset(line)
    }

    /// Check if the document is empty
    pub fn is_empty(&self) -> bool {
        self.file_size == 0
    }

    /// Get the memory-mapped data as bytes (for advanced use cases)
    pub fn as_bytes(&self) -> &[u8] {
        &self.mmap
    }
}

/// Load content from a specific viewport of a memory-mapped file
pub fn load_viewport_content(mmap: &Mmap, start_line: usize, visible_lines: usize) -> String {
    let line_index = LineIndex::from_mmap(mmap);
    let total_lines = line_index.total_lines();

    if start_line >= total_lines {
        return String::new();
    }

    let end_line = (start_line + visible_lines).min(total_lines);
    let range = line_index.line_range(start_line, end_line);

    if range.start >= mmap.len() {
        return String::new();
    }

    let end = range.end.min(mmap.len());
    String::from_utf8_lossy(&mmap[range.start..end]).to_string()
}

/// Count lines in a memory-mapped file
pub fn count_lines_in_mmap(mmap: &Mmap) -> usize {
    mmap.iter().filter(|&&byte| byte == b'\n').count() + 1
}