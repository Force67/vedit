use std::fmt;
use std::ops::{Bound, RangeBounds};
use std::sync::Arc;

/// Source identifier for a [`Piece`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PieceSource {
    Original,
    Added,
}

/// A contiguous slice of either the original or the append-only buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Piece {
    source: PieceSource,
    start: usize,
    len: usize,
}

impl Piece {
    fn new(source: PieceSource, start: usize, len: usize) -> Self {
        Self { source, start, len }
    }

    fn end(&self) -> usize {
        self.start + self.len
    }

    fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// Text buffer implementation inspired by VS Code's piece table.
///
/// Instead of copying and reallocating the entire document on each edit, the
/// buffer keeps the original text read from disk immutable and stores edits in
/// an append-only buffer. A small sequence of "pieces" references slices of
/// either buffer, allowing inserts and deletes to be expressed as cheap
/// operations on this table.
#[derive(Clone)]
pub struct TextBuffer {
    original: Arc<str>,
    added: String,
    pieces: Vec<Piece>,
    len: usize,
}

impl Default for TextBuffer {
    fn default() -> Self {
        Self {
            original: Arc::<str>::from(""),
            added: String::new(),
            pieces: Vec::new(),
            len: 0,
        }
    }
}



impl TextBuffer {
    /// Creates an empty [`TextBuffer`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a [`TextBuffer`] seeded with the provided text.
    pub fn from_text(text: impl Into<String>) -> Self {
        let string = text.into();
        let len = string.len();
        if len == 0 {
            return Self::new();
        }

        let original: Arc<str> = Arc::from(string.into_boxed_str());
        let mut pieces = Vec::new();
        if len > 0 {
            pieces.push(Piece::new(PieceSource::Original, 0, len));
        }

        Self {
            original,
            added: String::new(),
            pieces,
            len,
        }
    }

    /// Total length of the buffer in bytes.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns true if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the complete contents of the buffer as a fresh [`String`].
    pub fn to_string(&self) -> String {
        self.slice(..)
    }

    /// Returns the amount of `char`s contained in the buffer.
    pub fn char_count(&self) -> usize {
        self.pieces
            .iter()
            .map(|piece| match piece.source {
                PieceSource::Original => self.original[piece.start..piece.end()].chars().count(),
                PieceSource::Added => self.added[piece.start..piece.end()].chars().count(),
            })
            .sum()
    }

    /// Extracts a substring using byte offsets, similar to [`String::get`].
    pub fn slice<R>(&self, range: R) -> String
    where
        R: RangeBounds<usize>,
    {
        let (start, end) = self.normalize_range(range);
        if start >= end {
            return String::new();
        }

        let mut result = String::with_capacity(end - start);
        let mut offset = 0usize;

        for piece in &self.pieces {
            let piece_start = offset;
            let piece_end = offset + piece.len;

            if piece_end <= start {
                offset = piece_end;
                continue;
            }

            if piece_start >= end {
                break;
            }

            let local_start = start.saturating_sub(piece_start);
            let local_end = end.min(piece_end) - piece_start;

            if local_end <= local_start {
                offset = piece_end;
                continue;
            }

            let absolute_start = piece.start + local_start;
            let absolute_end = piece.start + local_end;

            match piece.source {
                PieceSource::Original => {
                    result.push_str(&self.original[absolute_start..absolute_end]);
                }
                PieceSource::Added => {
                    result.push_str(&self.added[absolute_start..absolute_end]);
                }
            }

            offset = piece_end;
        }

        result
    }

