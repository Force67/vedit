use crate::language::Language;
use crate::sticky::StickyNote;
use crate::text_buffer::TextBuffer;
use std::cmp;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io;
use std::path::{Path, PathBuf};
use vedit_config::StickyNoteRecord;

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
}
