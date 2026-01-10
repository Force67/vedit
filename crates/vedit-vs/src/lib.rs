use roxmltree::Document;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

/// Errors that can occur when parsing Visual Studio solutions and projects.
#[derive(Debug, Error)]
pub enum VisualStudioError {
    #[error("I/O error reading {path:?}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("Failed to parse Visual Studio solution entry in {path:?} at line {line}: {message}")]
    SolutionParse {
        path: PathBuf,
        line: usize,
        message: String,
    },
    #[error("Failed to parse XML in {path:?}: {source}")]
    Xml {
        path: PathBuf,
        #[source]
        source: roxmltree::Error,
    },
}

pub type Result<T> = std::result::Result<T, VisualStudioError>;

/// Representation of a Visual Studio solution (.sln) file.
#[derive(Debug, Clone)]
pub struct Solution {
    pub name: String,
    pub path: PathBuf,
    pub projects: Vec<SolutionProject>,
}

/// A project referenced from a Visual Studio solution.
#[derive(Debug, Clone)]
pub struct SolutionProject {
    pub name: String,
    pub relative_path: PathBuf,
    pub absolute_path: PathBuf,
    pub project_type_guid: Option<String>,
    pub project_guid: Option<String>,
    pub project: Option<VcxProject>,
    pub load_error: Option<String>,
}

/// Parsed representation of a Visual Studio C/C++ project (.vcxproj).
#[derive(Debug, Clone)]
pub struct VcxProject {
    pub name: String,
    pub path: PathBuf,
    pub files: Vec<VcxItem>,
    pub produces_executable: bool,
}

/// A file entry inside a Visual Studio C/C++ project.
#[derive(Debug, Clone)]
pub struct VcxItem {
    pub include: PathBuf,
    pub full_path: PathBuf,
    pub kind: VcxItemKind,
}

/// Categorization of file entries from a Visual Studio C/C++ project.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VcxItemKind {
    Source,
    Header,
    Resource,
    Custom,
    None,
    Image,
    Other,
}

impl Solution {
    /// Parse a Visual Studio solution file from disk.
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let contents = fs::read_to_string(path).map_err(|source| VisualStudioError::Io {
            path: path.to_path_buf(),
            source,
        })?;

        let name = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .map(|stem| stem.to_string())
            .unwrap_or_else(|| path.to_string_lossy().to_string());
        let base_dir = path
            .parent()
            .map(normalize_path)
            .unwrap_or_else(|| PathBuf::from("."));

        let mut projects = Vec::new();

        for (line_number, line) in contents.lines().enumerate() {
            let trimmed = line.trim_start();
            if !trimmed.starts_with("Project(") {
                continue;
            }

            let entry = parse_project_line(trimmed).map_err(|message| {
                VisualStudioError::SolutionParse {
                    path: path.to_path_buf(),
                    line: line_number + 1,
                    message,
                }
            })?;

            let normalized_rel = entry.relative_path.replace('\\', "/").trim().to_string();
            let relative_path = PathBuf::from(&normalized_rel);
            let absolute_path = resolve_path(&base_dir, &relative_path);

            let mut project = SolutionProject {
                name: entry.name,
                relative_path,
                absolute_path,
                project_type_guid: entry.project_type_guid,
                project_guid: entry.project_guid,
                project: None,
                load_error: None,
            };

            if project
                .relative_path
                .extension()
                .map(|ext| ext.eq_ignore_ascii_case("vcxproj"))
                == Some(true)
            {
                match VcxProject::from_path(&project.absolute_path) {
                    Ok(vcx) => project.project = Some(vcx),
                    Err(err) => project.load_error = Some(err.to_string()),
                }
            }

            projects.push(project);
        }

        Ok(Solution {
            name,
            path: path.to_path_buf(),
            projects,
        })
    }
}

impl VcxProject {
    /// Parse a Visual Studio C/C++ project file from disk.
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let contents = fs::read_to_string(path).map_err(|source| VisualStudioError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let document = Document::parse(&contents).map_err(|source| VisualStudioError::Xml {
            path: path.to_path_buf(),
            source,
        })?;

        let project_dir = path
            .parent()
            .map(normalize_path)
            .unwrap_or_else(|| PathBuf::from("."));
        let mut files = Vec::new();
        let mut produces_executable = false;

        for node in document.descendants() {
            if !node.is_element() {
                continue;
            }
            let tag_name = node.tag_name().name();
            if tag_name.eq_ignore_ascii_case("ConfigurationType") {
                if node
                    .text()
                    .map(|value| value.trim().eq_ignore_ascii_case("Application"))
                    .unwrap_or(false)
                {
                    produces_executable = true;
                }
            }
            let kind = match VcxItemKind::from_tag(tag_name) {
                Some(kind) => kind,
                None => continue,
            };

            if let Some(include) = node.attribute("Include") {
                if let Some(relative_path) = normalize_include(include) {
                    let full_path = resolve_path(&project_dir, &relative_path);
                    files.push(VcxItem {
                        include: relative_path,
                        full_path,
                        kind,
                    });
                }
            }
        }

        files.sort_by(|a, b| a.include.cmp(&b.include));
        files.dedup_by(|a, b| a.include == b.include);

        Ok(VcxProject {
            name: path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .map(|stem| stem.to_string())
                .unwrap_or_else(|| path.to_string_lossy().to_string()),
            path: normalize_path(path),
            files,
            produces_executable,
        })
    }
}

