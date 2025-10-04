use std::cmp::Ordering;
use std::fs;
use std::io;
use std::path::Path;

const SKIPPED_DIRECTORIES: &[&str] = &[".git", "target", "node_modules", ".idea", ".vscode"];

/// Node of a workspace file tree.
#[derive(Debug, Clone)]
pub struct FileNode {
    pub name: String,
    pub path: String,
    pub is_directory: bool,
    pub children: Vec<FileNode>,
}

impl FileNode {
    fn from_path(path: &Path) -> io::Result<Self> {
        let metadata = fs::metadata(path)?;
        let is_directory = metadata.is_dir();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.to_string())
            .unwrap_or_else(|| path.to_string_lossy().to_string());
        let path_string = path.to_string_lossy().to_string();

        let children = if is_directory {
            collect_directory(path)?
        } else {
            Vec::new()
        };

        Ok(Self {
            name,
            path: path_string,
            is_directory,
            children,
        })
    }
}

/// Build a workspace tree for the provided directory.
pub fn build_tree(root: impl AsRef<Path>) -> io::Result<Vec<FileNode>> {
    collect_directory(root.as_ref())
}

fn collect_directory(path: &Path) -> io::Result<Vec<FileNode>> {
    let mut children = Vec::new();

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let entry_path = entry.path();

        if entry.file_type()?.is_dir() {
            if let Some(name) = entry.file_name().to_str() {
                if SKIPPED_DIRECTORIES
                    .iter()
                    .any(|skip| name.eq_ignore_ascii_case(skip))
                {
                    continue;
                }
            }
        }

        match FileNode::from_path(&entry_path) {
            Ok(node) => children.push(node),
            Err(err) => {
                if err.kind() == io::ErrorKind::PermissionDenied {
                    continue;
                } else {
                    return Err(err);
                }
            }
        }
    }

    children.sort_by(|a, b| match (a.is_directory, b.is_directory) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });

    Ok(children)
}
