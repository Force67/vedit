use std::cmp::Ordering;
use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;
use slab::Slab;
use vedit_make::{Makefile, MakefileError};
use vedit_vs::{Solution, SolutionProject, VcxProject, VisualStudioError};

const SKIPPED_DIRECTORIES: &[&str] = &[".git", "target", "node_modules", ".idea", ".vscode"];

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
            let size = if metadata.is_file() { Some(metadata.len()) } else { None };
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
            size: if metadata.is_file() { Some(metadata.len()) } else { None },
            modified: metadata.modified().ok(),
            is_hidden: path.file_name().and_then(|n| n.to_str()).map(|n| n.starts_with('.')).unwrap_or(false),
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

/// Legacy Node of a workspace file tree. TODO: remove after migration
#[derive(Debug, Clone)]
pub struct FileNode {
    pub name: String,
    pub path: String,
    pub is_directory: bool,
    pub children: Vec<FileNode>,
    pub has_children: bool,
    pub is_fully_scanned: bool,
    pub kind: LegacyNodeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyNodeKind {
    Directory,
    File,
    Solution,
    Project,
    ProjectStub,
}

/// Build a workspace tree for the provided directory.
pub fn build_tree(root: impl AsRef<Path>) -> io::Result<Vec<FileNode>> {
    build_tree_with_ignored(root, &[])
}

pub fn build_tree_with_ignored(
    root: impl AsRef<Path>,
    ignored_directories: &[String],
) -> io::Result<Vec<FileNode>> {
    let normalized: Vec<String> = ignored_directories
        .iter()
        .map(|entry| entry.to_ascii_lowercase())
        .collect();
    collect_directory(root.as_ref(), &normalized)
}

pub fn build_solution_tree(path: impl AsRef<Path>) -> Result<Vec<FileNode>, VisualStudioError> {
    let node = try_build_solution_node(path.as_ref())?;
    Ok(vec![node])
}

fn collect_directory(path: &Path, ignored: &[String]) -> io::Result<Vec<FileNode>> {
    let mut children = Vec::new();

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let entry_path = entry.path();
        let metadata = match fs::metadata(&entry_path) {
            Ok(metadata) => metadata,
            Err(err) => {
                if err.kind() == io::ErrorKind::PermissionDenied {
                    continue;
                } else {
                    return Err(err);
                }
            }
        };

        if metadata.is_dir() {
            if let Some(name) = entry.file_name().to_str() {
                if should_skip(name, ignored) {
                    continue;
                }
            }

            let has_children = match directory_has_visible_children(&entry_path, ignored) {
                Ok(value) => value,
                Err(err) => {
                    if err.kind() == io::ErrorKind::PermissionDenied {
                        continue;
                    } else {
                        return Err(err);
                    }
                }
            };

            children.push(directory_stub(&entry_path, has_children));
            continue;
        }

        let mut handled_special = false;
        if let Some(file_name) = entry_path.file_name().and_then(|name| name.to_str()) {
            if is_makefile_name(file_name) {
                match try_build_makefile_node(&entry_path) {
                    Ok(node) => {
                        children.push(node);
                        handled_special = true;
                    }
                    Err(_) => {}
                }
            }
        }

        if !handled_special {
            if let Some(ext) = entry_path.extension().and_then(|ext| ext.to_str()) {
                if ext.eq_ignore_ascii_case("sln") {
                    match try_build_solution_node(&entry_path) {
                        Ok(node) => {
                            children.push(node);
                            handled_special = true;
                        }
                        Err(_) => {}
                    }
                } else if ext.eq_ignore_ascii_case("vcxproj") {
                    match try_build_vcxproj_node(&entry_path) {
                        Ok(node) => {
                            children.push(node);
                            handled_special = true;
                        }
                        Err(_) => {}
                    }
                } else if ext.eq_ignore_ascii_case("mk") {
                    match try_build_makefile_node(&entry_path) {
                        Ok(node) => {
                            children.push(node);
                            handled_special = true;
                        }
                        Err(_) => {}
                    }
                }
            }
        }

        if handled_special {
            continue;
        }

        children.push(file_node(&entry_path));
    }

    sort_nodes(&mut children);

