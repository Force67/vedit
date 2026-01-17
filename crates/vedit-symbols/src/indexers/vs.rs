//! Visual Studio solution indexer
//!
//! This module provides indexing support for Visual Studio solutions (.sln) and
//! C++ projects (.vcxproj).

use crate::indexers::ProjectIndexer;
use crate::{Result, SymbolError, SymbolIndex};
use std::path::{Path, PathBuf};
use vedit_vs::{Solution, VcxItemKind};

/// Indexer for Visual Studio solutions
///
/// This indexer extracts source files and include directories from VS solutions
/// and their associated .vcxproj files.
pub struct VsSolutionIndexer {
    /// Path to the .sln file
    solution_path: PathBuf,
    /// Parsed solution
    solution: Solution,
    /// Collected include directories
    include_dirs: Vec<PathBuf>,
    /// Collected header files to index
    header_files: Vec<PathBuf>,
}

impl VsSolutionIndexer {
    /// Create a new indexer from a solution file path
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        let solution_path = path.as_ref().to_path_buf();
        let solution = Solution::from_path(&solution_path)
            .map_err(|e| SymbolError::ProjectError(format!("Failed to parse solution: {}", e)))?;

        let mut indexer = Self {
            solution_path,
            solution,
            include_dirs: Vec::new(),
            header_files: Vec::new(),
        };

        indexer.collect_project_info();
        Ok(indexer)
    }

    /// Collect include directories and header files from all projects
    fn collect_project_info(&mut self) {
        for project in &self.solution.projects {
            if let Some(ref vcx) = project.project {
                // Collect include directories
                for dir in vcx.all_include_dirs() {
                    let abs_path = if Path::new(dir).is_absolute() {
                        PathBuf::from(dir)
                    } else {
                        vcx.path.parent().unwrap_or(Path::new(".")).join(dir)
                    };
                    if abs_path.exists() && !self.include_dirs.contains(&abs_path) {
                        self.include_dirs.push(abs_path);
                    }
                }

                // Collect header files
                for item in &vcx.files {
                    if item.kind == VcxItemKind::Header && item.full_path.exists() {
                        if !self.header_files.contains(&item.full_path) {
                            self.header_files.push(item.full_path.clone());
                        }
                    }
                }
            }
        }
    }

    /// Get the number of projects in the solution
    pub fn project_count(&self) -> usize {
        self.solution.projects.len()
    }

    /// Get preprocessor definitions from all projects
    pub fn preprocessor_definitions(&self) -> Vec<String> {
        let mut defs = Vec::new();
        for project in &self.solution.projects {
            if let Some(ref vcx) = project.project {
                for def in vcx.all_preprocessor_definitions() {
                    let def_str = def.to_string();
                    if !defs.contains(&def_str) {
                        defs.push(def_str);
                    }
                }
            }
        }
        defs
    }

    /// Get the solution name
    pub fn solution_name(&self) -> &str {
        &self.solution.name
    }
}

impl ProjectIndexer for VsSolutionIndexer {
    fn index(&self, index: &mut SymbolIndex) -> Result<usize> {
        let mut indexed_count = 0;

        // Add include directories to the index
        for dir in &self.include_dirs {
            index.add_include_dir(dir.clone());
        }

        // Index all header files
        for header_path in &self.header_files {
            if index.needs_reindex(header_path) {
                match std::fs::read_to_string(header_path) {
                    Ok(content) => {
                        if let Err(e) = index.index_file(header_path, &content) {
                            // Only log actual errors, not skipped files
                            if !e.to_string().contains("Skipping") {
                                eprintln!(
                                    "Warning: Failed to index {}: {}",
                                    header_path.display(),
                                    e
                                );
                            }
                        } else {
                            indexed_count += 1;
                        }
                    }
                    Err(e) => {
                        eprintln!("Warning: Failed to read {}: {}", header_path.display(), e);
                    }
                }
            }
        }
        Ok(indexed_count)
    }

    fn include_dirs(&self) -> Vec<PathBuf> {
        self.include_dirs.clone()
    }

    fn source_files(&self) -> Vec<PathBuf> {
        self.header_files.clone()
    }

    fn name(&self) -> &str {
        &self.solution.name
    }

    fn root_dir(&self) -> &Path {
        self.solution_path.parent().unwrap_or(Path::new("."))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_solution(dir: &Path) -> PathBuf {
        let sln_content = r#"
Microsoft Visual Studio Solution File, Format Version 12.00
# Visual Studio Version 17
Project("{8BC9CEB8-8B4A-11D0-8D11-00A0C91BC942}") = "TestProject", "TestProject\TestProject.vcxproj", "{12345678-1234-1234-1234-123456789ABC}"
EndProject
Global
EndGlobal
"#;
        let sln_path = dir.join("Test.sln");
        std::fs::write(&sln_path, sln_content).unwrap();

        // Create project directory
        let proj_dir = dir.join("TestProject");
        std::fs::create_dir_all(&proj_dir).unwrap();

        // Create minimal vcxproj
        let vcxproj_content = r#"<?xml version="1.0" encoding="utf-8"?>
<Project DefaultTargets="Build" xmlns="http://schemas.microsoft.com/developer/msbuild/2003">
  <ItemGroup Label="ProjectConfigurations">
    <ProjectConfiguration Include="Debug|x64">
      <Configuration>Debug</Configuration>
      <Platform>x64</Platform>
    </ProjectConfiguration>
  </ItemGroup>
  <PropertyGroup Label="Globals">
    <ProjectGuid>{12345678-1234-1234-1234-123456789ABC}</ProjectGuid>
  </PropertyGroup>
  <ItemGroup>
    <ClInclude Include="test.h" />
  </ItemGroup>
</Project>
"#;
        let vcxproj_path = proj_dir.join("TestProject.vcxproj");
        std::fs::write(&vcxproj_path, vcxproj_content).unwrap();

        // Create a header file
        let header_content = r#"
struct TestStruct {
    int value;
};
"#;
        std::fs::write(proj_dir.join("test.h"), header_content).unwrap();

        sln_path
    }

    #[test]
    fn test_vs_solution_indexer_creation() {
        let temp_dir = TempDir::new().unwrap();
        let sln_path = create_test_solution(temp_dir.path());

        let indexer = VsSolutionIndexer::from_path(&sln_path);
        assert!(indexer.is_ok());

        let indexer = indexer.unwrap();
        assert_eq!(indexer.project_count(), 1);
    }

    #[test]
    fn test_vs_solution_indexer_index() {
        let temp_dir = TempDir::new().unwrap();
        let sln_path = create_test_solution(temp_dir.path());

        let indexer = VsSolutionIndexer::from_path(&sln_path).unwrap();
        let mut index = SymbolIndex::new();

        let count = indexer.index(&mut index).unwrap();
        assert!(count >= 1);

        // Check if the struct was indexed
        let defs = index.find_definition("TestStruct");
        assert_eq!(defs.len(), 1);
    }
}
