use crate::language::Language;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io;
use std::path::{Path, PathBuf};

/// Represents an open file in the editor workspace.
#[derive(Debug, Clone)]
pub struct Document {
    pub path: Option<String>,
    pub buffer: String,
    pub is_modified: bool,
    pub fingerprint: Option<u64>,
}

impl Document {
    pub fn new(path: Option<String>, buffer: String) -> Self {
        let fingerprint = path.as_ref().map(|path| compute_fingerprint(path));
        Self {
            path,
            buffer,
            is_modified: false,
            fingerprint,
        }
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
}

impl Default for Document {
    fn default() -> Self {
        Self::new(None, String::new())
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
}
