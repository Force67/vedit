//! Visual Studio solution and project file parser.
//!
//! This crate provides parsing support for Visual Studio solution files (.sln)
//! and C/C++ project files (.vcxproj), extracting build configurations,
//! include paths, preprocessor definitions, and other project metadata.

use roxmltree::Document;
use std::collections::HashMap;
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

/// A build configuration + platform pair (e.g., "Debug|x64").
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConfigurationPlatform {
    pub configuration: String,
    pub platform: String,
}

impl ConfigurationPlatform {
    pub fn new(configuration: impl Into<String>, platform: impl Into<String>) -> Self {
        Self {
            configuration: configuration.into(),
            platform: platform.into(),
        }
    }

    /// Parse from "Configuration|Platform" format.
    pub fn parse(s: &str) -> Option<Self> {
        let (config, platform) = s.split_once('|')?;
        Some(Self {
            configuration: config.trim().to_string(),
            platform: platform.trim().to_string(),
        })
    }

    /// Format as "Configuration|Platform".
    pub fn as_str(&self) -> String {
        format!("{}|{}", self.configuration, self.platform)
    }
}

impl std::fmt::Display for ConfigurationPlatform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}|{}", self.configuration, self.platform)
    }
}

/// Representation of a Visual Studio solution (.sln) file.
#[derive(Debug, Clone)]
pub struct Solution {
    pub name: String,
    pub path: PathBuf,
    pub projects: Vec<SolutionProject>,
    /// Solution-level build configurations (e.g., Debug|x64, Release|Win32).
    pub configurations: Vec<ConfigurationPlatform>,
    /// Mapping of project GUID to its configuration mappings.
    pub project_configurations: HashMap<String, Vec<ProjectConfigurationMapping>>,
    /// Solution folders (virtual folders for organization).
    pub folders: Vec<SolutionFolder>,
    /// Visual Studio version from the solution header.
    pub vs_version: Option<String>,
    /// Minimum VS version from the solution header.
    pub minimum_vs_version: Option<String>,
}

/// Maps a solution configuration to a project configuration.
#[derive(Debug, Clone)]
pub struct ProjectConfigurationMapping {
    /// The solution-level configuration (e.g., Debug|x64).
    pub solution_config: ConfigurationPlatform,
    /// The project-level configuration (e.g., Debug|x64).
    pub project_config: ConfigurationPlatform,
    /// Whether the project builds in this configuration.
    pub build: bool,
    /// Whether the project deploys in this configuration.
    pub deploy: bool,
}

/// A virtual folder in the solution for organizing projects.
#[derive(Debug, Clone)]
pub struct SolutionFolder {
    pub name: String,
    pub guid: String,
    /// GUIDs of projects or folders nested in this folder.
    pub children: Vec<String>,
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
    /// Available build configurations in this project.
    pub configurations: Vec<ConfigurationPlatform>,
    /// Configuration-specific settings.
    pub config_settings: HashMap<String, ConfigurationSettings>,
    /// Project references (dependencies on other projects).
    pub project_references: Vec<ProjectReference>,
    /// Global properties that apply to all configurations.
    pub globals: ProjectGlobals,
}

/// Global project properties.
#[derive(Debug, Clone, Default)]
pub struct ProjectGlobals {
    /// Project GUID.
    pub project_guid: Option<String>,
    /// Root namespace.
    pub root_namespace: Option<String>,
    /// Windows target platform version.
    pub windows_target_platform_version: Option<String>,
    /// Platform toolset (e.g., v143, v142).
    pub platform_toolset: Option<String>,
    /// Project keyword (e.g., Win32Proj).
    pub keyword: Option<String>,
}

/// Configuration-specific build settings.
#[derive(Debug, Clone, Default)]
pub struct ConfigurationSettings {
    /// The configuration this applies to.
    pub config: Option<ConfigurationPlatform>,
    /// Output type (Application, DynamicLibrary, StaticLibrary).
    pub configuration_type: Option<ConfigurationType>,
    /// Use of MFC (false, Static, Dynamic).
    pub use_of_mfc: Option<String>,
    /// Character set (Unicode, MultiByte, NotSet).
    pub character_set: Option<String>,
    /// Whole program optimization.
    pub whole_program_optimization: Option<bool>,
    /// Output directory.
    pub out_dir: Option<String>,
    /// Intermediate directory.
    pub int_dir: Option<String>,
    /// Target name (output file name without extension).
    pub target_name: Option<String>,
    /// Target extension (e.g., .exe, .dll, .lib).
    pub target_ext: Option<String>,
    /// Compiler settings.
    pub compiler: CompilerSettings,
    /// Linker settings.
    pub linker: LinkerSettings,
}

/// Output type of the project.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigurationType {
    Application,
    DynamicLibrary,
    StaticLibrary,
    Utility,
    Makefile,
}