impl VcxItemKind {
    fn from_tag(tag: &str) -> Option<Self> {
        Some(match tag {
            "ClCompile" => VcxItemKind::Source,
            "ClInclude" => VcxItemKind::Header,
            "ResourceCompile" => VcxItemKind::Resource,
            "CustomBuild" => VcxItemKind::Custom,
            "None" => VcxItemKind::None,
            "Image" => VcxItemKind::Image,
            "Text" => VcxItemKind::Other,
            "Natvis" => VcxItemKind::Other,
            _ => return None,
        })
    }
}

struct ProjectLine {
    name: String,
    relative_path: String,
    project_type_guid: Option<String>,
    project_guid: Option<String>,
}

fn parse_project_line(line: &str) -> std::result::Result<ProjectLine, String> {
    let rest = line
        .strip_prefix("Project(")
        .ok_or_else(|| "Missing Project prefix".to_string())?;
    let (type_guid_raw, remainder) = rest
        .split_once(')')
        .ok_or_else(|| "Missing closing ')' for project type".to_string())?;
    let after_guid = remainder.trim_start();
    let values = after_guid
        .strip_prefix('=')
        .ok_or_else(|| "Missing '=' after project type".to_string())?
        .trim();

    let mut parts = values.split(',');
    let name_part = parts
        .next()
        .ok_or_else(|| "Missing project name".to_string())?
        .trim();
    let path_part = parts
        .next()
        .ok_or_else(|| "Missing project path".to_string())?
        .trim();
    let guid_part = parts
        .next()
        .ok_or_else(|| "Missing project GUID".to_string())?
        .trim();

    let name = trim_quotes(name_part)?;
    let relative_path = trim_quotes(path_part)?;
    let project_guid = trim_guid(guid_part)?;
    let project_type_guid = trim_guid(type_guid_raw.trim())?;

    Ok(ProjectLine {
        name,
        relative_path,
        project_type_guid,
        project_guid,
    })
}

fn trim_quotes(value: &str) -> std::result::Result<String, String> {
    let trimmed = value.trim();
    if let Some(stripped) = trimmed.strip_prefix('"').and_then(|v| v.strip_suffix('"')) {
        Ok(stripped.to_string())
    } else {
        Err(format!("Expected quoted string, found: {value}"))
    }
}

fn trim_guid(value: &str) -> std::result::Result<Option<String>, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let stripped = if let Some(inner) = trimmed.strip_prefix('"').and_then(|v| v.strip_suffix('"'))
    {
        inner
    } else {
        trimmed
    };
    let stripped = stripped
        .strip_prefix('{')
        .and_then(|v| v.strip_suffix('}'))
        .unwrap_or(stripped);
    let normalized = stripped.trim();
    if normalized.is_empty() {
        Ok(None)
    } else {
        Ok(Some(normalized.to_string()))
    }
}

fn normalize_include(value: &str) -> Option<PathBuf> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.contains("$(") || trimmed.contains("%(") {
        return None;
    }
    let normalized = trimmed.replace('\\', "/");
    Some(PathBuf::from(normalized))
}

fn resolve_path(base: &Path, relative: &Path) -> PathBuf {
    if relative
        .components()
        .next()
        .map(|comp| matches!(comp, Component::Prefix(_)))
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
                normalized.pop();
            }
            Component::Normal(part) => normalized.push(part),
        }
    }

    normalized
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn parse_solution_with_vcxproj() {
        let dir = tempdir().unwrap();
        let solution_path = dir.path().join("sample.sln");
        let project_path = dir.path().join("sample.vcxproj");

        fs::write(
            &project_path,
            r#"<?xml version="1.0" encoding="utf-8"?>
<Project DefaultTargets="Build" xmlns="http://schemas.microsoft.com/developer/msbuild/2003">
  <ItemGroup>
    <ClCompile Include="src\main.cpp" />
    <ClInclude Include="include\main.h" />
  </ItemGroup>
</Project>
"#,
        )
        .unwrap();

        fs::write(
            &solution_path,
            format!(
                "Project(\"{{8BC9CEB8-8B4A-11D0-8D11-00A0C91BC942}}\") = \"sample\", \"sample.vcxproj\", \"{{11111111-2222-3333-4444-555555555555}}\"\nEndProject\n"
            ),
        )
        .unwrap();

        let solution = Solution::from_path(&solution_path).unwrap();
        assert_eq!(solution.projects.len(), 1);
        let project = &solution.projects[0];
        assert!(project.project.is_some());
        let files = &project.project.as_ref().unwrap().files;
        assert_eq!(files.len(), 2);
        assert!(
            files
                .iter()
                .any(|item| item.include.to_string_lossy() == "src/main.cpp")
        );
    }
}
