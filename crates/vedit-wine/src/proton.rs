//! Proton detection and management
//!
//! This module provides support for detecting and managing Proton installations
//! from Steam and other sources (e.g., GloriousEggroll, custom builds).

use crate::error::{WineError, WineResult};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Represents a detected Proton installation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtonInstallation {
    /// Human-readable name (e.g., "Proton 8.0-5", "GE-Proton8-25")
    pub name: String,

    /// Path to the Proton installation directory
    pub path: PathBuf,

    /// Detected version information
    pub version: ProtonVersion,

    /// Where this installation was found
    pub source: ProtonSource,

    /// Path to the proton executable script
    pub proton_executable: PathBuf,

    /// Path to the Wine binary within this Proton installation
    pub wine_executable: PathBuf,
}

/// Source of a Proton installation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProtonSource {
    /// Official Steam Proton (from steamapps/common)
    SteamOfficial,

    /// User-installed compatibility tools (from compatibilitytools.d)
    CompatTools,

    /// GloriousEggroll custom Proton builds
    GloriousEggroll,

    /// User-specified custom path
    Custom(PathBuf),
}

impl std::fmt::Display for ProtonSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProtonSource::SteamOfficial => write!(f, "Steam Official"),
            ProtonSource::CompatTools => write!(f, "Compatibility Tools"),
            ProtonSource::GloriousEggroll => write!(f, "GE-Proton"),
            ProtonSource::Custom(path) => write!(f, "Custom ({})", path.display()),
        }
    }
}

/// Proton version information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProtonVersion {
    /// Official Valve Proton version
    Official {
        major: u32,
        minor: u32,
        patch: Option<u32>,
    },

    /// GloriousEggroll version
    GE { version: String },

    /// Experimental Proton
    Experimental,

    /// Unknown/unparseable version
    Unknown,
}

impl std::fmt::Display for ProtonVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProtonVersion::Official {
                major,
                minor,
                patch: Some(p),
            } => write!(f, "{}.{}-{}", major, minor, p),
            ProtonVersion::Official {
                major,
                minor,
                patch: None,
            } => write!(f, "{}.{}", major, minor),
            ProtonVersion::GE { version } => write!(f, "GE-{}", version),
            ProtonVersion::Experimental => write!(f, "Experimental"),
            ProtonVersion::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Manager for Proton installations
pub struct ProtonManager {
    /// All detected installations
    installations: Vec<ProtonInstallation>,

    /// Custom search paths provided by user
    custom_paths: Vec<PathBuf>,
}

impl ProtonManager {
    /// Create a new ProtonManager and detect all available installations
    pub fn detect() -> WineResult<Self> {
        let mut manager = Self {
            installations: Vec::new(),
            custom_paths: Vec::new(),
        };

        manager.refresh()?;
        Ok(manager)
    }

    /// Create a ProtonManager with additional custom search paths
    pub fn with_custom_paths(custom_paths: Vec<PathBuf>) -> WineResult<Self> {
        let mut manager = Self {
            installations: Vec::new(),
            custom_paths,
        };

        manager.refresh()?;
        Ok(manager)
    }

    /// Refresh the list of detected installations
    pub fn refresh(&mut self) -> WineResult<()> {
        self.installations.clear();

        // Detect from all sources
        self.installations.extend(Self::detect_steam_official());
        self.installations.extend(Self::detect_compat_tools());

        // Detect from custom paths
        for path in &self.custom_paths.clone() {
            if let Some(installation) =
                Self::detect_at_path(path, ProtonSource::Custom(path.clone()))
            {
                self.installations.push(installation);
            }
        }

        // Sort by version (newest first)
        self.installations.sort_by(|a, b| {
            // Prefer experimental, then by version number
            match (&a.version, &b.version) {
                (ProtonVersion::Experimental, ProtonVersion::Experimental) => {
                    std::cmp::Ordering::Equal
                }
                (ProtonVersion::Experimental, _) => std::cmp::Ordering::Less,
                (_, ProtonVersion::Experimental) => std::cmp::Ordering::Greater,
                (
                    ProtonVersion::Official {
                        major: am,
                        minor: an,
                        ..
                    },
                    ProtonVersion::Official {
                        major: bm,
                        minor: bn,
                        ..
                    },
                ) => (bm, bn).cmp(&(am, an)),
                (ProtonVersion::GE { version: a }, ProtonVersion::GE { version: b }) => b.cmp(a),
                (ProtonVersion::GE { .. }, _) => std::cmp::Ordering::Less,
                (_, ProtonVersion::GE { .. }) => std::cmp::Ordering::Greater,
                _ => std::cmp::Ordering::Equal,
            }
        });

        tracing::info!("Detected {} Proton installations", self.installations.len());
        Ok(())
    }

