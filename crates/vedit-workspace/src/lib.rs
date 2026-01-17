use slab::Slab;
use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub type NodeId = usize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeKind {
    File,
    Folder,
    Symlink(Box<NodeKind>),
}

#[derive(Debug, Clone)]
pub struct Node {
    pub id: NodeId,
    pub name: String,
    pub rel_path: String,
    pub kind: NodeKind,
    pub size: Option<u64>,
    pub modified: Option<SystemTime>,
    pub children: Option<Vec<NodeId>>,
    pub git: Option<GitStatus>,
    pub is_hidden: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitStatus {
    Added,
    Modified,
    Deleted,
    Unmerged,
    Untracked,
    Ignored,
}

#[derive(Debug, Clone)]
pub struct FilterState {
    pub query: String,
    pub match_case: bool,
    pub files_only: bool,
    pub folders_only: bool,
    pub show_hidden: bool,
}

#[derive(Debug, Clone)]
pub struct WorkspaceTree {
    pub root: NodeId,
    pub nodes: Slab<Node>,
    pub expanded: HashSet<NodeId>,
    pub selection: BTreeSet<NodeId>,
    pub cursor: Option<NodeId>,
    pub filter: FilterState,
}

pub trait WorkspaceProvider {
    fn read_dir(&self, rel: &str) -> io::Result<Vec<DirEntryMeta>>;
    fn read_meta(&self, rel: &str) -> io::Result<FileMeta>;
    fn is_dir(&self, rel: &str) -> bool;
    fn rename(&mut self, from: &str, to: &str) -> io::Result<()>;
    fn create_file(&mut self, rel: &str) -> io::Result<()>;
    fn create_dir(&mut self, rel: &str) -> io::Result<()>;
    fn remove(&mut self, rel: &str) -> io::Result<()>;
}

pub struct FsWorkspaceProvider {
    root: PathBuf,
}

impl FsWorkspaceProvider {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
}

impl FsWorkspaceProvider {
    pub fn load_children(&self, tree: &mut WorkspaceTree, id: NodeId) -> io::Result<()> {
        let rel_path = if let Some(node) = tree.nodes.get(id) {
            if node.children.is_none() {
                node.rel_path.clone()
            } else {
                return Ok(());
            }
        } else {
            return Ok(());
        };

        let entries = self.read_dir(&rel_path)?;
        let mut children = Vec::new();
        for entry in entries {
            let child_id = tree.nodes.insert(Node {
                id: 0, // will be set
                name: entry.name,
                rel_path: entry.rel_path,
                kind: entry.kind,
                size: entry.size,
                modified: entry.modified,
                children: None,
                git: None,
                is_hidden: entry.is_hidden,
            });
            tree.nodes[child_id].id = child_id;
            children.push(child_id);
        }

        if let Some(node) = tree.nodes.get_mut(id) {
            node.children = Some(children);
        }
        Ok(())
    }
}

impl WorkspaceProvider for FsWorkspaceProvider {
    fn read_dir(&self, rel: &str) -> io::Result<Vec<DirEntryMeta>> {
        let path = self.root.join(rel);
        let mut entries = Vec::new();
        for entry in fs::read_dir(&path)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            let rel_path = Path::new(rel).join(&name).to_string_lossy().to_string();
            let metadata = entry.metadata()?;
            let kind = if metadata.is_dir() {
                NodeKind::Folder
            } else if metadata.is_file() {
                NodeKind::File
            } else {
                // Handle symlinks, but for now treat as file
                NodeKind::File
            };
            let size = if metadata.is_file() {
                Some(metadata.len())
            } else {
                None
            };
            let modified = metadata.modified().ok();
            let is_hidden = name.starts_with('.');
            entries.push(DirEntryMeta {
                name,
                rel_path,
                kind,
                size,
                modified,
                is_hidden,
            });
        }
        entries.sort_by(|a, b| {
            let a_is_dir = matches!(a.kind, NodeKind::Folder);
            let b_is_dir = matches!(b.kind, NodeKind::Folder);
            if a_is_dir && !b_is_dir {
                std::cmp::Ordering::Less
            } else if !a_is_dir && b_is_dir {
                std::cmp::Ordering::Greater
            } else {
                a.name.cmp(&b.name)
            }
        });
        Ok(entries)
    }

