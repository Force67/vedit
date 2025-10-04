use std::fs;
use std::io;
use std::path::Path;

/// Represents an open file in the editor workspace.
#[derive(Debug, Clone)]
pub struct Document {
    pub path: Option<String>,
    pub buffer: String,
    pub is_modified: bool,
}

impl Document {
    pub fn new(path: Option<String>, buffer: String) -> Self {
        Self {
            path,
            buffer,
            is_modified: false,
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
}

impl Default for Document {
    fn default() -> Self {
        Self::new(None, String::new())
    }
}
