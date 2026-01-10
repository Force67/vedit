use memmap2::Mmap;
use std::collections::HashMap;
use std::ops::Range;

/// Line index for fast byte offset -> line number mapping
#[derive(Debug, Clone)]
pub struct LineIndex {
    /// Maps line number to byte offset in the file
    line_to_offset: Vec<usize>,
    /// Maps byte offset to approximate line number (for fast lookups)
    // TODO(Vince): This HashMap is populated but never queried - offset_to_line()
    // uses binary search instead. Consider removing to save memory, or use it
    // for O(1) lookups at line boundaries instead of binary search.
    #[allow(dead_code)]
    offset_to_line: HashMap<usize, usize>,
    total_lines: usize,
}

impl LineIndex {
    pub fn new() -> Self {
        Self {
            line_to_offset: vec![0],
            offset_to_line: HashMap::new(),
            total_lines: 0, // Empty document has 0 lines
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

        let total_lines = if mmap.is_empty() {
            0 // Empty file has 0 lines
        } else if mmap[mmap.len() - 1] == b'\n' {
            // File ends with newline, don't count empty line after it
            line_num - 1
        } else {
            // File doesn't end with newline, count all lines
            line_num
        };

        Self {
            line_to_offset,
            offset_to_line,
            total_lines,
        }
    }

    pub fn line_to_offset(&self, line: usize) -> usize {
        self.line_to_offset.get(line).copied().unwrap_or(0)
    }

    pub fn offset_to_line(&self, offset: usize) -> usize {
        // Binary search to find the line containing this offset
        match self.line_to_offset.binary_search(&offset) {
            Ok(line) => {
                // If we're exactly at the start of a line that's beyond the last line,
                // return the last line instead
                if line > 0 && line >= self.total_lines() {
                    self.total_lines().saturating_sub(1)
                } else {
                    line
                }
            }
            Err(insert_pos) => {
                let result = insert_pos.saturating_sub(1);
                // If result is beyond the last valid line, return the last line
                if result >= self.total_lines() {
                    self.total_lines().saturating_sub(1)
                } else {
                    result
                }
            }
        }
    }

    pub fn line_range(&self, start_line: usize, end_line: usize) -> Range<usize> {
        let start = self.line_to_offset(start_line);
        let end = if start_line >= end_line || end_line >= self.line_to_offset.len() {
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

#[cfg(test)]
mod tests {
    use super::*;
    use memmap2::MmapOptions;
    use std::fs::File;
    use std::io::{BufWriter, Write};
    use tempfile::tempdir;

    fn create_test_mmap(content: &str) -> Mmap {
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        let mut writer = BufWriter::new(&temp_file);
        writer.write_all(content.as_bytes()).unwrap();
        writer.flush().unwrap();

        let file = File::open(temp_file.path()).unwrap();
        unsafe { MmapOptions::new().map(&file).unwrap() }
    }

    fn create_multiline_mmap(lines: usize) -> Mmap {
        let content: String = (0..lines)
            .map(|i| format!("Line {}: Test content\n", i + 1))
            .collect();
        create_test_mmap(&content)
    }

    #[test]
    fn test_line_index_empty() {
        let index = LineIndex::new();
        assert_eq!(index.total_lines(), 0);
        assert_eq!(index.line_to_offset(0), 0);
        assert_eq!(index.offset_to_line(0), 0);
    }

    #[test]
    fn test_line_index_single_line() {
        let mmap = create_test_mmap("Single line without newline");
        let index = LineIndex::from_mmap(&mmap);

        assert_eq!(index.total_lines(), 1);
        assert_eq!(index.line_to_offset(0), 0);
        assert_eq!(index.offset_to_line(0), 0);
        assert_eq!(index.offset_to_line(5), 0);
    }

    #[test]
    fn test_line_index_multiple_lines() {
        let mmap = create_test_mmap("Line 1\nLine 2\nLine 3\n");
        let index = LineIndex::from_mmap(&mmap);

        assert_eq!(index.total_lines(), 3);

        // Test line to offset mappings
        assert_eq!(index.line_to_offset(0), 0); // "Line 1"
        assert_eq!(index.line_to_offset(1), 7); // "Line 2"
        assert_eq!(index.line_to_offset(2), 14); // "Line 3"

        // Test offset to line mappings
        assert_eq!(index.offset_to_line(0), 0); // Start of line 1
        assert_eq!(index.offset_to_line(5), 0); // Middle of line 1
        assert_eq!(index.offset_to_line(6), 0); // Before newline of line 1
        assert_eq!(index.offset_to_line(7), 1); // Start of line 2
        assert_eq!(index.offset_to_line(10), 1); // Middle of line 2
        assert_eq!(index.offset_to_line(20), 2); // Line 3
    }

    #[test]
    fn test_line_index_large_file() {
        let mmap = create_multiline_mmap(10000);
        let index = LineIndex::from_mmap(&mmap);

        assert_eq!(index.total_lines(), 10000);

        // Test random mappings
        for line_num in [0, 100, 1000, 5000, 9999] {
            let offset = index.line_to_offset(line_num);
            let computed_line = index.offset_to_line(offset);
            assert_eq!(
                computed_line, line_num,
                "Line mapping failed for {}",
                line_num
            );
        }

        // Test that offsets within lines map to correct line
        for line_num in [0, 100, 1000, 5000] {
            let line_start = index.line_to_offset(line_num);
            let next_line_start = if line_num + 1 < index.total_lines() {
                index.line_to_offset(line_num + 1)
            } else {
                mmap.len()
            };

            // Test various offsets within the line
            for offset in line_start..next_line_start.min(line_start + 10) {
                assert_eq!(
                    index.offset_to_line(offset),
                    line_num,
                    "Offset {} should map to line {}",
                    offset,
                    line_num
                );
            }
        }
    }

    #[test]
    fn test_line_range() {
        let mmap = create_test_mmap("Line 1\nLine 2\nLine 3\nLine 4\nLine 5\n");
        let index = LineIndex::from_mmap(&mmap);

        // Test normal range
        let range = index.line_range(1, 4); // Lines 2-4
        assert_eq!(range.start, 7); // Start of "Line 2"
        assert_eq!(range.end, 28); // Start of "Line 5"

        // Test single line range
        let single_range = index.line_range(2, 3); // Line 3 only
        assert_eq!(single_range.start, 14);
        assert_eq!(single_range.end, 21);

        // Test range at end
        let end_range = index.line_range(4, 5); // Line 5
        assert_eq!(end_range.start, 28);
        assert_eq!(end_range.end, mmap.len());

        // Test invalid range (beyond file)
        let invalid_range = index.line_range(10, 15);
        assert_eq!(invalid_range.start, invalid_range.end); // Empty range
    }

    #[test]
    fn test_line_index_performance() {
        let mmap = create_multiline_mmap(50000);

        let start = std::time::Instant::now();
        let index = LineIndex::from_mmap(&mmap);
        let creation_time = start.elapsed();

        // Index creation should be fast even for large files
        assert!(
            creation_time.as_millis() < 100,
            "LineIndex creation took {}ms for 50k lines",
            creation_time.as_millis()
        );

        // Random lookups should be fast
        let start = std::time::Instant::now();
        for i in 0..1000 {
            let line_num = i * 47 % 50000; // Pseudo-random distribution
            let _offset = index.line_to_offset(line_num);
            let _computed_line = index.offset_to_line(_offset);
        }
        let lookup_time = start.elapsed();

        assert!(
            lookup_time.as_millis() < 50,
            "1000 lookups took {}ms",
            lookup_time.as_millis()
        );
    }

    #[test]
    fn test_line_index_memory_usage() {
        let mmap = create_multiline_mmap(10000);
        let index = LineIndex::from_mmap(&mmap);

        // Index should use reasonable memory
        let index_size = std::mem::size_of::<LineIndex>();
        let estimated_overhead = index.line_to_offset.len() * std::mem::size_of::<usize>()
            + index.offset_to_line.len()
                * (std::mem::size_of::<usize>() + std::mem::size_of::<usize>());

        // Should be less than 1MB for 10k lines
        assert!(
            index_size + estimated_overhead < 1024 * 1024,
            "LineIndex memory usage: {} bytes",
            index_size + estimated_overhead
        );
    }

    #[test]
    fn test_line_index_edge_cases() {
        // File ending with newline
        let mmap_with_newline = create_test_mmap("Line 1\nLine 2\n");
        let index_with_newline = LineIndex::from_mmap(&mmap_with_newline);
        assert_eq!(index_with_newline.total_lines(), 2);

        // File not ending with newline
        let mmap_without_newline = create_test_mmap("Line 1\nLine 2");
        let index_without_newline = LineIndex::from_mmap(&mmap_without_newline);
        assert_eq!(index_without_newline.total_lines(), 2);

        // File with consecutive newlines
        let mmap_consecutive = create_test_mmap("Line 1\n\nLine 3\n");
        let index_consecutive = LineIndex::from_mmap(&mmap_consecutive);
        assert_eq!(index_consecutive.total_lines(), 3);

        // Verify empty line handling
        let empty_line_offset = index_consecutive.line_to_offset(1);
        assert_eq!(empty_line_offset, 7); // After "Line 1\n"
        assert_eq!(index_consecutive.offset_to_line(empty_line_offset), 1);
    }

    #[test]
    fn test_line_index_unicode_handling() {
        let content = "Line 1: ASCII\nLine 2: café 🚀\nLine 3: 世界\n";
        let mmap = create_test_mmap(content);
        let index = LineIndex::from_mmap(&mmap);

        assert_eq!(index.total_lines(), 3);

        // Test that byte offsets work correctly with UTF-8
        let line2_offset = index.line_to_offset(1);
        assert_eq!(line2_offset, 14);

        let line3_offset = index.line_to_offset(2);
        assert_eq!(line3_offset, 33); // Note: This is byte offset, not character offset

        // Verify range extraction works
        let range = index.line_range(1, 2);
        let line2_content = std::str::from_utf8(&mmap[range]).unwrap();
        assert!(line2_content.contains("café 🚀"));
    }

    #[test]
    fn test_line_index_boundary_conditions() {
        let mmap = create_multiline_mmap(1000);
        let index = LineIndex::from_mmap(&mmap);

        // Test offset at exact line boundaries
        for line_num in 0..999 {
            let offset = index.line_to_offset(line_num);
            assert_eq!(
                index.offset_to_line(offset),
                line_num,
                "Boundary condition failed for line {}",
                line_num
            );
        }

        // Test offset beyond file end
        let file_end = mmap.len();
        let last_line = index.total_lines().saturating_sub(1);
        assert_eq!(index.offset_to_line(file_end), last_line); // After last line
        assert_eq!(index.offset_to_line(file_end + 100), last_line);

        // Test invalid line numbers
        let offset_1000 = index.line_to_offset(1000); // Line 1000 exists (empty line)
        let invalid_offset = index.line_to_offset(10000); // Should default to 0
        println!(
            "Line 1000 offset: {}, Total lines: {}",
            offset_1000,
            index.total_lines()
        );
        assert_eq!(invalid_offset, 0);
        // Line 1000 should exist as empty line after the last newline
        if index.total_lines() > 1000 {
            assert!(offset_1000 > 0);
        }
    }

    #[test]
    fn test_line_index_accuracy_with_mixed_line_lengths() {
        let content = "Short\nThis is a much longer line with many words\n\nMedium length line\n";
        let mmap = create_test_mmap(content);
        let index = LineIndex::from_mmap(&mmap);

        assert_eq!(index.total_lines(), 4);

        // Verify each line offset
        assert_eq!(index.line_to_offset(0), 0); // "Short"
        assert_eq!(index.line_to_offset(1), 6); // "This is..."
        assert_eq!(index.line_to_offset(2), 49); // "" (empty line)
        assert_eq!(index.line_to_offset(3), 50); // "Medium..."

        // Test that ranges are accurate
        for line_num in 0..4 {
            let offset = index.line_to_offset(line_num);
            let computed_line = index.offset_to_line(offset);
            assert_eq!(
                computed_line, line_num,
                "Accuracy test failed for line {} with offset {}",
                line_num, offset
            );
        }
    }
}