    Ok(children)
}

fn should_skip(name: &str, ignored: &[String]) -> bool {
    SKIPPED_DIRECTORIES
        .iter()
        .any(|skip| name.eq_ignore_ascii_case(skip))
        || ignored.iter().any(|entry| name.eq_ignore_ascii_case(entry))
}

fn directory_stub(path: &Path, has_children: bool) -> FileNode {
    FileNode {
        name: path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.to_string())
            .unwrap_or_else(|| path.to_string_lossy().to_string()),
        path: path.to_string_lossy().to_string(),
        is_directory: true,
        children: Vec::new(),
        has_children,
        is_fully_scanned: false,
        kind: LegacyNodeKind::Directory,
    }
}

fn file_node(path: &Path) -> FileNode {
    FileNode {
        name: path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.to_string())
            .unwrap_or_else(|| path.to_string_lossy().to_string()),
        path: path.to_string_lossy().to_string(),
        is_directory: false,
        children: Vec::new(),
        has_children: false,
        is_fully_scanned: true,
        kind: LegacyNodeKind::File,
    }
}

fn directory_has_visible_children(path: &Path, ignored: &[String]) -> io::Result<bool> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            if let Some(name) = entry.file_name().to_str() {
                if should_skip(name, ignored) {
                    continue;
                }
            }
        }
        return Ok(true);
    }
    Ok(false)
}

pub fn find_node_mut<'a>(nodes: &'a mut [FileNode], target: &str) -> Option<&'a mut FileNode> {
    for node in nodes {
        if node.path == target {
            return Some(node);
        }

        if node.is_directory {
            if let Some(found) = find_node_mut(&mut node.children, target) {
                return Some(found);
            }
        }
    }

    None
}

pub fn load_directory_children(node: &mut FileNode, ignored: &[String]) -> io::Result<bool> {
    if !node.is_directory || node.is_fully_scanned {
        return Ok(false);
    }

    let path = Path::new(&node.path);
    if !path.is_dir() {
        node.is_fully_scanned = true;
        node.has_children = !node.children.is_empty();
        return Ok(false);
    }

    let children = collect_directory(path, ignored)?;
    node.children = children;
    node.is_fully_scanned = true;
    node.has_children = !node.children.is_empty();
    Ok(true)
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(part) => normalized.push(part),
        }
    }

    normalized
}

fn try_build_solution_node(path: &Path) -> Result<FileNode, VisualStudioError> {
    let solution = Solution::from_path(path)?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_string())
        .unwrap_or_else(|| solution.name.clone());
    let path_string = path.to_string_lossy().to_string();
    let mut children: Vec<FileNode> = solution
        .projects
        .into_iter()
        .map(project_to_node)
        .collect();
    sort_nodes(&mut children);
    let has_children = !children.is_empty();

    Ok(FileNode {
        name,
        path: path_string,
        is_directory: true,
        children,
        has_children,
        is_fully_scanned: true,
        kind: LegacyNodeKind::Solution,
    })
}

fn project_to_node(project: SolutionProject) -> FileNode {
    let absolute_path = normalize_path(project.absolute_path.as_path());
    let path_string = absolute_path.to_string_lossy().to_string();
    let mut name = project.name;

    if let Some(vcx) = project.project {
        let mut children = build_vcxproj_children(&vcx);
        sort_nodes(&mut children);
        let has_children = !children.is_empty();
        FileNode {
            name,
            path: path_string,
            is_directory: true,
            children,
            has_children,
            is_fully_scanned: true,
            kind: LegacyNodeKind::Project,
        }
    } else {
        if project.load_error.is_some() {
            name = format!("{name} (unparsed)");
        }
        FileNode {
            name,
            path: path_string,
            is_directory: false,
            children: Vec::new(),
            has_children: false,
            is_fully_scanned: true,
            kind: LegacyNodeKind::ProjectStub,
        }
    }
}

fn try_build_vcxproj_node(path: &Path) -> Result<FileNode, VisualStudioError> {
    let project = VcxProject::from_path(path)?;
    let mut children = build_vcxproj_children(&project);
    sort_nodes(&mut children);
    let has_children = !children.is_empty();

    Ok(FileNode {
        name: project
            .name
            .clone(),
        path: path.to_string_lossy().to_string(),
        is_directory: true,
        children,
        has_children,
        is_fully_scanned: true,
        kind: LegacyNodeKind::Project,
    })
}

