use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use crate::console::{ConsoleLineKind, ConsoleState};

static EDITOR_LOGGER: std::sync::OnceLock<std::sync::Arc<Mutex<Option<EditorLogger>>>> = std::sync::OnceLock::new();
static mut CONSOLE_STATE: Option<*mut ConsoleState> = None;

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: u64,
    pub level: LogLevel,
    pub category: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
}

impl LogLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warning => "WARN",
            LogLevel::Error => "ERROR",
        }
    }

    pub fn as_console_line_kind(&self) -> ConsoleLineKind {
        match self {
            LogLevel::Debug => ConsoleLineKind::Info,
            LogLevel::Info => ConsoleLineKind::Info,
            LogLevel::Warning => ConsoleLineKind::Info,
            LogLevel::Error => ConsoleLineKind::Error,
        }
    }
}

pub struct EditorLogger {
    log_entries: Vec<LogEntry>,
    max_entries: usize,
}

impl EditorLogger {
    pub fn new() -> Self {
        Self {
            log_entries: Vec::new(),
            max_entries: 5000, // Keep last 5000 log entries
        }
    }

    pub fn log(&mut self, level: LogLevel, category: &str, message: &str) {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let entry = LogEntry {
            timestamp,
            level,
            category: category.to_string(),
            message: message.to_string(),
        };

        // Add to in-memory log
        self.log_entries.push(entry.clone());
        if self.log_entries.len() > self.max_entries {
            self.log_entries.drain(0..self.log_entries.len() - self.max_entries);
        }

        // Send to console if available
        unsafe {
            if let Some(console_state_ptr) = CONSOLE_STATE {
                if let Some(console_state) = console_state_ptr.as_mut() {
                    let formatted_message = self.format_log_entry(&entry);
                    console_state.log_to_editor(level.as_console_line_kind(), formatted_message);
                }
            }
        }
    }

    fn format_log_entry(&self, entry: &LogEntry) -> String {
        let time_str = chrono::DateTime::from_timestamp(entry.timestamp as i64, 0)
            .map(|dt| dt.format("%H:%M:%S").to_string())
            .unwrap_or_else(|| "??:??:??".to_string());

        format!("[{} {}:{}] {}", time_str, entry.level.as_str(), entry.category, entry.message)
    }

    pub fn get_recent_logs(&self, limit: Option<usize>) -> &[LogEntry] {
        let start = if let Some(limit) = limit {
            if self.log_entries.len() > limit {
                self.log_entries.len() - limit
            } else {
                0
            }
        } else {
            0
        };
        &self.log_entries[start..]
    }
}

// Global logging functions
pub fn init_logger() {
    let logger = EDITOR_LOGGER.get_or_init(|| Arc::new(Mutex::new(None)));
    let mut logger_guard = logger.lock().unwrap();
    *logger_guard = Some(EditorLogger::new());
}

pub fn set_console_state(console_state: *mut ConsoleState) {
    unsafe {
        CONSOLE_STATE = Some(console_state);
    }
}

pub fn log_debug(category: &str, message: &str) {
    if let Some(logger) = EDITOR_LOGGER.get() {
        if let Some(ref mut logger_instance) = logger.lock().unwrap().as_mut() {
            logger_instance.log(LogLevel::Debug, category, message);
        }
    }
}

pub fn log_info(category: &str, message: &str) {
    if let Some(logger) = EDITOR_LOGGER.get() {
        if let Some(ref mut logger_instance) = logger.lock().unwrap().as_mut() {
            logger_instance.log(LogLevel::Info, category, message);
        }
    }
}

pub fn log_warning(category: &str, message: &str) {
    if let Some(logger) = EDITOR_LOGGER.get() {
        if let Some(ref mut logger_instance) = logger.lock().unwrap().as_mut() {
            logger_instance.log(LogLevel::Warning, category, message);
        }
    }
}

pub fn log_error(category: &str, message: &str) {
    if let Some(logger) = EDITOR_LOGGER.get() {
        if let Some(ref mut logger_instance) = logger.lock().unwrap().as_mut() {
            logger_instance.log(LogLevel::Error, category, message);
        }
    }
}

// Convenience macro for logging
#[macro_export]
macro_rules! editor_log_debug {
    ($category:expr, $($arg:tt)*) => {
        $crate::editor_log::log_debug($category, &format!($($arg)*))
    };
}

#[macro_export]
macro_rules! editor_log_info {
    ($category:expr, $($arg:tt)*) => {
        $crate::editor_log::log_info($category, &format!($($arg)*))
    };
}

#[macro_export]
macro_rules! editor_log_warning {
    ($category:expr, $($arg:tt)*) => {
        $crate::editor_log::log_warning($category, &format!($($arg)*))
    };
}

#[macro_export]
macro_rules! editor_log_error {
    ($category:expr, $($arg:tt)*) => {
        $crate::editor_log::log_error($category, &format!($($arg)*))
    };
}