    /// Get all detected installations
    pub fn installations(&self) -> &[ProtonInstallation] {
        &self.installations
    }

    /// Find an installation by name
    pub fn find_by_name(&self, name: &str) -> Option<&ProtonInstallation> {
        self.installations.iter().find(|i| i.name == name)
    }

    /// Find an installation by path
    pub fn find_by_path(&self, path: &Path) -> Option<&ProtonInstallation> {
        self.installations.iter().find(|i| i.path == path)
    }

    /// Add a custom search path
    pub fn add_custom_path(&mut self, path: PathBuf) -> WineResult<Option<ProtonInstallation>> {
        if !self.custom_paths.contains(&path) {
            self.custom_paths.push(path.clone());
        }

        if let Some(installation) = Self::detect_at_path(&path, ProtonSource::Custom(path.clone()))
        {
            if !self
                .installations
                .iter()
                .any(|i| i.path == installation.path)
            {
                self.installations.push(installation.clone());
                return Ok(Some(installation));
            }
        }

        Ok(None)
    }

    /// Detect official Steam Proton installations
    fn detect_steam_official() -> Vec<ProtonInstallation> {
        let mut installations = Vec::new();

        // Common Steam library paths
        let steam_paths = Self::get_steam_library_paths();

        for steam_path in steam_paths {
            let common_path = steam_path.join("steamapps").join("common");

            if !common_path.exists() {
                continue;
            }

            // Look for Proton* directories
            if let Ok(entries) = std::fs::read_dir(&common_path) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        if name.starts_with("Proton") {
                            if let Some(installation) =
                                Self::detect_at_path(&path, ProtonSource::SteamOfficial)
                            {
                                installations.push(installation);
                            }
                        }
                    }
                }
            }
        }

        installations
    }

    /// Detect installations from compatibility tools directories
    fn detect_compat_tools() -> Vec<ProtonInstallation> {
        let mut installations = Vec::new();

        let compat_dirs = Self::get_compat_tools_paths();

        for compat_dir in compat_dirs {
            if !compat_dir.exists() {
                continue;
            }

            if let Ok(entries) = std::fs::read_dir(&compat_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        // Determine if this is GE-Proton or regular
                        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                        let source = if name.contains("GE-Proton") || name.contains("Proton-GE") {
                            ProtonSource::GloriousEggroll
                        } else {
                            ProtonSource::CompatTools
                        };

                        if let Some(installation) = Self::detect_at_path(&path, source) {
                            installations.push(installation);
                        }
                    }
                }
            }
        }

        installations
    }

    /// Attempt to detect a Proton installation at a specific path
    fn detect_at_path(path: &Path, source: ProtonSource) -> Option<ProtonInstallation> {
        if !path.is_dir() {
            return None;
        }

        // Look for proton script
        let proton_executable = path.join("proton");
        if !proton_executable.exists() {
            return None;
        }

        // Look for wine binary (could be in different locations)
        let wine_locations = [
            path.join("files").join("bin").join("wine64"),
            path.join("files").join("bin").join("wine"),
            path.join("dist").join("bin").join("wine64"),
            path.join("dist").join("bin").join("wine"),
        ];

        let wine_executable = wine_locations.into_iter().find(|p| p.exists())?;

        // Parse version from directory name
        let dir_name = path.file_name()?.to_str()?;
        let version = Self::parse_version(dir_name);

        // Generate human-readable name
        let name = dir_name.to_string();

        Some(ProtonInstallation {
            name,
            path: path.to_path_buf(),
            version,
            source,
            proton_executable,
            wine_executable,
        })
    }

    /// Parse version from directory name
    fn parse_version(name: &str) -> ProtonVersion {
        // Check for experimental
        if name.contains("Experimental") {
            return ProtonVersion::Experimental;
        }

        // Check for GE-Proton (e.g., "GE-Proton8-25", "Proton-GE-8-25")
        if name.contains("GE-Proton") || name.contains("Proton-GE") {
            let version = name
                .replace("GE-Proton", "")
                .replace("Proton-GE-", "")
                .replace("Proton-GE", "")
                .trim_start_matches('-')
                .to_string();
            return ProtonVersion::GE { version };
        }

        // Parse official Proton version (e.g., "Proton 8.0", "Proton - 8.0-5")
        let version_str = name
            .replace("Proton", "")
            .replace("-", " ")
            .trim()
            .to_string();

        // Try to parse as "major.minor" or "major.minor-patch"
        let parts: Vec<&str> = version_str.split_whitespace().collect();
        if let Some(version_part) = parts.first() {
            let nums: Vec<&str> = version_part.split('.').collect();
            if nums.len() >= 2 {
                if let (Ok(major), Ok(minor_patch)) =
                    (nums[0].parse::<u32>(), nums[1].parse::<String>())
                {
                    // Check if minor has a patch suffix (e.g., "0-5")
                    let minor_parts: Vec<&str> = minor_patch.split('-').collect();
                    if let Ok(minor) = minor_parts[0].parse::<u32>() {
                        let patch = minor_parts.get(1).and_then(|p| p.parse::<u32>().ok());
                        return ProtonVersion::Official {
                            major,
                            minor,
                            patch,
                        };
                    }
                }
            }
        }

        ProtonVersion::Unknown
    }

    /// Get Steam library paths
    fn get_steam_library_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();

        if let Some(home) = dirs::home_dir() {
            // Default Steam installation paths
            let default_paths = [
                home.join(".steam").join("steam"),
                home.join(".steam").join("root"),
                home.join(".local").join("share").join("Steam"),
            ];

            for path in default_paths {
                if path.exists() {
                    paths.push(path.clone());

                    // Also check for additional library folders via libraryfolders.vdf
                    let library_folders = path.join("steamapps").join("libraryfolders.vdf");
                    if library_folders.exists() {
                        if let Ok(content) = std::fs::read_to_string(&library_folders) {
                            // Simple VDF parsing - look for "path" entries
                            for line in content.lines() {
                                if let Some(path_start) = line.find("\"path\"") {
                                    let remaining = &line[path_start + 7..];
                                    if let Some(start) = remaining.find('"') {
                                        let remaining = &remaining[start + 1..];
                                        if let Some(end) = remaining.find('"') {
                                            let lib_path = PathBuf::from(&remaining[..end]);
                                            if lib_path.exists() && !paths.contains(&lib_path) {
                                                paths.push(lib_path);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        paths
    }

    /// Get compatibility tools directories
    fn get_compat_tools_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();

        if let Some(home) = dirs::home_dir() {
            let compat_paths = [
                home.join(".steam")
                    .join("root")
                    .join("compatibilitytools.d"),
                home.join(".steam")
                    .join("steam")
                    .join("compatibilitytools.d"),
                home.join(".local")
                    .join("share")
                    .join("Steam")
                    .join("compatibilitytools.d"),
            ];

            for path in compat_paths {
                if path.exists() && !paths.contains(&path) {
                    paths.push(path);
                }
            }
        }

        paths
    }
}

impl ProtonInstallation {
    /// Get the Wine prefix directory for a given project
    pub fn prefix_path_for_project(&self, project_path: &Path, env_id: &str) -> PathBuf {
        project_path
            .join(".wine")
            .join(format!("proton-{}", env_id))
    }

    /// Get environment variables needed to run Wine through this Proton
    pub fn get_env_vars(&self, prefix_path: &Path) -> std::collections::HashMap<String, String> {
        let mut env = std::collections::HashMap::new();

        // Basic Wine environment
        env.insert(
            "WINEPREFIX".to_string(),
            prefix_path.to_string_lossy().to_string(),
        );
        env.insert("WINEARCH".to_string(), "win64".to_string());

        // Proton-specific
        env.insert("PROTON_NO_ESYNC".to_string(), "1".to_string());
        env.insert("PROTON_NO_FSYNC".to_string(), "1".to_string());

        // Add Proton's bin to PATH
        if let Some(bin_dir) = self.wine_executable.parent() {
            if let Ok(current_path) = std::env::var("PATH") {
                env.insert(
                    "PATH".to_string(),
                    format!("{}:{}", bin_dir.display(), current_path),
                );
            } else {
                env.insert("PATH".to_string(), bin_dir.to_string_lossy().to_string());
            }
        }

        // Add Proton's lib directories
        let lib_paths = [
            self.path.join("files").join("lib"),
            self.path.join("files").join("lib64"),
            self.path.join("dist").join("lib"),
            self.path.join("dist").join("lib64"),
        ];

        let existing_libs: Vec<String> = lib_paths
            .iter()
            .filter(|p| p.exists())
            .map(|p| p.to_string_lossy().to_string())
            .collect();

        if !existing_libs.is_empty() {
            let lib_path = existing_libs.join(":");
            if let Ok(current) = std::env::var("LD_LIBRARY_PATH") {
                env.insert(
                    "LD_LIBRARY_PATH".to_string(),
                    format!("{}:{}", lib_path, current),
                );
            } else {
                env.insert("LD_LIBRARY_PATH".to_string(), lib_path);
            }
        }

        env
    }

    /// Check if this installation appears to be functional
    pub fn verify(&self) -> WineResult<()> {
        if !self.path.exists() {
            return Err(WineError::ProtonNotFound(format!(
                "Installation path does not exist: {}",
                self.path.display()
            )));
        }

        if !self.proton_executable.exists() {
            return Err(WineError::ProtonNotFound(format!(
                "Proton executable not found: {}",
                self.proton_executable.display()
            )));
        }

        if !self.wine_executable.exists() {
            return Err(WineError::ProtonNotFound(format!(
                "Wine executable not found: {}",
                self.wine_executable.display()
            )));
        }

        Ok(())
    }
}

/// Result of environment discovery
#[derive(Debug, Clone, Default)]
pub struct EnvironmentDiscovery {
    /// System Wine installation path (if available)
    pub system_wine: Option<PathBuf>,

    /// Detected Proton installations
    pub proton_installations: Vec<ProtonInstallation>,

    /// User-configured custom paths
    pub custom_paths: Vec<PathBuf>,
}

impl EnvironmentDiscovery {
    /// Perform full environment discovery
    pub fn detect() -> Self {
        let system_wine = which::which("wine").ok();

        let proton_installations = ProtonManager::detect()
            .map(|m| m.installations.clone())
            .unwrap_or_default();

        Self {
            system_wine,
            proton_installations,
            custom_paths: Vec::new(),
        }
    }

    /// Check if any Wine/Proton environment is available
    pub fn has_any(&self) -> bool {
        self.system_wine.is_some() || !self.proton_installations.is_empty()
    }

    /// Get total number of available environments
    pub fn count(&self) -> usize {
        let wine_count = if self.system_wine.is_some() { 1 } else { 0 };
        wine_count + self.proton_installations.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_version_official() {
        assert!(matches!(
            ProtonManager::parse_version("Proton 8.0"),
            ProtonVersion::Official {
                major: 8,
                minor: 0,
                patch: None
            }
        ));

        assert!(matches!(
            ProtonManager::parse_version("Proton - 8.0-5"),
            ProtonVersion::Official {
                major: 8,
                minor: 0,
                patch: Some(5)
            }
        ));
    }

    #[test]
    fn test_parse_version_ge() {
        match ProtonManager::parse_version("GE-Proton8-25") {
            ProtonVersion::GE { version } => assert_eq!(version, "8-25"),
            _ => panic!("Expected GE version"),
        }
    }

    #[test]
    fn test_parse_version_experimental() {
        assert!(matches!(
            ProtonManager::parse_version("Proton Experimental"),
            ProtonVersion::Experimental
        ));
    }
}
