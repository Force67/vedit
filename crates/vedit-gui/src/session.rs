use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;

/// Application configuration directory following XDG Base Directory Specification
pub fn get_app_config_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("vedit"))
}

/// Ensure the config directory exists
pub fn ensure_config_dir() -> io::Result<PathBuf> {
    let config_dir = get_app_config_dir()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Could not find config directory"))?;

    if !config_dir.exists() {
        fs::create_dir_all(&config_dir)?;
    }

    Ok(config_dir)
}

/// Window state information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowState {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub maximized: bool,
    pub monitor: Option<u32>,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            x: 100,
            y: 100,
            width: 1200,
            height: 800,
            maximized: false,
            monitor: None,
        }
    }
}

/// Workspace state information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceState {
    pub last_folder: Option<PathBuf>,
    pub open_files: Vec<PathBuf>,
    pub active_file_index: Option<usize>,
    pub workspace_root: Option<PathBuf>,
}

impl Default for WorkspaceState {
    fn default() -> Self {
        Self {
            last_folder: None,
            open_files: Vec::new(),
            active_file_index: None,
            workspace_root: None,
        }
    }
}

/// Complete session state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    pub window: WindowState,
    pub workspace: WorkspaceState,
}

impl Default for SessionState {
    fn default() -> Self {
        Self {
            window: WindowState::default(),
            workspace: WorkspaceState::default(),
        }
    }
}

/// Session storage manager
#[derive(Clone)]
pub struct SessionManager {
    pub config_dir: PathBuf,
}

impl SessionManager {
    /// Create a new session manager
    pub fn new() -> io::Result<Self> {
        let config_dir = ensure_config_dir()?;
        Ok(Self { config_dir })
    }

    /// Create a session manager with a specific config directory
    pub fn with_config_dir(config_dir: PathBuf) -> Self {
        Self { config_dir }
    }

    /// Get path to window state file
    pub fn window_state_path(&self) -> PathBuf {
        self.config_dir.join("window_state.toml")
    }

    /// Get path to workspace state file
    pub fn workspace_state_path(&self) -> PathBuf {
        self.config_dir.join("workspace_state.toml")
    }

    /// Get path to complete session file
    pub fn session_state_path(&self) -> PathBuf {
        self.config_dir.join("session.toml")
    }

    /// Save window state
    pub fn save_window_state(&self, state: &WindowState) -> io::Result<()> {
        let toml_string = toml::to_string_pretty(state)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        fs::write(self.window_state_path(), toml_string)
    }

    /// Load window state
    pub fn load_window_state(&self) -> io::Result<WindowState> {
        let path = self.window_state_path();
        if !path.exists() {
            return Ok(WindowState::default());
        }

        let content = fs::read_to_string(path)?;
        toml::from_str(&content)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    /// Save workspace state
    pub fn save_workspace_state(&self, state: &WorkspaceState) -> io::Result<()> {
        let toml_string = toml::to_string_pretty(state)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        fs::write(self.workspace_state_path(), toml_string)
    }

    /// Load workspace state
    pub fn load_workspace_state(&self) -> io::Result<WorkspaceState> {
        let path = self.workspace_state_path();
        if !path.exists() {
            return Ok(WorkspaceState::default());
        }

        let content = fs::read_to_string(path)?;
        toml::from_str(&content)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    /// Save complete session state
    pub fn save_session_state(&self, state: &SessionState) -> io::Result<()> {
        let toml_string = toml::to_string_pretty(state)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        fs::write(self.session_state_path(), toml_string)
    }

    /// Load complete session state
    pub fn load_session_state(&self) -> io::Result<SessionState> {
        let path = self.session_state_path();
        if !path.exists() {
            return Ok(SessionState::default());
        }

        let content = fs::read_to_string(path)?;
        toml::from_str(&content)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    /// Clear all session data
    pub fn clear_session(&self) -> io::Result<()> {
        let paths = [
            self.window_state_path(),
            self.workspace_state_path(),
            self.session_state_path(),
        ];

        for path in &paths {
            if path.exists() {
                fs::remove_file(path)?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_window_state_serialization() {
        let state = WindowState {
            x: 100,
            y: 200,
            width: 1920,
            height: 1080,
            maximized: true,
            monitor: Some(0),
        };

        let toml_string = toml::to_string_pretty(&state).unwrap();
        let deserialized: WindowState = toml::from_str(&toml_string).unwrap();

        assert_eq!(state.x, deserialized.x);
        assert_eq!(state.y, deserialized.y);
        assert_eq!(state.width, deserialized.width);
        assert_eq!(state.height, deserialized.height);
        assert_eq!(state.maximized, deserialized.maximized);
        assert_eq!(state.monitor, deserialized.monitor);
    }

    #[test]
    fn test_workspace_state_serialization() {
        let state = WorkspaceState {
            last_folder: Some(PathBuf::from("/home/user/project")),
            open_files: vec![
                PathBuf::from("/home/user/project/src/main.rs"),
                PathBuf::from("/home/user/project/src/lib.rs"),
            ],
            active_file_index: Some(0),
            workspace_root: Some(PathBuf::from("/home/user/project")),
        };

        let toml_string = toml::to_string_pretty(&state).unwrap();
        let deserialized: WorkspaceState = toml::from_str(&toml_string).unwrap();

        assert_eq!(state.last_folder, deserialized.last_folder);
        assert_eq!(state.open_files, deserialized.open_files);
        assert_eq!(state.active_file_index, deserialized.active_file_index);
        assert_eq!(state.workspace_root, deserialized.workspace_root);
    }
}