    /// Inserts `text` at the provided byte `offset`.
    pub fn insert(&mut self, offset: usize, text: &str) {
        assert!(offset <= self.len, "insert offset out of bounds");
        if text.is_empty() {
            return;
        }

        let insertion_index = self.find_piece_index(offset);
        let added_start = self.added.len();
        self.added.push_str(text);
        let new_piece = Piece::new(PieceSource::Added, added_start, text.len());

        match insertion_index {
            InsertPosition::Empty => {
                self.pieces.push(new_piece);
            }
            InsertPosition::AtEnd => {
                self.pieces.push(new_piece);
                let idx = self.pieces.len() - 1;
                self.merge_neighbors(idx);
            }
            InsertPosition::At(index) => {
                let (piece_index, local_offset) = index;
                if local_offset == 0 {
                    self.pieces.insert(piece_index, new_piece);
                    self.merge_neighbors(piece_index);
                } else {
                    let original_piece = self.pieces[piece_index].clone();
                    if local_offset == original_piece.len {
                        self.pieces.insert(piece_index + 1, new_piece);
                        self.merge_neighbors(piece_index + 1);
                    } else {
                        // Split the existing piece in two halves around the insertion point.
                        self.pieces[piece_index].len = local_offset;
                        let right_piece = Piece::new(
                            original_piece.source,
                            original_piece.start + local_offset,
                            original_piece.len - local_offset,
                        );
                        self.pieces.insert(piece_index + 1, new_piece);
                        self.pieces.insert(piece_index + 2, right_piece);
                        self.merge_neighbors(piece_index + 1);
                    }
                }
            }
        }

        self.len += text.len();
    }

    /// Deletes the text in the provided byte range.
    pub fn delete<R>(&mut self, range: R)
    where
        R: RangeBounds<usize>,
    {
        let (start, end) = self.normalize_range(range);
        if start >= end {
            return;
        }

        assert!(end <= self.len, "delete range out of bounds");

        let mut cursor = 0usize;
        let mut index = 0usize;

        while index < self.pieces.len() {
            let piece_start = cursor;
            let piece = self.pieces[index].clone();
            let piece_len = piece.len;
            let piece_end = piece_start + piece_len;

            if piece_end <= start {
                cursor = piece_end;
                index += 1;
                continue;
            }

            if piece_start >= end {
                break;
            }

            let removal_start = start.max(piece_start);
            let removal_end = end.min(piece_end);
            let removal_len = removal_end - removal_start;

            if removal_len == 0 {
                cursor = piece_end;
                index += 1;
                continue;
            }

            let local_start = removal_start - piece_start;
            let local_end = removal_end - piece_start;

            if local_start == 0 && local_end == piece_len {
                self.pieces.remove(index);
                cursor = piece_start;
                continue;
            }

            if local_start == 0 {
                let new_start = piece.start + removal_len;
                let new_len = piece_len - removal_len;
                self.pieces[index].start = new_start;
                self.pieces[index].len = new_len;

                if self.pieces[index].is_empty() {
                    self.pieces.remove(index);
                    cursor = piece_start;
                } else {
                    cursor = piece_start + new_len;
                    index += 1;
                }

                continue;
            }

            if local_end == piece_len {
                let new_len = local_start;
                self.pieces[index].len = new_len;

                if self.pieces[index].is_empty() {
                    self.pieces.remove(index);
                    cursor = piece_start;
                } else {
                    cursor = piece_start + new_len;
                    index += 1;
                }

                continue;
            }

            // Removal occurs strictly inside the current piece; split into two pieces.
            let right_piece = Piece::new(
                piece.source,
                piece.start + local_end,
                piece_len - local_end,
            );
            self.pieces[index].len = local_start;
            self.pieces.insert(index + 1, right_piece);

            break;
        }

        self.len -= end - start;
        self.coalesce_all();
    }

    /// Replaces the text in `range` with `text`.
    pub fn replace<R>(&mut self, range: R, text: &str)
    where
        R: RangeBounds<usize>,
    {
        let start = match range.start_bound() {
            Bound::Included(&value) => value,
            Bound::Excluded(&value) => value + 1,
            Bound::Unbounded => 0,
        };
        self.delete(range);
        self.insert(start, text);
    }

    fn normalize_range<R>(&self, range: R) -> (usize, usize)
    where
        R: RangeBounds<usize>,
    {
        let start = match range.start_bound() {
            Bound::Included(&value) => value,
            Bound::Excluded(&value) => value + 1,
            Bound::Unbounded => 0,
        };
        let end = match range.end_bound() {
            Bound::Included(&value) => value + 1,
            Bound::Excluded(&value) => value,
            Bound::Unbounded => self.len,
        };
        (start, end)
    }