impl ConfigurationType {
    fn from_str(s: &str) -> Option<Self> {
        match s.trim() {
            "Application" => Some(Self::Application),
            "DynamicLibrary" => Some(Self::DynamicLibrary),
            "StaticLibrary" => Some(Self::StaticLibrary),
            "Utility" => Some(Self::Utility),
            "Makefile" => Some(Self::Makefile),
            _ => None,
        }
    }

    /// Returns true if this configuration type produces an executable.
    pub fn is_executable(&self) -> bool {
        matches!(self, Self::Application)
    }
}

/// Compiler (ClCompile) settings.
#[derive(Debug, Clone, Default)]
pub struct CompilerSettings {
    /// Additional include directories.
    pub include_dirs: Vec<String>,
    /// Preprocessor definitions.
    pub preprocessor_definitions: Vec<String>,
    /// Warning level (e.g., Level3, Level4, EnableAllWarnings).
    pub warning_level: Option<String>,
    /// Treat warnings as errors.
    pub treat_warnings_as_errors: Option<bool>,
    /// Optimization level (Disabled, MinSpace, MaxSpeed, Full).
    pub optimization: Option<String>,
    /// Function-level linking.
    pub function_level_linking: Option<bool>,
    /// Intrinsic functions.
    pub intrinsic_functions: Option<bool>,
    /// SDL checks.
    pub sdl_check: Option<bool>,
    /// Conformance mode.
    pub conformance_mode: Option<bool>,
    /// C++ language standard (e.g., stdcpp17, stdcpp20).
    pub language_standard: Option<String>,
    /// C language standard (e.g., stdc11, stdc17).
    pub c_language_standard: Option<String>,
    /// Debug information format (None, ProgramDatabase, EditAndContinue).
    pub debug_information_format: Option<String>,
    /// Runtime library (MultiThreaded, MultiThreadedDLL, MultiThreadedDebug, MultiThreadedDebugDLL).
    pub runtime_library: Option<String>,
    /// Precompiled header mode.
    pub precompiled_header: Option<String>,
    /// Precompiled header file.
    pub precompiled_header_file: Option<String>,
    /// Additional compiler options.
    pub additional_options: Vec<String>,
}

/// Linker settings.
#[derive(Debug, Clone, Default)]
pub struct LinkerSettings {
    /// Additional library directories.
    pub library_dirs: Vec<String>,
    /// Additional dependencies (libraries to link).
    pub additional_dependencies: Vec<String>,
    /// Generate debug info.
    pub generate_debug_information: Option<bool>,
    /// Subsystem (Console, Windows, Native).
    pub subsystem: Option<String>,
    /// Enable COMDAT folding.
    pub enable_comdat_folding: Option<bool>,
    /// Optimize references.
    pub optimize_references: Option<bool>,
    /// Output file path.
    pub output_file: Option<String>,
    /// Import library path.
    pub import_library: Option<String>,
    /// Program database file.
    pub program_database_file: Option<String>,
    /// Additional linker options.
    pub additional_options: Vec<String>,
}

/// A reference to another project.
#[derive(Debug, Clone)]
pub struct ProjectReference {
    /// Path to the referenced project file.
    pub include: PathBuf,
    /// Full resolved path.
    pub full_path: PathBuf,
    /// Project GUID of the referenced project.
    pub project_guid: Option<String>,
    /// Name hint for the referenced project.
    pub name: Option<String>,
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

// Well-known project type GUIDs
pub mod project_types {
    /// C++ project
    pub const VCXPROJ: &str = "8BC9CEB8-8B4A-11D0-8D11-00A0C91BC942";
    /// C# project
    pub const CSPROJ: &str = "FAE04EC0-301F-11D3-BF4B-00C04F79EFBC";
    /// Solution folder (virtual)
    pub const SOLUTION_FOLDER: &str = "2150E333-8FDC-42A3-9474-1A3956D46DE8";
    /// VB.NET project
    pub const VBPROJ: &str = "F184B08F-C81C-45F6-A57F-5ABD9991F28F";
    /// F# project
    pub const FSPROJ: &str = "F2A71F9B-5D33-465A-A702-920D77279786";
}

impl Solution {
    /// Parse a Visual Studio solution file from disk.
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let contents = fs::read_to_string(path).map_err(|source| VisualStudioError::Io {
            path: path.to_path_buf(),
            source,
        })?;

        Self::parse(&contents, path)
    }

    /// Parse a Visual Studio solution from a string.
    pub fn parse(contents: &str, path: &Path) -> Result<Self> {
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
        let mut configurations = Vec::new();
        let mut project_configurations: HashMap<String, Vec<ProjectConfigurationMapping>> =
            HashMap::new();
        let mut folders = Vec::new();
        let mut vs_version = None;
        let mut minimum_vs_version = None;

        // Track nested project relationships
        let mut nested_projects: HashMap<String, String> = HashMap::new();

        let lines: Vec<&str> = contents.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i];
            let trimmed = line.trim();

