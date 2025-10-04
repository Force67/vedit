use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use thiserror::Error;

const WORKSPACE_DIR: &str = ".vedit";
const WORKSPACE_FILE: &str = "workspace.toml";
const MAX_RECENT_FILES: usize = 10;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceConfig {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub ignored_directories: Vec<String>,
    #[serde(default)]
    recent_files: VecDeque<String>,
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            name: None,
            ignored_directories: Vec::new(),
            recent_files: VecDeque::new(),
        }
    }
}

impl WorkspaceConfig {
    pub fn load(root: impl AsRef<Path>) -> Result<Self, WorkspaceConfigError> {
        let path = config_path(root);
        let contents = fs::read_to_string(&path)?;
        let mut config: Self = toml::from_str(&contents)?;
        config.normalize();
        Ok(config)
    }

    pub fn load_or_default(root: impl AsRef<Path>) -> Result<Self, WorkspaceConfigError> {
        match Self::load(root) {
            Ok(config) => Ok(config),
            Err(WorkspaceConfigError::Io(err)) if err.kind() == io::ErrorKind::NotFound => {
                Ok(Self::default())
            }
            Err(err) => Err(err),
        }
    }

    pub fn save(&self, root: impl AsRef<Path>) -> Result<(), WorkspaceConfigError> {
        let path = config_path(&root);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let contents = toml::to_string_pretty(self)?;
        fs::write(&path, contents)?;
        Ok(())
    }

    pub fn ignored_directories(&self) -> impl Iterator<Item = &str> {
        self.ignored_directories.iter().map(|entry| entry.as_str())
    }

    pub fn recent_files(&self) -> impl Iterator<Item = &str> {
        self.recent_files.iter().map(|entry| entry.as_str())
    }

    pub fn record_recent_file(&mut self, file: impl AsRef<Path>) -> bool {
        let file = file.as_ref();
        if file.as_os_str().is_empty() {
            return false;
        }
        let display = normalize_path(file);
        if display.trim().is_empty() {
            return false;
        }

        if let Some(pos) = self.recent_files.iter().position(|entry| entry == &display) {
            if pos == 0 {
                return false;
            }
            self.recent_files.remove(pos);
        }

        self.recent_files.push_front(display);
        while self.recent_files.len() > MAX_RECENT_FILES {
            self.recent_files.pop_back();
        }
        true
    }

    fn normalize(&mut self) {
        self.ignored_directories
            .iter_mut()
            .for_each(|entry| *entry = entry.trim().to_string());
        self.ignored_directories
            .retain(|entry| !entry.trim().is_empty());

        let mut deduped = VecDeque::new();
        for entry in self.recent_files.drain(..) {
            if !entry.trim().is_empty() && !deduped.contains(&entry) {
                deduped.push_back(entry);
            }
        }
        self.recent_files = deduped;
    }
}

#[derive(Debug, Error)]
pub enum WorkspaceConfigError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("Failed to parse workspace configuration: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("Failed to serialize workspace configuration: {0}")]
    Serialize(#[from] toml::ser::Error),
}

fn config_path(root: impl AsRef<Path>) -> PathBuf {
    root.as_ref()
        .join(WORKSPACE_DIR)
        .join(WORKSPACE_FILE)
}

fn normalize_path(path: &Path) -> String {
    let display = path.to_string_lossy().to_string();
    if cfg!(windows) {
        display.replace('\\', "/")
    } else {
        display
    }
}

impl fmt::Display for WorkspaceConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WorkspaceConfig(name={:?})", self.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn record_recent_file_promotes_and_limits() {
        let mut config = WorkspaceConfig::default();
        for idx in 0..12 {
            config.record_recent_file(format!("file{}", idx));
        }

        assert!(config.recent_files().count() <= MAX_RECENT_FILES);
        assert_eq!(config.recent_files().next().unwrap(), "file11");

        assert!(config.record_recent_file("file5"));
        assert_eq!(config.recent_files().next().unwrap(), "file5");
    }

    #[test]
    fn load_and_save_round_trip() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        let mut config = WorkspaceConfig::default();
        config.name = Some("My Project".into());
        config.ignored_directories = vec!["build".into(), "tmp".into()];
        config.record_recent_file("src/main.rs");
        config.save(root).unwrap();

        let loaded = WorkspaceConfig::load(root).unwrap();
        assert_eq!(loaded.name, Some("My Project".into()));
        assert_eq!(loaded.ignored_directories.len(), 2);
        assert_eq!(loaded.recent_files().next().unwrap(), "src/main.rs");

        fs::remove_dir_all(dir).ok();
    }
}
