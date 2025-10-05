use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

const WORKSPACE_DIR: &str = ".vedit";
const WORKSPACE_FILE: &str = "workspace.toml";
const WORKSPACE_METADATA_FILE: &str = "metadata.json";
const MAX_RECENT_FILES: usize = 10;
pub const MAX_RECENT_DEBUG_TARGETS: usize = 8;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DebugTargetRecord {
    pub name: String,
    pub executable: String,
}

impl DebugTargetRecord {
    pub fn new(name: impl Into<String>, executable: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            executable: executable.into(),
        }
    }

    fn normalized(name: impl Into<String>, executable: &Path) -> Option<Self> {
        let name = name.into().trim().to_string();
        if name.is_empty() {
            return None;
        }

        let executable = normalize_path(executable);
        if executable.trim().is_empty() {
            return None;
        }

        Some(Self { name, executable })
    }

    fn is_valid(&self) -> bool {
        !self.name.trim().is_empty() && !self.executable.trim().is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceConfig {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub ignored_directories: Vec<String>,
    #[serde(default)]
    recent_files: VecDeque<String>,
    #[serde(default)]
    recent_debug_targets: VecDeque<DebugTargetRecord>,
    #[serde(default)]
    last_debug_target: Option<DebugTargetRecord>,
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            name: None,
            ignored_directories: Vec::new(),
            recent_files: VecDeque::new(),
            recent_debug_targets: VecDeque::new(),
            last_debug_target: None,
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

    pub fn recent_debug_targets(&self) -> impl Iterator<Item = &DebugTargetRecord> {
        self.recent_debug_targets.iter()
    }

    pub fn last_debug_target(&self) -> Option<&DebugTargetRecord> {
        self.last_debug_target.as_ref()
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

    pub fn record_debug_target(&mut self, name: &str, executable: impl AsRef<Path>) -> bool {
        let Some(record) = DebugTargetRecord::normalized(name, executable.as_ref()) else {
            return false;
        };

        let mut changed = false;

        if let Some(position) = self
            .recent_debug_targets
            .iter()
            .position(|entry| entry == &record)
        {
            if position != 0 {
                self.recent_debug_targets.remove(position);
                self.recent_debug_targets.push_front(record.clone());
                changed = true;
            }
        } else {
            self.recent_debug_targets.push_front(record.clone());
            changed = true;
            while self.recent_debug_targets.len() > MAX_RECENT_DEBUG_TARGETS {
                self.recent_debug_targets.pop_back();
            }
        }

        if self.last_debug_target.as_ref() != Some(&record) {
            self.last_debug_target = Some(record);
            changed = true;
        }

        changed
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

        let mut deduped_targets: VecDeque<DebugTargetRecord> = VecDeque::new();
        for entry in self.recent_debug_targets.drain(..) {
            if entry.is_valid() && !deduped_targets.contains(&entry) {
                deduped_targets.push_back(entry);
            }
        }
        while deduped_targets.len() > MAX_RECENT_DEBUG_TARGETS {
            deduped_targets.pop_back();
        }
        self.recent_debug_targets = deduped_targets;

        if let Some(last) = self.last_debug_target.as_ref() {
            if !last.is_valid() {
                self.last_debug_target = None;
            }
        }

        if let Some(last) = self.last_debug_target.clone() {
            if !self.recent_debug_targets.contains(&last) {
                self.recent_debug_targets.push_front(last);
                while self.recent_debug_targets.len() > MAX_RECENT_DEBUG_TARGETS {
                    self.recent_debug_targets.pop_back();
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StickyNoteRecord {
    pub id: u64,
    pub file: String,
    pub line: usize,
    pub column: usize,
    #[serde(default)]
    pub content: String,
}

impl StickyNoteRecord {
    pub fn new(id: u64, file: String, line: usize, column: usize, content: String) -> Self {
        Self {
            id,
            file,
            line,
            column,
            content,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ConsoleWorkspaceState {
    #[serde(default)]
    pub visible: bool,
    #[serde(default)]
    pub shell_tabs: usize,
    #[serde(default)]
    pub active_shell: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct WorkspaceMetadata {
    #[serde(default)]
    pub sticky_notes: Vec<StickyNoteRecord>,
    #[serde(default)]
    pub console: ConsoleWorkspaceState,
}

impl WorkspaceMetadata {
    pub fn load(root: impl AsRef<Path>) -> Result<Self, WorkspaceMetadataError> {
        let path = metadata_path(root);
        let contents = fs::read_to_string(&path)?;
        let metadata: Self = serde_json::from_str(&contents)?;
        Ok(metadata)
    }

    pub fn load_or_default(root: impl AsRef<Path>) -> Result<Self, WorkspaceMetadataError> {
        match Self::load(root) {
            Ok(metadata) => Ok(metadata),
            Err(WorkspaceMetadataError::Io(err)) if err.kind() == io::ErrorKind::NotFound => {
                Ok(Self::default())
            }
            Err(err) => Err(err),
        }
    }

    pub fn save(&self, root: impl AsRef<Path>) -> Result<(), WorkspaceMetadataError> {
        let path = metadata_path(&root);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let contents = serde_json::to_string_pretty(self)?;
        fs::write(&path, contents)?;
        Ok(())
    }

    pub fn notes_for_file(&self, file: &str) -> Vec<StickyNoteRecord> {
        self.sticky_notes
            .iter()
            .filter(|entry| entry.file == file)
            .cloned()
            .collect()
    }

    pub fn set_notes_for_file(&mut self, file: &str, notes: Vec<StickyNoteRecord>) -> bool {
        let existing: Vec<StickyNoteRecord> = self
            .sticky_notes
            .iter()
            .filter(|entry| entry.file == file)
            .cloned()
            .collect();

        if existing == notes {
            return false;
        }

        self.sticky_notes.retain(|entry| entry.file != file);
        self.sticky_notes.extend(notes);
        true
    }

    pub fn remove_file(&mut self, file: &str) -> bool {
        let original_len = self.sticky_notes.len();
        self.sticky_notes.retain(|entry| entry.file != file);
        original_len != self.sticky_notes.len()
    }

    pub fn next_sticky_id(&self) -> u64 {
        let max_id = self
            .sticky_notes
            .iter()
            .map(|entry| entry.id)
            .max()
            .unwrap_or(0);
        let time_based = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_micros() as u64)
            .unwrap_or(0);
        max_id.max(time_based).wrapping_add(1)
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

#[derive(Debug, Error)]
pub enum WorkspaceMetadataError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("Failed to parse workspace metadata: {0}")]
    Parse(#[from] serde_json::Error),
}

fn config_path(root: impl AsRef<Path>) -> PathBuf {
    root.as_ref()
        .join(WORKSPACE_DIR)
        .join(WORKSPACE_FILE)
}

fn metadata_path(root: impl AsRef<Path>) -> PathBuf {
    root.as_ref()
        .join(WORKSPACE_DIR)
        .join(WORKSPACE_METADATA_FILE)
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
    fn record_recent_debug_target_promotes_and_limits() {
        let mut config = WorkspaceConfig::default();
        for idx in 0..10 {
            let exe = format!("/bin/tool{}", idx);
            assert!(config.record_debug_target(&format!("tool{}", idx), &exe));
        }

        assert!(config
            .recent_debug_targets()
            .count()
            <= MAX_RECENT_DEBUG_TARGETS);
        let first = config.recent_debug_targets().next().unwrap();
        assert_eq!(first.name, "tool9");

        assert!(config.record_debug_target("tool3", "/bin/tool3"));
        let new_first = config.recent_debug_targets().next().unwrap();
        assert_eq!(new_first.name, "tool3");
        assert_eq!(config.last_debug_target().unwrap().name, "tool3");
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

    #[test]
    fn workspace_metadata_round_trip() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        let mut metadata = WorkspaceMetadata::default();
        let note = StickyNoteRecord::new(1, "src/lib.rs".into(), 10, 4, "Note".into());
        assert!(metadata.set_notes_for_file("src/lib.rs", vec![note.clone()]));
        metadata.save(root).unwrap();

        let loaded = WorkspaceMetadata::load(root).unwrap();
        assert_eq!(loaded.notes_for_file("src/lib.rs"), vec![note]);

        fs::remove_dir_all(dir).ok();
    }
}
