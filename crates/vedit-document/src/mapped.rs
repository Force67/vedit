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
    if mmap.is_empty() {
        return 0; // Empty file has 0 lines
    }
    let newline_count = mmap.iter().filter(|&&byte| byte == b'\n').count();
    if mmap[mmap.len() - 1] == b'\n' {
        newline_count  // Don't count empty line after final newline
    } else {
        newline_count + 1  // Count the last line if no trailing newline
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::{Write, BufWriter};
    use tempfile::tempdir;

    fn create_test_mmap_file(path: &str, lines: usize) -> io::Result<Mmap> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);

        for i in 0..lines {
            writeln!(writer, "Line {}: Test content for line {}", i + 1, i + 1)?;
        }

        writer.flush()?;

        let file = std::fs::File::open(path)?;
        unsafe { MmapOptions::new().map(&file) }
    }

    fn create_large_mmap_file(path: &str, size_mb: usize) -> io::Result<Mmap> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);
        let line_content = "This is a test line for memory mapping with reasonable length.\n";
        let line_size = line_content.len();
        let total_lines = (size_mb * 1024 * 1024) / line_size;

        for i in 0..total_lines {
            writeln!(writer, "Line {}: {}", i + 1, line_content.trim())?;
        }

        writer.flush()?;

        let file = std::fs::File::open(path)?;
        unsafe { MmapOptions::new().map(&file) }
    }

    #[test]
    fn test_mapped_document_creation() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        let path_str = file_path.to_str().unwrap();

        let mmap = create_test_mmap_file(path_str, 1000).unwrap();
        let doc = MappedDocument::from_path(path_str).unwrap();

        assert_eq!(doc.total_lines(), 1000);
        assert!(!doc.is_empty());
        assert_eq!(doc.path(), file_path.as_path());
    }

    #[test]
    fn test_mapped_document_empty_file() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("empty.txt");
        let path_str = file_path.to_str().unwrap();

        File::create(&file_path).unwrap();
        let doc = MappedDocument::from_path(path_str).unwrap();

        assert_eq!(doc.total_lines(), 0);
        assert!(doc.is_empty());
        assert_eq!(doc.file_size(), 0);
    }

    #[test]
    fn test_get_single_line() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("single_line.txt");
        let path_str = file_path.to_str().unwrap();

        let mmap = create_test_mmap_file(path_str, 100).unwrap();
        let doc = MappedDocument::from_path(path_str).unwrap();

        let line = doc.get_line(50).unwrap();
        assert!(line.contains("Line 51"));
        assert!(!line.contains('\n'));

        // Test invalid line numbers
        assert!(doc.get_line(100).is_none());
        assert!(doc.get_line(1000).is_none());
    }

    #[test]
    fn test_get_line_range() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("range_test.txt");
        let path_str = file_path.to_str().unwrap();

        let mmap = create_test_mmap_file(path_str, 1000).unwrap();
        let doc = MappedDocument::from_path(path_str).unwrap();

        let range_content = doc.get_line_range(100, 110).unwrap();
        let lines: Vec<&str> = range_content.lines().collect();

        assert_eq!(lines.len(), 10);
        assert!(lines[0].contains("Line 101"));
        assert!(lines[9].contains("Line 110"));

        // Test edge cases
        assert!(doc.get_line_range(1000, 1001).is_none());
        assert!(doc.get_line_range(999, 1000).is_some()); // Last line
    }

    #[test]
    fn test_viewport_content_loading() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("viewport.txt");
        let path_str = file_path.to_str().unwrap();

        let mmap = create_large_mmap_file(path_str, 10).unwrap(); // 10MB file
        let doc = MappedDocument::from_path(path_str).unwrap();

        let viewport = Viewport {
            start_line: 5000,
            visible_lines: 100,
            line_height: 1.5,
            buffer_capacity: 1000,
        };

        let content = doc.get_viewport_content(&viewport);
        let lines: Vec<&str> = content.lines().collect();

        assert_eq!(lines.len(), 100);
        assert!(lines[0].contains("Line 5001"));
        assert!(lines[99].contains("Line 5100"));

        // Verify memory usage is reasonable
        assert!(content.len() < 50 * 1024); // Less than 50KB
    }

    #[test]
    fn test_offset_to_line_conversion() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("offset_test.txt");
        let path_str = file_path.to_str().unwrap();

        let mmap = create_test_mmap_file(path_str, 100).unwrap();
        let doc = MappedDocument::from_path(path_str).unwrap();

        // Test some offset conversions
        let line0_offset = doc.line_to_offset(0);
        assert_eq!(line0_offset, 0);
        assert_eq!(doc.offset_to_line(0), 0);

        let line1_offset = doc.line_to_offset(1);
        assert!(line1_offset > 0);
        assert_eq!(doc.offset_to_line(line1_offset), 1);

        let line10_offset = doc.line_to_offset(10);
        assert!(line10_offset > 0);
        assert_eq!(doc.offset_to_line(line10_offset), 10);

        // Test boundary conditions
        assert_eq!(doc.offset_to_line(doc.file_size() as usize - 1), 99);
    }

    #[test]
    fn test_large_file_performance() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("large_perf.txt");
        let path_str = file_path.to_str().unwrap();

        let start = std::time::Instant::now();
        let mmap = create_large_mmap_file(path_str, 50).unwrap(); // 50MB
        let creation_time = start.elapsed();

        let doc = MappedDocument::from_path(path_str).unwrap();

        // Document creation should be fast with memory mapping
        assert!(creation_time.as_millis() < 1000, "Creation took {}ms", creation_time.as_millis());

        // Random access should be fast
        let start = std::time::Instant::now();
        for i in [0, 1000, 10000, 50000, 100000] {
            if i < doc.total_lines() {
                let _line = doc.get_line(i);
            }
        }
        let access_time = start.elapsed();

        assert!(access_time.as_millis() < 100, "Random access took {}ms", access_time.as_millis());
    }

    #[test]
    fn test_viewport_boundary_conditions() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("boundary_test.txt");
        let path_str = file_path.to_str().unwrap();

        let mmap = create_test_mmap_file(path_str, 100).unwrap();
        let doc = MappedDocument::from_path(path_str).unwrap();

        // Test viewport at file start
        let start_viewport = Viewport {
            start_line: 0,
            visible_lines: 10,
            line_height: 1.5,
            buffer_capacity: 1000,
        };
        let start_content = doc.get_viewport_content(&start_viewport);
        assert!(start_content.contains("Line 1"));

        // Test viewport at file end
        let end_viewport = Viewport {
            start_line: 95,
            visible_lines: 20,
            line_height: 1.5,
            buffer_capacity: 1000,
        };
        let end_content = doc.get_viewport_content(&end_viewport);
        let end_lines: Vec<&str> = end_content.lines().collect();
        assert!(end_lines.len() <= 5); // Should be truncated
        assert!(end_lines.last().unwrap().contains("Line 100"));

        // Test viewport beyond file
        let beyond_viewport = Viewport {
            start_line: 200,
            visible_lines: 10,
            line_height: 1.5,
            buffer_capacity: 1000,
        };
        let beyond_content = doc.get_viewport_content(&beyond_viewport);
        assert!(beyond_content.is_empty());
    }

    #[test]
    fn test_memory_efficiency() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("memory_test.txt");
        let path_str = file_path.to_str().unwrap();

        let mmap = create_large_mmap_file(path_str, 100).unwrap(); // 100MB file
        let doc = MappedDocument::from_path(path_str).unwrap();

        // The MappedDocument itself should be small
        let doc_size = std::mem::size_of::<MappedDocument>();
        assert!(doc_size < 1024, "MappedDocument size: {} bytes", doc_size);

        // Viewport content should be small relative to file
        let viewport = Viewport {
            start_line: 10000,
            visible_lines: 1000,
            line_height: 1.5,
            buffer_capacity: 1000,
        };
        let content = doc.get_viewport_content(&viewport);

        assert!(content.len() < 100 * 1024, "Viewport content size: {} bytes", content.len());

        // Memory usage should not scale with file size
        let file_size_mb = doc.file_size() as f64 / (1024.0 * 1024.0);
        let content_size_kb = content.len() as f64 / 1024.0;
        let ratio = content_size_kb / file_size_mb;

        assert!(ratio < 10.0, "Content/file size ratio: {}", ratio);
    }

    #[test]
    fn test_line_index_accuracy() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("index_test.txt");
        let path_str = file_path.to_str().unwrap();

        let mmap = create_test_mmap_file(path_str, 1000).unwrap();
        let doc = MappedDocument::from_path(path_str).unwrap();

        // Test that line index is accurate
        for line_num in [0, 100, 500, 999] {
            let offset = doc.line_to_offset(line_num);
            let computed_line = doc.offset_to_line(offset);
            assert_eq!(computed_line, line_num, "Line index mismatch for line {}", line_num);
        }

        // Test that line ranges are correct
        let range = doc.line_index.line_range(100, 105);
        let range_content = std::str::from_utf8(&doc.as_bytes()[range]).unwrap();
        let range_lines: Vec<&str> = range_content.lines().collect();

        assert_eq!(range_lines.len(), 5);
        assert!(range_lines[0].contains("Line 101"));
        assert!(range_lines[4].contains("Line 105"));
    }

    #[test]
    fn test_unicode_handling() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("unicode_test.txt");
        let path_str = file_path.to_str().unwrap();

        let file = File::create(&file_path).unwrap();
        let mut writer = BufWriter::new(file);
        writeln!(writer, "Line 1: ASCII text").unwrap();
        writeln!(writer, "Line 2: Unicode text: café, naïve, résumé").unwrap();
        writeln!(writer, "Line 3: Emoji: 🚀 🎉 🦀").unwrap();
        writeln!(writer, "Line 4: Mixed: Hello 世界 🌍").unwrap();
        writer.flush().unwrap();

        let doc = MappedDocument::from_path(path_str).unwrap();

        let line2 = doc.get_line(1).unwrap();
        assert!(line2.contains("café"));

        let line3 = doc.get_line(2).unwrap();
        assert!(line3.contains("🚀"));

        let line4 = doc.get_line(3).unwrap();
        assert!(line4.contains("世界"));
    }

    #[test]
    fn test_load_viewport_content_function() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("func_test.txt");
        let path_str = file_path.to_str().unwrap();

        let mmap = create_large_mmap_file(path_str, 5).unwrap(); // 5MB file

        let content = load_viewport_content(&mmap, 1000, 100);
        let lines: Vec<&str> = content.lines().collect();

        assert_eq!(lines.len(), 100);
        assert!(lines[0].contains("Line 1001"));
        assert!(lines[99].contains("Line 1100"));

        // Test beyond file bounds
        let empty_content = load_viewport_content(&mmap, 100000, 100);
        assert!(empty_content.is_empty());
    }

    #[test]
    fn test_count_lines_in_mmap_function() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("count_test.txt");
        let path_str = file_path.to_str().unwrap();

        let mmap = create_test_mmap_file(path_str, 500).unwrap();
        let line_count = count_lines_in_mmap(&mmap);

        assert_eq!(line_count, 500);

        // Test with empty file
        let empty_file = temp_dir.path().join("empty.txt");
        File::create(&empty_file).unwrap();
        let empty_mmap = unsafe {
            let file = std::fs::File::open(&empty_file).unwrap();
            MmapOptions::new().map(&file).unwrap()
        };
        let empty_count = count_lines_in_mmap(&empty_mmap);
        assert_eq!(empty_count, 0); // Empty file should count as 0 lines
    }
}