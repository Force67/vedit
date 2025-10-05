use std::cmp::Ordering;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use vedit_make::{Makefile, MakefileError};
use vedit_vs::{Solution, SolutionProject, VcxProject, VisualStudioError};

const SKIPPED_DIRECTORIES: &[&str] = &[".git", "target", "node_modules", ".idea", ".vscode"];

/// Node of a workspace file tree.
#[derive(Debug, Clone)]
pub struct FileNode {
    pub name: String,
    pub path: String,
    pub is_directory: bool,
    pub children: Vec<FileNode>,
    pub has_children: bool,
    pub is_fully_scanned: bool,
    pub kind: NodeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
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
        kind: NodeKind::Directory,
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
        kind: NodeKind::File,
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
        kind: NodeKind::Solution,
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
            kind: NodeKind::Project,
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
            kind: NodeKind::ProjectStub,
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
        kind: NodeKind::Project,
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
        kind: NodeKind::Project,
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
            kind: NodeKind::File,
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
                kind: NodeKind::Directory,
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
    nodes.sort_by(|a, b| match (a.is_directory, b.is_directory) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
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
