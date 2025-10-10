use std::collections::HashMap;
use std::ops::Range;
use memmap2::Mmap;

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