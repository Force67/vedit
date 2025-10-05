use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

/// Errors that can occur when parsing Makefiles.
#[derive(Debug, Error)]
pub enum MakefileError {
    #[error("I/O error reading {path:?}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

pub type Result<T> = std::result::Result<T, MakefileError>;

/// Parsed representation of a Makefile.
#[derive(Debug, Clone)]
pub struct Makefile {
    pub name: String,
    pub path: PathBuf,
    pub files: Vec<MakefileItem>,
}

/// A referenced file within a Makefile.
#[derive(Debug, Clone)]
pub struct MakefileItem {
    pub include: PathBuf,
    pub full_path: PathBuf,
}

impl Makefile {
    /// Parse a Makefile from disk.
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let contents = fs::read_to_string(path).map_err(|source| MakefileError::Io {
            path: path.to_path_buf(),
            source,
        })?;

        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.to_string())
            .unwrap_or_else(|| path.to_string_lossy().to_string());
        let base_dir = path
            .parent()
            .map(normalize_path)
            .unwrap_or_else(|| PathBuf::from("."));

        let mut files = Vec::new();
        let mut seen = BTreeSet::new();

        for token in extract_references(&contents) {
            let include = PathBuf::from(&token);
            if !seen.insert(include.clone()) {
                continue;
            }

            let full_path = resolve_path(&base_dir, &include);
            match fs::metadata(&full_path) {
                Ok(metadata) => {
                    if metadata.is_file() {
                        files.push(MakefileItem {
                            include,
                            full_path: normalize_path(&full_path),
                        });
                    }
                }
                Err(err) => {
                    if err.kind() != io::ErrorKind::NotFound {
                        return Err(MakefileError::Io {
                            path: full_path,
                            source: err,
                        });
                    }
                }
            }
        }

        files.sort_by(|a, b| a.include.cmp(&b.include));

        Ok(Makefile {
            name,
            path: normalize_path(path),
            files,
        })
    }
}

fn extract_references(contents: &str) -> Vec<String> {
    let mut references = Vec::new();

    for line in logical_lines(contents) {
        let stripped = strip_comment(&line);
        if stripped.trim().is_empty() {
            continue;
        }

        if stripped.starts_with('\t') {
            continue;
        }

        let trimmed = stripped.trim();

        if let Some(rest) = directive_arguments(trimmed) {
            for token in rest.split_whitespace() {
                if let Some(clean) = sanitize_token(token) {
                    references.push(clean);
                }
            }
            continue;
        }

        if let Some(rest) = split_after_separator(trimmed) {
            for token in rest.split_whitespace() {
                if let Some(clean) = sanitize_token(token) {
                    references.push(clean);
                }
            }
        }
    }

    references
}

fn logical_lines(contents: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();

    for raw_line in contents.lines() {
        let mut line = raw_line.trim_end_matches('\r');
        let mut continued = false;

        if line.trim_end().ends_with('\\') {
            continued = true;
            line = line
                .trim_end()
                .trim_end_matches('\\')
                .trim_end_matches(char::is_whitespace);
        }

        if current.is_empty() {
            current.push_str(line);
        } else {
            current.push(' ');
            current.push_str(line.trim_start());
        }

        if !continued {
            if !current.is_empty() {
                lines.push(current.clone());
                current.clear();
            }
        }
    }

    if !current.trim().is_empty() {
        lines.push(current);
    }

    lines
}

fn strip_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'#' {
            if i == 0 || bytes[i - 1] != b'\\' {
                return &line[..i];
            }
        }
        i += 1;
    }
    line
}

fn directive_arguments(line: &str) -> Option<&str> {
    const DIRECTIVES: [&str; 3] = ["include", "-include", "sinclude"];

    for directive in DIRECTIVES.iter() {
        if let Some(rest) = line.strip_prefix(directive) {
            if rest.chars().next().map_or(false, char::is_whitespace) {
                return Some(rest.trim_start());
            }
        }
    }

    None
}

fn split_after_separator(line: &str) -> Option<&str> {
    let mut chars = line.char_indices();
    while let Some((idx, ch)) = chars.next() {
        match ch {
            ':' => {
                let mut offset = 1;
                if line[idx + 1..].starts_with(':') {
                    offset += 1;
                }
                return Some(line[idx + offset..].trim_start());
            }
            '=' => {
                return Some(line[idx + 1..].trim_start());
            }
            _ => {}
        }
    }
    None
}

fn sanitize_token(token: &str) -> Option<String> {
    let trimmed = token.trim_matches(|ch: char| matches!(ch, ';' | ',' | '\r' | '\n'));
    if trimmed.is_empty() {
        return None;
    }
    if matches!(trimmed.as_bytes().first(), Some(b'-' | b'@' | b'+')) {
        return None;
    }
    if trimmed
        .chars()
        .any(|ch| matches!(ch, '$' | '%' | '*' | '?' | '(' | ')' | '{' | '}'))
    {
        return None;
    }
    let unquoted = trimmed
        .trim_matches('"')
        .trim_matches('\'')
        .trim();
    if unquoted.is_empty() {
        return None;
    }

    let normalized = unquoted.replace('\\', "/").trim().to_string();
    if normalized.is_empty() {
        return None;
    }

    Some(normalized)
}

fn resolve_path(base: &Path, relative: &Path) -> PathBuf {
    if relative
        .components()
        .next()
        .map(|component| matches!(component, Component::Prefix(_)))
        .unwrap_or(false)
    {
        return normalize_path(relative);
    }

    if relative.is_absolute() {
        normalize_path(relative)
    } else {
        normalize_path(&base.join(relative))
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    normalized.push("..");
                }
            }
            Component::Normal(part) => normalized.push(part),
        }
    }

    normalized
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn parse_simple_makefile() {
        let dir = tempdir().unwrap();
        let dir_path = dir.path();
        fs::create_dir_all(dir_path.join("src")).unwrap();
        fs::create_dir_all(dir_path.join("include")).unwrap();
        fs::write(dir_path.join("src/main.c"), "int main() { return 0; }\n").unwrap();
        fs::write(dir_path.join("src/util.c"), "void util() {}\n").unwrap();
        fs::write(dir_path.join("include/util.h"), "void util();\n").unwrap();
        fs::write(dir_path.join("config.mk"), "# config\n").unwrap();

        let makefile_path = dir_path.join("Makefile");
        let mut makefile = fs::File::create(&makefile_path).unwrap();
        writeln!(
            makefile,
            "SOURCES = src/main.c \\\n+ src/util.c\n"
        )
        .unwrap();
        writeln!(makefile, "HEADERS := include/util.h").unwrap();
        writeln!(makefile, "include config.mk").unwrap();
        writeln!(makefile, "app: $(SOURCES) $(HEADERS) extra.o").unwrap();

        drop(makefile);

        let parsed = Makefile::from_path(&makefile_path).unwrap();
        assert_eq!(parsed.name, "Makefile");
        let includes: BTreeSet<_> = parsed
            .files
            .iter()
            .map(|item| item.include.to_string_lossy().to_string())
            .collect();
        assert_eq!(includes.len(), 4);
        assert!(includes.contains("src/main.c"));
        assert!(includes.contains("src/util.c"));
        assert!(includes.contains("include/util.h"));
        assert!(includes.contains("config.mk"));
    }
}