    fn read_meta(&self, rel: &str) -> io::Result<FileMeta> {
        let path = self.root.join(rel);
        let metadata = fs::metadata(&path)?;
        Ok(FileMeta {
            size: if metadata.is_file() {
                Some(metadata.len())
            } else {
                None
            },
            modified: metadata.modified().ok(),
            is_hidden: path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with('.'))
                .unwrap_or(false),
        })
    }

    fn is_dir(&self, rel: &str) -> bool {
        self.root.join(rel).is_dir()
    }

    fn rename(&mut self, from: &str, to: &str) -> io::Result<()> {
        let from_path = self.root.join(from);
        let to_path = self.root.join(to);
        fs::rename(from_path, to_path)
    }

    fn create_file(&mut self, rel: &str) -> io::Result<()> {
        let path = self.root.join(rel);
        fs::File::create(path)?;
        Ok(())
    }

    fn create_dir(&mut self, rel: &str) -> io::Result<()> {
        let path = self.root.join(rel);
        fs::create_dir_all(path)
    }

    fn remove(&mut self, rel: &str) -> io::Result<()> {
        let path = self.root.join(rel);
        if path.is_dir() {
            fs::remove_dir_all(path)
        } else {
            fs::remove_file(path)
        }
    }
}

#[derive(Debug, Clone)]
pub struct DirEntryMeta {
    pub name: String,
    pub rel_path: String,
    pub kind: NodeKind,
    pub size: Option<u64>,
    pub modified: Option<SystemTime>,
    pub is_hidden: bool,
}

#[derive(Debug, Clone)]
pub struct FileMeta {
    pub size: Option<u64>,
    pub modified: Option<SystemTime>,
    pub is_hidden: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn node_creation_and_properties() {
        let node = Node {
            id: 1,
            name: "test.txt".to_string(),
            rel_path: "test.txt".to_string(),
            kind: NodeKind::File,
            size: Some(1024),
            modified: None,
            children: None,
            git: Some(GitStatus::Modified),
            is_hidden: false,
        };

        assert_eq!(node.id, 1);
        assert_eq!(node.name, "test.txt");
        assert_eq!(node.rel_path, "test.txt");
        assert!(matches!(node.kind, NodeKind::File));
        assert_eq!(node.size, Some(1024));
        assert!(matches!(node.git, Some(GitStatus::Modified)));
        assert!(!node.is_hidden);
    }

    #[test]
    fn node_kinds() {
        let file_node = Node {
            id: 1,
            name: "file.txt".to_string(),
            rel_path: "file.txt".to_string(),
            kind: NodeKind::File,
            size: None,
            modified: None,
            children: None,
            git: None,
            is_hidden: false,
        };

        let folder_node = Node {
            id: 2,
            name: "folder".to_string(),
            rel_path: "folder".to_string(),
            kind: NodeKind::Folder,
            size: None,
            modified: None,
            children: None,
            git: None,
            is_hidden: false,
        };

        assert!(matches!(file_node.kind, NodeKind::File));
        assert!(matches!(folder_node.kind, NodeKind::Folder));
    }

    #[test]
    fn git_status_variants() {
        let statuses = vec![
            GitStatus::Added,
            GitStatus::Modified,
            GitStatus::Deleted,
            GitStatus::Unmerged,
            GitStatus::Untracked,
            GitStatus::Ignored,
        ];

        for status in statuses {
            let node = Node {
                id: 1,
                name: "test.txt".to_string(),
                rel_path: "test.txt".to_string(),
                kind: NodeKind::File,
                size: None,
                modified: None,
                children: None,
                git: Some(status.clone()),
                is_hidden: false,
            };
            assert!(matches!(node.git, Some(s) if s == status));
        }
    }

    #[test]
    fn filter_state_creation() {
        let filter = FilterState {
            query: "test".to_string(),
            match_case: true,
            files_only: false,
            folders_only: false,
            show_hidden: true,
        };

        assert_eq!(filter.query, "test");
        assert!(filter.match_case);
        assert!(!filter.files_only);
        assert!(!filter.folders_only);
        assert!(filter.show_hidden);
    }

    #[test]
    fn workspace_tree_creation() {
        let mut nodes = slab::Slab::new();
        let root_id = nodes.insert(Node {
            id: 0,
            name: "root".to_string(),
            rel_path: "".to_string(),
            kind: NodeKind::Folder,
            size: None,
            modified: None,
            children: Some(vec![]),
            git: None,
            is_hidden: false,
        });

        let tree = WorkspaceTree {
            root: root_id,
            nodes,
            expanded: std::collections::HashSet::new(),
            selection: std::collections::BTreeSet::new(),
            cursor: Some(root_id),
            filter: FilterState {
                query: "".to_string(),
                match_case: false,
                files_only: false,
                folders_only: false,
                show_hidden: false,
            },
        };

        assert_eq!(tree.root, root_id);
        assert_eq!(tree.cursor, Some(root_id));
        assert!(tree.expanded.is_empty());
        assert!(tree.selection.is_empty());
    }

    #[test]
    fn fs_workspace_provider_creation() {
        let path = PathBuf::from("/tmp");
        let provider = FsWorkspaceProvider::new(path.clone());
        assert_eq!(provider.root, path);
    }
}