            // Parse VS version from header
            if trimmed.starts_with("VisualStudioVersion") {
                if let Some(value) = trimmed.split('=').nth(1) {
                    vs_version = Some(value.trim().to_string());
                }
            } else if trimmed.starts_with("MinimumVisualStudioVersion") {
                if let Some(value) = trimmed.split('=').nth(1) {
                    minimum_vs_version = Some(value.trim().to_string());
                }
            }
            // Parse project entries
            else if trimmed.starts_with("Project(") {
                let entry = parse_project_line(trimmed).map_err(|message| {
                    VisualStudioError::SolutionParse {
                        path: path.to_path_buf(),
                        line: i + 1,
                        message,
                    }
                })?;

                // Check if this is a solution folder
                let is_folder = entry
                    .project_type_guid
                    .as_ref()
                    .map(|g| g.eq_ignore_ascii_case(project_types::SOLUTION_FOLDER))
                    .unwrap_or(false);

                if is_folder {
                    folders.push(SolutionFolder {
                        name: entry.name,
                        guid: entry.project_guid.clone().unwrap_or_default(),
                        children: Vec::new(),
                    });
                } else {
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

                    // Load vcxproj files
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
            }
            // Parse Global section
            else if trimmed == "Global" {
                i += 1;
                while i < lines.len() {
                    let global_line = lines[i].trim();
                    if global_line == "EndGlobal" {
                        break;
                    }

                    // Parse SolutionConfigurationPlatforms
                    if global_line.starts_with("GlobalSection(SolutionConfigurationPlatforms)") {
                        i += 1;
                        while i < lines.len() {
                            let config_line = lines[i].trim();
                            if config_line == "EndGlobalSection" {
                                break;
                            }
                            // Format: Debug|x64 = Debug|x64
                            if let Some((left, _)) = config_line.split_once('=') {
                                if let Some(config) = ConfigurationPlatform::parse(left.trim()) {
                                    if !configurations.contains(&config) {
                                        configurations.push(config);
                                    }
                                }
                            }
                            i += 1;
                        }
                    }
                    // Parse ProjectConfigurationPlatforms
                    else if global_line
                        .starts_with("GlobalSection(ProjectConfigurationPlatforms)")
                    {
                        i += 1;
                        while i < lines.len() {
                            let config_line = lines[i].trim();
                            if config_line == "EndGlobalSection" {
                                break;
                            }
                            // Format: {GUID}.Debug|x64.ActiveCfg = Debug|x64
                            // Format: {GUID}.Debug|x64.Build.0 = Debug|x64
                            if let Some((left, right)) = config_line.split_once('=') {
                                parse_project_config_line(
                                    left.trim(),
                                    right.trim(),
                                    &mut project_configurations,
                                );
                            }
                            i += 1;
                        }
                    }
                    // Parse NestedProjects
                    else if global_line.starts_with("GlobalSection(NestedProjects)") {
                        i += 1;
                        while i < lines.len() {
                            let nested_line = lines[i].trim();
                            if nested_line == "EndGlobalSection" {
                                break;
                            }
                            // Format: {ChildGUID} = {ParentGUID}
                            if let Some((child, parent)) = nested_line.split_once('=') {
                                let child_guid = extract_guid(child.trim());
                                let parent_guid = extract_guid(parent.trim());
                                if let (Some(c), Some(p)) = (child_guid, parent_guid) {
                                    nested_projects.insert(c, p);
                                }
                            }
                            i += 1;
                        }
                    }

                    i += 1;
                }
            }

            i += 1;
        }

        // Apply nested project relationships to folders
        for folder in &mut folders {
            for (child_guid, parent_guid) in &nested_projects {
                if parent_guid.eq_ignore_ascii_case(&folder.guid) {
                    folder.children.push(child_guid.clone());
                }
            }
        }