fn try_build_makefile_node(path: &Path) -> Result<FileNode, MakefileError> {
    let makefile = Makefile::from_path(path)?;
    let mut children = build_makefile_children(&makefile);
    sort_nodes(&mut children);
    let has_children = !children.is_empty();

    Ok(FileNode {
        name: makefile.name.clone(),
        path: path.to_string_lossy().to_string(),
        is_directory: true,
        children,
        has_children,
        is_fully_scanned: true,
        kind: LegacyNodeKind::Project,
    })
}

fn build_vcxproj_children(project: &VcxProject) -> Vec<FileNode> {
    build_project_children(
        project
            .files
            .iter()
            .map(|item| (item.include.as_path(), item.full_path.as_path())),
    )
}

fn build_makefile_children(makefile: &Makefile) -> Vec<FileNode> {
    build_project_children(
        makefile
            .files
            .iter()
            .map(|item| (item.include.as_path(), item.full_path.as_path())),
    )
}

fn build_project_children<'a, I>(items: I) -> Vec<FileNode>
where
    I: IntoIterator<Item = (&'a Path, &'a Path)>,
{
    let mut nodes = Vec::new();
    for (include, full_path) in items {
        let segments: Vec<String> = include
            .components()
            .filter_map(|component| component.as_os_str().to_str().map(|s| s.to_string()))
            .collect();
        if segments.is_empty() {
            continue;
        }
        insert_item(&mut nodes, &segments, full_path, segments.len(), 0);
    }
    nodes
}

fn insert_item(
    nodes: &mut Vec<FileNode>,
    segments: &[String],
    full_path: &Path,
    total_segments: usize,
    depth: usize,
) {
    if segments.is_empty() {
        return;
    }

    if segments.len() == 1 {
        let path_string = full_path.to_string_lossy().to_string();
        if nodes.iter().any(|node| !node.is_directory && node.path == path_string) {
            return;
        }
        nodes.push(FileNode {
            name: segments[0].clone(),
            path: path_string,
            is_directory: false,
            children: Vec::new(),
            has_children: false,
            is_fully_scanned: true,
            kind: LegacyNodeKind::File,
        });
        return;
    }

    let directory_name = &segments[0];
    let directory_path = ancestor_for_depth(full_path, total_segments, depth);
    let path_string = directory_path
        .as_ref()
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_else(|| full_path.to_string_lossy().to_string());

    let child = match nodes
        .iter_mut()
        .find(|node| node.is_directory && node.name == *directory_name)
    {
        Some(node) => node,
        None => {
            nodes.push(FileNode {
                name: directory_name.clone(),
                path: path_string,
                is_directory: true,
                children: Vec::new(),
                has_children: false,
                is_fully_scanned: true,
        kind: LegacyNodeKind::Directory,
            });
            nodes.last_mut().unwrap()
        }
    };

    insert_item(&mut child.children, &segments[1..], full_path, total_segments, depth + 1);
    child.has_children = !child.children.is_empty();
}

fn ancestor_for_depth(
    full_path: &Path,
    total_segments: usize,
    depth: usize,
) -> Option<PathBuf> {
    // If depth is beyond or equal to total segments, return None
    if depth >= total_segments {
        return None;
    }

    let mut current = full_path.to_path_buf();
    let mut remove = total_segments.saturating_sub(depth + 1);
    while remove > 0 {
        if let Some(parent) = current.parent() {
            current = parent.to_path_buf();
        } else {
            return None;
        }
        remove -= 1;
    }
    Some(current)
}

fn is_makefile_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower == "makefile" || lower == "gnumakefile" || lower.ends_with(".makefile")
}

