use std::cmp;

/// In-memory representation of a sticky note anchored to a document position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StickyNote {
    pub id: u64,
    pub line: usize,
    pub column: usize,
    pub content: String,
    pub offset: usize,
}

impl StickyNote {
    pub fn new(id: u64, line: usize, column: usize, content: String, offset: usize) -> Self {
        Self {
            id,
            line,
            column,
            content,
            offset,
        }
    }

    pub fn update(&mut self, line: usize, column: usize, offset: usize) {
        self.line = cmp::max(1, line);
        self.column = cmp::max(1, column);
        self.offset = offset;
    }
}
