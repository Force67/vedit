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

    pub fn language(&self) -> &'static str {
        self.path
            .as_deref()
            .map(detect_language_from_path)
            .unwrap_or("Plain Text")
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

fn detect_language_from_path(path: &str) -> &'static str {
    let path = Path::new(path);

    if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
        let lower = name.to_ascii_lowercase();
        match lower.as_str() {
            "makefile" => return "Makefile",
            "dockerfile" => return "Dockerfile",
            "cmakelists.txt" => return "CMake",
            _ => {}
        }
    }

    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
    {
        Some(ext) => match ext.as_str() {
            "rs" => "Rust",
            "c" => "C",
            "h" => "C Header",
            "hh" | "hpp" | "hxx" | "h++" => "C++ Header",
            "cpp" | "cc" | "cxx" | "c++" => "C++",
            "m" => "Objective-C",
            "mm" => "Objective-C++",
            "swift" => "Swift",
            "java" => "Java",
            "kt" | "kts" => "Kotlin",
            "cs" => "C#",
            "go" => "Go",
            "py" => "Python",
            "rb" => "Ruby",
            "php" => "PHP",
            "hs" => "Haskell",
            "erl" | "hrl" => "Erlang",
            "ex" | "exs" => "Elixir",
            "js" => "JavaScript",
            "jsx" => "JavaScript JSX",
            "ts" => "TypeScript",
            "tsx" => "TypeScript JSX",
            "json" => "JSON",
            "toml" => "TOML",
            "yaml" | "yml" => "YAML",
            "ini" => "INI",
            "md" | "markdown" => "Markdown",
            "sql" => "SQL",
            "html" | "htm" => "HTML",
            "css" => "CSS",
            "scss" | "sass" => "SCSS",
            "less" => "Less",
            "lua" => "Lua",
            "zig" => "Zig",
            "dart" => "Dart",
            "scala" => "Scala",
            "sh" | "bash" => "Shell",
            "fish" => "Fish",
            "ps1" => "PowerShell",
            "bat" => "Batch",
            "vue" => "Vue",
            "svelte" => "Svelte",
            _ => "Plain Text",
        },
        None => "Plain Text",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_language_from_extension() {
        let doc = Document::new(Some("/tmp/test.rs".into()), String::new());
        assert_eq!(doc.language(), "Rust");
    }

    #[test]
    fn detects_language_from_special_file_name() {
        let doc = Document::new(Some("/tmp/Makefile".into()), String::new());
        assert_eq!(doc.language(), "Makefile");
    }

    #[test]
    fn defaults_to_plain_text_without_path() {
        let doc = Document::default();
        assert_eq!(doc.language(), "Plain Text");
    }
}