    fn coalesce_all(&mut self) {
        if self.pieces.len() < 2 {
            return;
        }

        let mut index = 1usize;
        while index < self.pieces.len() {
            if let Some(merged) = Self::try_merge(&self.pieces[index - 1], &self.pieces[index]) {
                self.pieces[index - 1] = merged;
                self.pieces.remove(index);
            } else {
                index += 1;
            }
        }
    }

    fn merge_neighbors(&mut self, index: usize) {
        if self.pieces.is_empty() || index >= self.pieces.len() {
            return;
        }

        // Merge with previous piece if compatible.
        if index > 0 {
            let prev_index = index - 1;
            if let Some(merged) = Self::try_merge(&self.pieces[prev_index], &self.pieces[index]) {
                self.pieces[prev_index] = merged;
                self.pieces.remove(index);
                // Recursively merge newly extended piece with its neighbors.
                self.merge_neighbors(prev_index);
                return;
            }
        }

        // Merge with next piece if compatible.
        if index + 1 < self.pieces.len() {
            if let Some(merged) = Self::try_merge(&self.pieces[index], &self.pieces[index + 1]) {
                self.pieces[index] = merged;
                self.pieces.remove(index + 1);
                self.merge_neighbors(index);
            }
        }
    }

    fn try_merge(left: &Piece, right: &Piece) -> Option<Piece> {
        if left.source == right.source && left.end() == right.start {
            Some(Piece::new(left.source, left.start, left.len + right.len))
        } else {
            None
        }
    }

    fn find_piece_index(&self, offset: usize) -> InsertPosition {
        if self.pieces.is_empty() {
            return InsertPosition::Empty;
        }

        let mut cursor = 0usize;
        for (index, piece) in self.pieces.iter().enumerate() {
            let next = cursor + piece.len;
            if offset < next {
                return InsertPosition::At((index, offset - cursor));
            }
            cursor = next;
        }

        if offset == self.len {
            InsertPosition::AtEnd
        } else {
            InsertPosition::At((self.pieces.len() - 1, self.pieces.last().unwrap().len))
        }
    }
}

enum InsertPosition {
    Empty,
    At((usize, usize)),
    AtEnd,
}

impl From<String> for TextBuffer {
    fn from(value: String) -> Self {
        Self::from_text(value)
    }
}

impl From<&str> for TextBuffer {
    fn from(value: &str) -> Self {
        Self::from_text(value)
    }
}

impl fmt::Debug for TextBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TextBuffer")
            .field("len", &self.len)
            .field("pieces", &self.pieces)
            .finish()
    }
}

impl PartialEq for TextBuffer {
    fn eq(&self, other: &Self) -> bool {
        self.len == other.len && self.to_string() == other.to_string()
    }
}

impl Eq for TextBuffer {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_text_roundtrips() {
        let buffer = TextBuffer::from_text("hello world");
        assert_eq!(buffer.to_string(), "hello world");
        assert_eq!(buffer.len(), "hello world".len());
    }

    #[test]
    fn insert_and_delete_segments() {
        let mut buffer = TextBuffer::from_text("hello world");
        buffer.insert(5, ", brave");
        assert_eq!(buffer.to_string(), "hello, brave world");

        buffer.delete(5..13);
        assert_eq!(buffer.to_string(), "hello world");

        buffer.insert(buffer.len(), "!");
        assert_eq!(buffer.to_string(), "hello world!");
    }

    #[test]
    fn replace_and_slice() {
        let mut buffer = TextBuffer::from_text("lorem ipsum dolor");
        buffer.replace(6..11, "editor");
        assert_eq!(buffer.to_string(), "lorem editor dolor");
        assert_eq!(buffer.slice(0..5), "lorem");
    }

    #[test]
    fn delete_entire_range() {
        let mut buffer = TextBuffer::from_text("temporary");
        buffer.delete(..);
        assert!(buffer.is_empty());
        assert_eq!(buffer.len(), 0);
    }

    #[test]
    fn maintains_unicode_boundaries() {
        let mut buffer = TextBuffer::from_text("😀👍");
        assert_eq!(buffer.char_count(), 2);

        let smile = "😀".len();
        buffer.delete(0..smile);
        assert_eq!(buffer.to_string(), "👍");

        buffer.insert(0, "✨");
        assert_eq!(buffer.to_string(), "✨👍");
        assert_eq!(buffer.char_count(), 2);
    }
}