        Ok(Solution {
            name,
            path: path.to_path_buf(),
            projects,
            configurations,
            project_configurations,
            folders,
            vs_version,
            minimum_vs_version,
        })
    }

    /// Get projects that produce executables.
    pub fn executable_projects(&self) -> impl Iterator<Item = &SolutionProject> {
        self.projects.iter().filter(|p| {
            p.project
                .as_ref()
                .map(|vcx| vcx.produces_executable)
                .unwrap_or(false)
        })
    }

    /// Get project by GUID.
    pub fn project_by_guid(&self, guid: &str) -> Option<&SolutionProject> {
        self.projects.iter().find(|p| {
            p.project_guid
                .as_ref()
                .map(|g| g.eq_ignore_ascii_case(guid))
                .unwrap_or(false)
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

        Self::parse(&contents, path)
    }

    /// Parse a Visual Studio C/C++ project from a string.
    pub fn parse(contents: &str, path: &Path) -> Result<Self> {
        let document = Document::parse(contents).map_err(|source| VisualStudioError::Xml {
            path: path.to_path_buf(),
            source,
        })?;

        let project_dir = path
            .parent()
            .map(normalize_path)
            .unwrap_or_else(|| PathBuf::from("."));

        let mut files = Vec::new();
        let mut produces_executable = false;
        let mut configurations = Vec::new();
        let mut config_settings: HashMap<String, ConfigurationSettings> = HashMap::new();
        let mut project_references = Vec::new();
        let mut globals = ProjectGlobals::default();

        // First pass: collect configurations and global properties
        for node in document.descendants() {
            if !node.is_element() {
                continue;
            }

            let tag_name = node.tag_name().name();

            // Parse ProjectConfiguration items
            if tag_name == "ProjectConfiguration" {
                if let Some(include) = node.attribute("Include") {
                    if let Some(config) = ConfigurationPlatform::parse(include) {
                        if !configurations.contains(&config) {
                            configurations.push(config.clone());
                            config_settings.insert(
                                config.as_str(),
                                ConfigurationSettings {
                                    config: Some(config),
                                    ..Default::default()
                                },
                            );
                        }
                    }
                }
            }

            // Parse PropertyGroup globals
            if tag_name == "PropertyGroup" {
                let label = node.attribute("Label").unwrap_or("");
                if label == "Globals" {
                    for child in node.children().filter(|c| c.is_element()) {
                        let child_tag = child.tag_name().name();
                        let text = child.text().map(|t| t.trim().to_string());
                        match child_tag {
                            "ProjectGuid" => {
                                globals.project_guid = text.as_ref().and_then(|t| extract_guid(t))
                            }
                            "RootNamespace" => globals.root_namespace = text,
                            "WindowsTargetPlatformVersion" => {
                                globals.windows_target_platform_version = text
                            }
                            "Keyword" => globals.keyword = text,
                            _ => {}
                        }
                    }
                }
            }
        }

        // Second pass: collect configuration-specific settings
        for node in document.descendants() {
            if !node.is_element() {
                continue;
            }

            let tag_name = node.tag_name().name();
            let condition = node.attribute("Condition").unwrap_or("");

            // Parse PropertyGroup with configuration condition
            if tag_name == "PropertyGroup" {
                if let Some(config_key) = extract_config_from_condition(condition) {
                    let settings = config_settings.entry(config_key).or_default();

                    for child in node.children().filter(|c| c.is_element()) {
                        let child_tag = child.tag_name().name();
                        let text = child.text().map(|t| t.trim().to_string());

                        match child_tag {
                            "ConfigurationType" => {
                                if let Some(t) = text.as_ref() {
                                    settings.configuration_type = ConfigurationType::from_str(t);
                                    if settings
                                        .configuration_type
                                        .map(|ct| ct.is_executable())
                                        .unwrap_or(false)
                                    {
                                        produces_executable = true;
                                    }
                                }
                            }
                            "UseOfMfc" => settings.use_of_mfc = text,
                            "CharacterSet" => settings.character_set = text,
                            "WholeProgramOptimization" => {
                                settings.whole_program_optimization =
                                    text.map(|t| t.eq_ignore_ascii_case("true"))
                            }
                            "OutDir" => settings.out_dir = text,
                            "IntDir" => settings.int_dir = text,
                            "TargetName" => settings.target_name = text,
                            "TargetExt" => settings.target_ext = text,
                            "PlatformToolset" => globals.platform_toolset = text,
                            _ => {}
                        }
                    }
                }
            }

            // Parse ItemDefinitionGroup (ClCompile and Link settings)
            if tag_name == "ItemDefinitionGroup" {
                if let Some(config_key) = extract_config_from_condition(condition) {
                    let settings = config_settings.entry(config_key).or_default();

                    for child in node.children().filter(|c| c.is_element()) {
                        let child_tag = child.tag_name().name();

                        if child_tag == "ClCompile" {
                            parse_compiler_settings(child, &mut settings.compiler);
                        } else if child_tag == "Link" {
                            parse_linker_settings(child, &mut settings.linker);
                        }
                    }
                }
            }

            // Also check for ConfigurationType without condition (fallback)
            if tag_name == "ConfigurationType" && condition.is_empty() {
                if let Some(text) = node.text() {
                    if text.trim().eq_ignore_ascii_case("Application") {
                        produces_executable = true;
                    }
                }
            }
        }

        // Third pass: collect files and project references
        for node in document.descendants() {
            if !node.is_element() {
                continue;
            }

            let tag_name = node.tag_name().name();

            // Parse file items
            if let Some(kind) = VcxItemKind::from_tag(tag_name) {
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

            // Parse project references
            if tag_name == "ProjectReference" {
                if let Some(include) = node.attribute("Include") {
                    if let Some(relative_path) = normalize_include(include) {
                        let full_path = resolve_path(&project_dir, &relative_path);

                        let mut project_guid = None;
                        let mut name = None;

                        for child in node.children().filter(|c| c.is_element()) {
                            match child.tag_name().name() {
                                "Project" => {
                                    project_guid = child.text().and_then(|t| extract_guid(t.trim()))
                                }
                                "Name" => name = child.text().map(|t| t.trim().to_string()),
                                _ => {}
                            }
                        }

                        project_references.push(ProjectReference {
                            include: relative_path,
                            full_path,
                            project_guid,
                            name,
                        });
                    }
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
            configurations,
            config_settings,
            project_references,
            globals,
        })
    }

    /// Get settings for a specific configuration.
    pub fn settings_for(&self, config: &ConfigurationPlatform) -> Option<&ConfigurationSettings> {
        self.config_settings.get(&config.as_str())
    }

    /// Get all include directories across all configurations.
    pub fn all_include_dirs(&self) -> Vec<&str> {
        let mut dirs: Vec<&str> = self
            .config_settings
            .values()
            .flat_map(|s| s.compiler.include_dirs.iter().map(|d| d.as_str()))
            .collect();
        dirs.sort();
        dirs.dedup();
        dirs
    }

    /// Get all preprocessor definitions across all configurations.
    pub fn all_preprocessor_definitions(&self) -> Vec<&str> {
        let mut defs: Vec<&str> = self
            .config_settings
            .values()
            .flat_map(|s| {
                s.compiler
                    .preprocessor_definitions
                    .iter()
                    .map(|d| d.as_str())
            })
            .collect();
        defs.sort();
        defs.dedup();
        defs
    }

    /// Get the guessed output path for a configuration.
    pub fn output_path(&self, config: &ConfigurationPlatform) -> Option<PathBuf> {
        let settings = self.settings_for(config)?;
        let out_dir = settings.out_dir.as_ref()?;
        let target_name = settings
            .target_name
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or(&self.name);
        let target_ext = settings
            .target_ext
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or(".exe");

        let project_dir = self.path.parent()?;
        let out_path = resolve_path(project_dir, Path::new(out_dir));
        Some(out_path.join(format!("{}{}", target_name, target_ext)))
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

// Helper to parse compiler settings from ClCompile element
fn parse_compiler_settings(node: roxmltree::Node, settings: &mut CompilerSettings) {
    for child in node.children().filter(|c| c.is_element()) {
        let tag = child.tag_name().name();
        let text = child.text().map(|t| t.trim());

        match tag {
            "AdditionalIncludeDirectories" => {
                if let Some(t) = text {
                    settings.include_dirs = parse_semicolon_list(t);
                }
            }
            "PreprocessorDefinitions" => {
                if let Some(t) = text {
                    settings.preprocessor_definitions = parse_semicolon_list(t);
                }
            }
            "WarningLevel" => settings.warning_level = text.map(|t| t.to_string()),
            "TreatWarningAsError" => {
                settings.treat_warnings_as_errors = text.map(|t| t.eq_ignore_ascii_case("true"))
            }
            "Optimization" => settings.optimization = text.map(|t| t.to_string()),
            "FunctionLevelLinking" => {
                settings.function_level_linking = text.map(|t| t.eq_ignore_ascii_case("true"))
            }
            "IntrinsicFunctions" => {
                settings.intrinsic_functions = text.map(|t| t.eq_ignore_ascii_case("true"))
            }
            "SDLCheck" => settings.sdl_check = text.map(|t| t.eq_ignore_ascii_case("true")),
            "ConformanceMode" => {
                settings.conformance_mode = text.map(|t| t.eq_ignore_ascii_case("true"))
            }
            "LanguageStandard" => settings.language_standard = text.map(|t| t.to_string()),
            "LanguageStandard_C" => settings.c_language_standard = text.map(|t| t.to_string()),
            "DebugInformationFormat" => {
                settings.debug_information_format = text.map(|t| t.to_string())
            }
            "RuntimeLibrary" => settings.runtime_library = text.map(|t| t.to_string()),
            "PrecompiledHeader" => settings.precompiled_header = text.map(|t| t.to_string()),
            "PrecompiledHeaderFile" => {
                settings.precompiled_header_file = text.map(|t| t.to_string())
            }
            "AdditionalOptions" => {
                if let Some(t) = text {
                    settings.additional_options = parse_space_list(t);
                }
            }
            _ => {}
        }
    }
}

// Helper to parse linker settings from Link element
fn parse_linker_settings(node: roxmltree::Node, settings: &mut LinkerSettings) {
    for child in node.children().filter(|c| c.is_element()) {
        let tag = child.tag_name().name();
        let text = child.text().map(|t| t.trim());

        match tag {
            "AdditionalLibraryDirectories" => {
                if let Some(t) = text {
                    settings.library_dirs = parse_semicolon_list(t);
                }
            }
            "AdditionalDependencies" => {
                if let Some(t) = text {
                    settings.additional_dependencies = parse_semicolon_list(t);
                }
            }
            "GenerateDebugInformation" => {
                settings.generate_debug_information = text
                    .map(|t| t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("DebugFull"))
            }
            "SubSystem" => settings.subsystem = text.map(|t| t.to_string()),
            "EnableCOMDATFolding" => {
                settings.enable_comdat_folding = text.map(|t| t.eq_ignore_ascii_case("true"))
            }
            "OptimizeReferences" => {
                settings.optimize_references = text.map(|t| t.eq_ignore_ascii_case("true"))
            }
            "OutputFile" => settings.output_file = text.map(|t| t.to_string()),
            "ImportLibrary" => settings.import_library = text.map(|t| t.to_string()),
            "ProgramDatabaseFile" => settings.program_database_file = text.map(|t| t.to_string()),
            "AdditionalOptions" => {
                if let Some(t) = text {
                    settings.additional_options = parse_space_list(t);
                }
            }
            _ => {}
        }
    }
}

// Parse semicolon-separated list, filtering out MSBuild variables
fn parse_semicolon_list(s: &str) -> Vec<String> {
    s.split(';')
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .filter(|part| !part.contains("%("))
        .map(|part| part.replace('\\', "/"))
        .collect()
}

// Parse space-separated options
fn parse_space_list(s: &str) -> Vec<String> {
    s.split_whitespace()
        .filter(|part| !part.is_empty())
        .map(|part| part.to_string())
        .collect()
}

// Extract configuration key from MSBuild condition
fn extract_config_from_condition(condition: &str) -> Option<String> {
    // Format: '$(Configuration)|$(Platform)'=='Debug|x64'
    if let Some(start) = condition.find("=='") {
        let rest = &condition[start + 3..];
        if let Some(end) = rest.find('\'') {
            let config_str = &rest[..end];
            return Some(config_str.to_string());
        }
    }
    None
}

// Extract GUID from string (handles {GUID} format)
fn extract_guid(s: &str) -> Option<String> {
    let trimmed = s.trim();
    let inner = trimmed
        .strip_prefix('{')
        .and_then(|s| s.strip_suffix('}'))
        .unwrap_or(trimmed);
    if inner.is_empty() {
        None
    } else {
        Some(inner.to_uppercase())
    }
}

// Parse project configuration line from GlobalSection(ProjectConfigurationPlatforms)
fn parse_project_config_line(
    left: &str,
    right: &str,
    mappings: &mut HashMap<String, Vec<ProjectConfigurationMapping>>,
) {
    // Format: {GUID}.Debug|x64.ActiveCfg = Debug|x64
    // Format: {GUID}.Debug|x64.Build.0 = Debug|x64

    let parts: Vec<&str> = left.splitn(3, '.').collect();
    if parts.len() < 3 {
        return;
    }

    let guid = match extract_guid(parts[0]) {
        Some(g) => g,
        None => return,
    };

    let solution_config = match ConfigurationPlatform::parse(parts[1]) {
        Some(c) => c,
        None => return,
    };

    let action = parts[2];
    let project_config = match ConfigurationPlatform::parse(right) {
        Some(c) => c,
        None => return,
    };

    let entry = mappings.entry(guid).or_default();

    // Find or create mapping for this solution config
    let mapping = entry
        .iter_mut()
        .find(|m| m.solution_config == solution_config);

    if let Some(m) = mapping {
        if action == "Build.0" {
            m.build = true;
        } else if action.starts_with("Deploy") {
            m.deploy = true;
        }
    } else {
        entry.push(ProjectConfigurationMapping {
            solution_config,
            project_config,
            build: action == "Build.0",
            deploy: action.starts_with("Deploy"),
        });
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
            "Project(\"{8BC9CEB8-8B4A-11D0-8D11-00A0C91BC942}\") = \"sample\", \"sample.vcxproj\", \"{11111111-2222-3333-4444-555555555555}\"\nEndProject\n",
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

    #[test]
    fn parse_configuration_platform() {
        let config = ConfigurationPlatform::parse("Debug|x64").unwrap();
        assert_eq!(config.configuration, "Debug");
        assert_eq!(config.platform, "x64");
        assert_eq!(config.as_str(), "Debug|x64");
    }

    #[test]
    fn parse_solution_configurations() {
        let dir = tempdir().unwrap();
        let solution_path = dir.path().join("test.sln");

        fs::write(
            &solution_path,
            r#"
Microsoft Visual Studio Solution File, Format Version 12.00
# Visual Studio Version 17
VisualStudioVersion = 17.5.33516.290
MinimumVisualStudioVersion = 10.0.40219.1
Global
    GlobalSection(SolutionConfigurationPlatforms) = preSolution
        Debug|x64 = Debug|x64
        Debug|x86 = Debug|x86
        Release|x64 = Release|x64
        Release|x86 = Release|x86
    EndGlobalSection
EndGlobal
"#,
        )
        .unwrap();

        let solution = Solution::from_path(&solution_path).unwrap();
        assert_eq!(solution.configurations.len(), 4);
        assert_eq!(solution.vs_version, Some("17.5.33516.290".to_string()));
        assert_eq!(
            solution.minimum_vs_version,
            Some("10.0.40219.1".to_string())
        );
    }

    #[test]
    fn parse_vcxproj_configurations_and_settings() {
        let dir = tempdir().unwrap();
        let project_path = dir.path().join("test.vcxproj");

        fs::write(
            &project_path,
            r#"<?xml version="1.0" encoding="utf-8"?>
<Project DefaultTargets="Build" xmlns="http://schemas.microsoft.com/developer/msbuild/2003">
  <ItemGroup Label="ProjectConfigurations">
    <ProjectConfiguration Include="Debug|x64">
      <Configuration>Debug</Configuration>
      <Platform>x64</Platform>
    </ProjectConfiguration>
    <ProjectConfiguration Include="Release|x64">
      <Configuration>Release</Configuration>
      <Platform>x64</Platform>
    </ProjectConfiguration>
  </ItemGroup>
  <PropertyGroup Label="Globals">
    <ProjectGuid>{12345678-1234-1234-1234-123456789012}</ProjectGuid>
    <RootNamespace>TestProject</RootNamespace>
    <WindowsTargetPlatformVersion>10.0</WindowsTargetPlatformVersion>
  </PropertyGroup>
  <PropertyGroup Condition="'$(Configuration)|$(Platform)'=='Debug|x64'">
    <ConfigurationType>Application</ConfigurationType>
    <OutDir>$(SolutionDir)bin\Debug\</OutDir>
    <IntDir>$(SolutionDir)obj\Debug\</IntDir>
    <TargetName>test_app</TargetName>
    <TargetExt>.exe</TargetExt>
  </PropertyGroup>
  <PropertyGroup Condition="'$(Configuration)|$(Platform)'=='Release|x64'">
    <ConfigurationType>Application</ConfigurationType>
    <OutDir>$(SolutionDir)bin\Release\</OutDir>
    <WholeProgramOptimization>true</WholeProgramOptimization>
  </PropertyGroup>
  <ItemDefinitionGroup Condition="'$(Configuration)|$(Platform)'=='Debug|x64'">
    <ClCompile>
      <AdditionalIncludeDirectories>src;include;third_party</AdditionalIncludeDirectories>
      <PreprocessorDefinitions>DEBUG;_DEBUG;WIN32</PreprocessorDefinitions>
      <WarningLevel>Level4</WarningLevel>
      <Optimization>Disabled</Optimization>
      <LanguageStandard>stdcpp17</LanguageStandard>
    </ClCompile>
    <Link>
      <AdditionalLibraryDirectories>lib;third_party\lib</AdditionalLibraryDirectories>
      <AdditionalDependencies>kernel32.lib;user32.lib</AdditionalDependencies>
      <SubSystem>Console</SubSystem>
      <GenerateDebugInformation>true</GenerateDebugInformation>
    </Link>
  </ItemDefinitionGroup>
  <ItemGroup>
    <ClCompile Include="src\main.cpp" />
    <ClInclude Include="include\header.h" />
    <ProjectReference Include="..\other\other.vcxproj">
      <Project>{AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE}</Project>
      <Name>OtherProject</Name>
    </ProjectReference>
  </ItemGroup>
</Project>
"#,
        )
        .unwrap();

        let project = VcxProject::from_path(&project_path).unwrap();

        // Check configurations
        assert_eq!(project.configurations.len(), 2);
        assert!(
            project
                .configurations
                .iter()
                .any(|c| c.as_str() == "Debug|x64")
        );
        assert!(
            project
                .configurations
                .iter()
                .any(|c| c.as_str() == "Release|x64")
        );

        // Check globals
        assert_eq!(
            project.globals.project_guid,
            Some("12345678-1234-1234-1234-123456789012".to_string())
        );
        assert_eq!(
            project.globals.root_namespace,
            Some("TestProject".to_string())
        );

        // Check debug settings
        let debug_config = ConfigurationPlatform::new("Debug", "x64");
        let debug_settings = project.settings_for(&debug_config).unwrap();
        assert_eq!(
            debug_settings.configuration_type,
            Some(ConfigurationType::Application)
        );
        assert_eq!(debug_settings.target_name, Some("test_app".to_string()));

        // Check compiler settings
        assert_eq!(debug_settings.compiler.include_dirs.len(), 3);
        assert!(
            debug_settings
                .compiler
                .include_dirs
                .contains(&"src".to_string())
        );
        assert_eq!(
            debug_settings.compiler.warning_level,
            Some("Level4".to_string())
        );
        assert_eq!(
            debug_settings.compiler.language_standard,
            Some("stdcpp17".to_string())
        );

        // Check preprocessor definitions
        assert!(
            debug_settings
                .compiler
                .preprocessor_definitions
                .contains(&"DEBUG".to_string())
        );

        // Check linker settings
        assert_eq!(debug_settings.linker.library_dirs.len(), 2);
        assert_eq!(debug_settings.linker.subsystem, Some("Console".to_string()));
        assert_eq!(debug_settings.linker.generate_debug_information, Some(true));

        // Check project references
        assert_eq!(project.project_references.len(), 1);
        assert_eq!(
            project.project_references[0].name,
            Some("OtherProject".to_string())
        );
        assert_eq!(
            project.project_references[0].project_guid,
            Some("AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE".to_string())
        );

        // Check helper methods
        let all_includes = project.all_include_dirs();
        assert!(all_includes.contains(&"src"));
        assert!(all_includes.contains(&"include"));

        let all_defs = project.all_preprocessor_definitions();
        assert!(all_defs.contains(&"DEBUG"));
    }

    #[test]
    fn parse_solution_folders() {
        let dir = tempdir().unwrap();
        let solution_path = dir.path().join("test.sln");

        fs::write(
            &solution_path,
            r#"
Microsoft Visual Studio Solution File, Format Version 12.00
Project("{2150E333-8FDC-42A3-9474-1A3956D46DE8}") = "Libraries", "Libraries", "{FOLDER-GUID-1234}"
EndProject
Project("{8BC9CEB8-8B4A-11D0-8D11-00A0C91BC942}") = "MyLib", "libs\MyLib.vcxproj", "{PROJECT-GUID-5678}"
EndProject
Global
    GlobalSection(NestedProjects) = preSolution
        {PROJECT-GUID-5678} = {FOLDER-GUID-1234}
    EndGlobalSection
EndGlobal
"#,
        )
        .unwrap();

        let solution = Solution::from_path(&solution_path).unwrap();

        // Should have one folder
        assert_eq!(solution.folders.len(), 1);
        assert_eq!(solution.folders[0].name, "Libraries");

        // Folder should contain the project
        assert!(
            solution.folders[0]
                .children
                .iter()
                .any(|c| c.contains("PROJECT-GUID-5678"))
        );

        // Should have one actual project (not counting folder)
        assert_eq!(solution.projects.len(), 1);
        assert_eq!(solution.projects[0].name, "MyLib");
    }

    #[test]
    fn parse_project_configuration_mappings() {
        let dir = tempdir().unwrap();
        let solution_path = dir.path().join("test.sln");

        fs::write(
            &solution_path,
            r#"
Microsoft Visual Studio Solution File, Format Version 12.00
Project("{8BC9CEB8-8B4A-11D0-8D11-00A0C91BC942}") = "App", "App.vcxproj", "{11111111-2222-3333-4444-555555555555}"
EndProject
Global
    GlobalSection(SolutionConfigurationPlatforms) = preSolution
        Debug|x64 = Debug|x64
        Release|x64 = Release|x64
    EndGlobalSection
    GlobalSection(ProjectConfigurationPlatforms) = postSolution
        {11111111-2222-3333-4444-555555555555}.Debug|x64.ActiveCfg = Debug|x64
        {11111111-2222-3333-4444-555555555555}.Debug|x64.Build.0 = Debug|x64
        {11111111-2222-3333-4444-555555555555}.Release|x64.ActiveCfg = Release|x64
    EndGlobalSection
EndGlobal
"#,
        )
        .unwrap();

        let solution = Solution::from_path(&solution_path).unwrap();

        // Check project configurations
        let guid = "11111111-2222-3333-4444-555555555555";
        let mappings = solution.project_configurations.get(guid).unwrap();

        // Debug should have build enabled
        let debug_mapping = mappings
            .iter()
            .find(|m| m.solution_config.configuration == "Debug")
            .unwrap();
        assert!(debug_mapping.build);

        // Release should NOT have build enabled (no Build.0 line)
        let release_mapping = mappings
            .iter()
            .find(|m| m.solution_config.configuration == "Release")
            .unwrap();
        assert!(!release_mapping.build);
    }

    #[test]
    fn configuration_type_detection() {
        assert!(ConfigurationType::Application.is_executable());
        assert!(!ConfigurationType::DynamicLibrary.is_executable());
        assert!(!ConfigurationType::StaticLibrary.is_executable());
    }

    #[test]
    fn extract_guid_variations() {
        assert_eq!(extract_guid("{ABC-123}"), Some("ABC-123".to_string()));
        assert_eq!(extract_guid("ABC-123"), Some("ABC-123".to_string()));
        assert_eq!(extract_guid("  {abc-123}  "), Some("ABC-123".to_string()));
        assert_eq!(extract_guid(""), None);
        assert_eq!(extract_guid("{}"), None);
    }
}