fn sort_nodes(nodes: &mut Vec<FileNode>) {
    nodes.sort_by(|a, b| {
        // First sort by node kind priority
        let kind_order = |kind: &LegacyNodeKind| match kind {
            LegacyNodeKind::Directory => 0,
            LegacyNodeKind::Solution => 1,
            LegacyNodeKind::Project => 2,
            LegacyNodeKind::ProjectStub => 3,
            LegacyNodeKind::File => 4,
        };

        let a_kind_order = kind_order(&a.kind);
        let b_kind_order = kind_order(&b.kind);

        if a_kind_order != b_kind_order {
            return a_kind_order.cmp(&b_kind_order);
        }

        // Within same kind, sort directories before files
        match (a.is_directory, b.is_directory) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        }
    });
    for node in nodes.iter_mut() {
        if node.is_directory {
            sort_nodes(&mut node.children);
            if node.is_fully_scanned {
                node.has_children = !node.children.is_empty();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

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
        // Since we can't easily test file operations without actual files,
        // we just test that the provider is created correctly
        // The path is stored internally but we can't access it directly
        // This test mainly ensures the constructor works
        assert_eq!(provider.root, path);
    }

    #[test]
    fn file_node_creation() {
        let node = FileNode {
            name: "test.txt".to_string(),
            path: "/tmp/test.txt".to_string(),
            is_directory: false,
            children: vec![],
            has_children: false,
            is_fully_scanned: true,
            kind: LegacyNodeKind::File,
        };

        assert_eq!(node.name, "test.txt");
        assert_eq!(node.path, "/tmp/test.txt");
        assert!(!node.is_directory);
        assert!(!node.has_children);
        assert!(node.is_fully_scanned);
        assert!(matches!(node.kind, LegacyNodeKind::File));
    }

    #[test]
    fn directory_node_creation() {
        let node = FileNode {
            name: "test_dir".to_string(),
            path: "/tmp/test_dir".to_string(),
            is_directory: true,
            children: vec![],
            has_children: false,
            is_fully_scanned: false,
            kind: LegacyNodeKind::Directory,
        };

        assert_eq!(node.name, "test_dir");
        assert!(node.is_directory);
        assert!(!node.is_fully_scanned);
        assert!(matches!(node.kind, LegacyNodeKind::Directory));
    }

    #[test]
    fn legacy_node_kinds() {
        let kinds = vec![
            LegacyNodeKind::Directory,
            LegacyNodeKind::File,
            LegacyNodeKind::Solution,
            LegacyNodeKind::Project,
            LegacyNodeKind::ProjectStub,
        ];

        for kind in kinds {
            let node = FileNode {
                name: "test".to_string(),
                path: "/tmp/test".to_string(),
                is_directory: matches!(kind, LegacyNodeKind::Directory),
                children: vec![],
                has_children: false,
                is_fully_scanned: true,
                kind: kind.clone(),
            };
            assert!(matches!(node.kind, _kind));
        }
    }

    #[test]
    fn find_node_mut_recursive() {
        let mut nodes = vec![
            FileNode {
                name: "dir1".to_string(),
                path: "/tmp/dir1".to_string(),
                is_directory: true,
                children: vec![
                    FileNode {
                        name: "file1.txt".to_string(),
                        path: "/tmp/dir1/file1.txt".to_string(),
                        is_directory: false,
                        children: vec![],
                        has_children: false,
                        is_fully_scanned: true,
                        kind: LegacyNodeKind::File,
                    },
                ],
                has_children: true,
                is_fully_scanned: true,
                kind: LegacyNodeKind::Directory,
            },
            FileNode {
                name: "file2.txt".to_string(),
                path: "/tmp/file2.txt".to_string(),
                is_directory: false,
                children: vec![],
                has_children: false,
                is_fully_scanned: true,
                kind: LegacyNodeKind::File,
            },
        ];

        // Find nested file
        let found = find_node_mut(&mut nodes, "/tmp/dir1/file1.txt");
        assert!(found.is_some());
        let node = found.unwrap();
        assert_eq!(node.name, "file1.txt");

        // Find top-level file
        let found = find_node_mut(&mut nodes, "/tmp/file2.txt");
        assert!(found.is_some());
        let node = found.unwrap();
        assert_eq!(node.name, "file2.txt");

        // Find non-existent file
        let found = find_node_mut(&mut nodes, "/tmp/nonexistent.txt");
        assert!(found.is_none());
    }

    #[test]
    fn load_directory_children_noop() {
        let mut node = FileNode {
            name: "test.txt".to_string(),
            path: "/tmp/test.txt".to_string(),
            is_directory: false,
            children: vec![],
            has_children: false,
            is_fully_scanned: true,
            kind: LegacyNodeKind::File,
        };

        let ignored = vec![];
        let result = load_directory_children(&mut node, &ignored).unwrap();
        assert!(!result); // Should return false for non-directory

        // Node should be unchanged
        assert!(!node.is_directory);
        assert!(node.is_fully_scanned);
    }

    #[test]
    fn directory_stub_creation() {
        let path = PathBuf::from("/tmp/test_dir");
        let stub = directory_stub(&path, true);

        assert_eq!(stub.name, "test_dir");
        assert_eq!(stub.path, "/tmp/test_dir");
        assert!(stub.is_directory);
        assert!(stub.has_children);
        assert!(!stub.is_fully_scanned);
        assert!(matches!(stub.kind, LegacyNodeKind::Directory));
    }

    #[test]
    fn file_node_creation_from_path() {
        let path = PathBuf::from("/tmp/test.txt");
        let node = file_node(&path);

        assert_eq!(node.name, "test.txt");
        assert_eq!(node.path, "/tmp/test.txt");
        assert!(!node.is_directory);
        assert!(!node.has_children);
        assert!(node.is_fully_scanned);
        assert!(matches!(node.kind, LegacyNodeKind::File));
    }

    #[test]
    fn should_skip_logic() {
        let ignored = vec!["target".to_string(), "build".to_string()];

        // Should skip known directories
        assert!(should_skip(".git", &[]));
        assert!(should_skip("target", &[]));
        assert!(should_skip("node_modules", &[]));

        // Should skip custom ignored
        assert!(should_skip("target", &ignored));
        assert!(should_skip("build", &ignored));

        // Should not skip normal directories
        assert!(!should_skip("src", &[]));
        assert!(!should_skip("docs", &ignored));

        // Case insensitive
        assert!(should_skip(".GIT", &[]));
        assert!(should_skip("Target", &ignored));
    }

    #[test]
    fn is_makefile_name_detection() {
        // Should detect makefiles
        assert!(is_makefile_name("makefile"));
        assert!(is_makefile_name("Makefile"));
        assert!(is_makefile_name("GNUMakefile"));
        assert!(is_makefile_name("test.makefile"));

        // Should not detect other files
        assert!(!is_makefile_name("Makefile.txt"));
        assert!(!is_makefile_name("makefile.sh"));
        assert!(!is_makefile_name("readme"));
    }

    #[test]
    fn normalize_path_functionality() {
        // Test basic normalization
        assert_eq!(
            normalize_path(&PathBuf::from("a/b/../c")),
            PathBuf::from("a/c")
        );

        // Test current directory removal
        assert_eq!(
            normalize_path(&PathBuf::from("a/./b")),
            PathBuf::from("a/b")
        );

        // Test parent directory handling
        assert_eq!(
            normalize_path(&PathBuf::from("a/b/../../c")),
            PathBuf::from("c")
        );

        // Test absolute path preservation
        let abs_path = PathBuf::from("/a/b/../c");
        assert_eq!(normalize_path(&abs_path), PathBuf::from("/a/c"));
    }

    #[test]
    fn ancestor_for_depth_calculation() {
        let path = PathBuf::from("/a/b/c/d/e");

        // Test various depths
        assert_eq!(
            ancestor_for_depth(&path, 5, 0),
            Some(PathBuf::from("/a"))
        );

        assert_eq!(
            ancestor_for_depth(&path, 5, 2),
            Some(PathBuf::from("/a/b/c"))
        );

        assert_eq!(
            ancestor_for_depth(&path, 5, 4),
            Some(PathBuf::from("/a/b/c/d/e"))
        );

        // Depth 5 should return None since depth >= total_segments
        assert_eq!(
            ancestor_for_depth(&path, 5, 5),
            None
        );
    }

    #[test]
    fn sort_nodes_by_priority() {
        let mut nodes = vec![
            FileNode {
                name: "file.txt".to_string(),
                path: "/tmp/file.txt".to_string(),
                is_directory: false,
                children: vec![],
                has_children: false,
                is_fully_scanned: true,
                kind: LegacyNodeKind::File,
            },
            FileNode {
                name: "project.sln".to_string(),
                path: "/tmp/project.sln".to_string(),
                is_directory: true,
                children: vec![],
                has_children: false,
                is_fully_scanned: true,
                kind: LegacyNodeKind::Solution,
            },
            FileNode {
                name: "src".to_string(),
                path: "/tmp/src".to_string(),
                is_directory: true,
                children: vec![],
                has_children: false,
                is_fully_scanned: true,
                kind: LegacyNodeKind::Directory,
            },
        ];

        sort_nodes(&mut nodes);

        // Should be sorted: Directory, Solution, File
        assert!(matches!(nodes[0].kind, LegacyNodeKind::Directory));
        assert!(matches!(nodes[1].kind, LegacyNodeKind::Solution));
        assert!(matches!(nodes[2].kind, LegacyNodeKind::File));

        assert_eq!(nodes[0].name, "src");
        assert_eq!(nodes[1].name, "project.sln");
        assert_eq!(nodes[2].name, "file.txt");
    }

    #[test]
    fn sort_nodes_case_insensitive() {
        let mut nodes = vec![
            FileNode {
                name: "Zfile.txt".to_string(),
                path: "/tmp/Zfile.txt".to_string(),
                is_directory: false,
                children: vec![],
                has_children: false,
                is_fully_scanned: true,
                kind: LegacyNodeKind::File,
            },
            FileNode {
                name: "afile.txt".to_string(),
                path: "/tmp/afile.txt".to_string(),
                is_directory: false,
                children: vec![],
                has_children: false,
                is_fully_scanned: true,
                kind: LegacyNodeKind::File,
            },
            FileNode {
                name: "Mfile.txt".to_string(),
                path: "/tmp/Mfile.txt".to_string(),
                is_directory: false,
                children: vec![],
                has_children: false,
                is_fully_scanned: true,
                kind: LegacyNodeKind::File,
            },
        ];

        sort_nodes(&mut nodes);

        // Should be sorted case-insensitively
        assert_eq!(nodes[0].name, "afile.txt");
        assert_eq!(nodes[1].name, "Mfile.txt");
        assert_eq!(nodes[2].name, "Zfile.txt");
    }

    #[test]
    fn dir_entry_meta_creation() {
        let entry = DirEntryMeta {
            name: "test.txt".to_string(),
            rel_path: "test.txt".to_string(),
            kind: NodeKind::File,
            size: Some(2048),
            modified: Some(std::time::SystemTime::UNIX_EPOCH),
            is_hidden: false,
        };

        assert_eq!(entry.name, "test.txt");
        assert_eq!(entry.rel_path, "test.txt");
        assert!(matches!(entry.kind, NodeKind::File));
        assert_eq!(entry.size, Some(2048));
        assert!(entry.modified.is_some());
        assert!(!entry.is_hidden);
    }

    #[test]
    fn file_meta_creation() {
        let meta = FileMeta {
            size: Some(4096),
            modified: Some(std::time::SystemTime::UNIX_EPOCH),
            is_hidden: true,
        };

        assert_eq!(meta.size, Some(4096));
        assert!(meta.modified.is_some());
        assert!(meta.is_hidden);
    }

    #[test]
    fn insert_item_logic() {
        let mut nodes = vec![];

        // Insert root level item
        let segments = vec!["file.txt".to_string()];
        let path = PathBuf::from("/tmp/file.txt");
        insert_item(&mut nodes, &segments, &path, segments.len(), 0);

        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].name, "file.txt");
        assert!(!nodes[0].is_directory);

        // Insert nested item
        let segments = vec!["src".to_string(), "main.rs".to_string()];
        let path = PathBuf::from("/tmp/src/main.rs");
        insert_item(&mut nodes, &segments, &path, segments.len(), 0);

        assert_eq!(nodes.len(), 2);
        let src_node = nodes.iter().find(|n| n.name == "src").unwrap();
        assert!(src_node.is_directory);
        assert_eq!(src_node.children.len(), 1);
        assert_eq!(src_node.children[0].name, "main.rs");
    }

    #[test]
    fn workspace_state_modification() {
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

        let mut tree = WorkspaceTree {
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

        // Test expansion state
        tree.expanded.insert(root_id);
        assert!(tree.expanded.contains(&root_id));

        // Test selection
        tree.selection.insert(root_id);
        assert!(tree.selection.contains(&root_id));

        // Test cursor
        tree.cursor = None;
        assert!(tree.cursor.is_none());
    }

    #[test]
    fn node_symlink_kind() {
        let symlink_node = Node {
            id: 1,
            name: "link".to_string(),
            rel_path: "link".to_string(),
            kind: NodeKind::Symlink(Box::new(NodeKind::File)),
            size: None,
            modified: None,
            children: None,
            git: None,
            is_hidden: false,
        };

        assert!(matches!(symlink_node.kind, NodeKind::Symlink(target) if matches!(*target, NodeKind::File)));
    }

    #[test]
    fn filter_state_edge_cases() {
        let filter = FilterState {
            query: "".to_string(),
            match_case: false,
            files_only: false,
            folders_only: false,
            show_hidden: false,
        };

        // Empty query should be valid
        assert!(filter.query.is_empty());

        // Mutually exclusive options should both be false for valid state
        assert!(!filter.files_only || !filter.folders_only);
    }

    #[test]
    fn git_status_equality() {
        assert_eq!(GitStatus::Modified, GitStatus::Modified);
        assert_ne!(GitStatus::Modified, GitStatus::Added);

        let status1 = GitStatus::Modified;
        let status2 = GitStatus::Modified;
        assert_eq!(status1, status2);
    }

    #[test]
    fn nodekind_equality() {
        assert_eq!(NodeKind::File, NodeKind::File);
        assert_ne!(NodeKind::File, NodeKind::Folder);

        let symlink_target = Box::new(NodeKind::File);
        let symlink1 = NodeKind::Symlink(symlink_target.clone());
        let symlink2 = NodeKind::Symlink(symlink_target);
        assert_eq!(symlink1, symlink2);
    }

    #[test]
    fn debug_implementations() {
        let node = Node {
            id: 1,
            name: "test".to_string(),
            rel_path: "test".to_string(),
            kind: NodeKind::File,
            size: None,
            modified: None,
            children: None,
            git: None,
            is_hidden: false,
        };

        let debug_str = format!("{:?}", node);
        assert!(debug_str.contains("Node"));
        assert!(debug_str.contains("test"));

        let tree = WorkspaceTree {
            root: 1,
            nodes: slab::Slab::new(),
            expanded: std::collections::HashSet::new(),
            selection: std::collections::BTreeSet::new(),
            cursor: Some(1),
            filter: FilterState {
                query: "".to_string(),
                match_case: false,
                files_only: false,
                folders_only: false,
                show_hidden: false,
            },
        };

        let debug_str = format!("{:?}", tree);
        assert!(debug_str.contains("WorkspaceTree"));
    }

    #[test]
    fn clone_implementations() {
        let node = Node {
            id: 1,
            name: "test".to_string(),
            rel_path: "test".to_string(),
            kind: NodeKind::File,
            size: Some(1024),
            modified: Some(std::time::SystemTime::UNIX_EPOCH),
            children: Some(vec![]),
            git: Some(GitStatus::Modified),
            is_hidden: false,
        };

        let cloned = node.clone();
        assert_eq!(node.id, cloned.id);
        assert_eq!(node.name, cloned.name);
        assert_eq!(node.kind, cloned.kind);
        assert_eq!(node.git, cloned.git);

        let file_node = FileNode {
            name: "test.txt".to_string(),
            path: "/tmp/test.txt".to_string(),
            is_directory: false,
            children: vec![],
            has_children: false,
            is_fully_scanned: true,
            kind: LegacyNodeKind::File,
        };

        let cloned = file_node.clone();
        assert_eq!(file_node.name, cloned.name);
        assert_eq!(file_node.path, cloned.path);
        assert_eq!(file_node.kind, cloned.kind);
    }
}