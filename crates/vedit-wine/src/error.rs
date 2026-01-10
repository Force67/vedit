//! Error types for Wine integration

use thiserror::Error;

/// Wine integration errors
#[derive(Error, Debug, Clone)]
pub enum WineError {
    #[error("Wine is not available on this system")]
    WineNotAvailable,

    #[error("Wine environment not found: {0}")]
    EnvironmentNotFound(String),

    #[error("Failed to create Wine environment: {0}")]
    EnvironmentCreationFailed(String),

    #[error("Wine process not found: {0}")]
    ProcessNotFound(uuid::Uuid),

    #[error("Failed to spawn Wine process: {0}")]
    ProcessSpawnFailed(String),

    #[error("IO error: {0}")]
    IoError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Nix integration error: {0}")]
    NixError(String),

    #[error("Remote desktop error: {0}")]
    RemoteDesktopError(String),

    #[error("Command execution failed: {0}")]
    CommandFailed(String),

    #[error("Invalid Wine prefix: {0}")]
    InvalidPrefix(String),

    #[error("Windows executable not found: {0}")]
    ExecutableNotFound(String),

    #[error("Runtime not installed: {0}")]
    RuntimeNotInstalled(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),
}

impl From<std::io::Error> for WineError {
    fn from(err: std::io::Error) -> Self {
        WineError::IoError(err.to_string())
    }
}

/// Result type for Wine operations
pub type WineResult<T> = Result<T, WineError>;
