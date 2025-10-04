use serde::Deserialize;
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

/// Identifier used for the quick command menu toggle.
pub const QUICK_COMMAND_MENU_ACTION: &str = "quick_command_menu.toggle";
pub const SAVE_ACTION: &str = "file.save";

/// Logical key identifier supported by keybindings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    Character(char),
    Function(u8),
    Escape,
    Enter,
    Tab,
    Space,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Backspace,
}

impl Key {
    fn parse(value: &str) -> Result<Self, ParseKeyCombinationError> {
        let normalized = value.trim();
        match normalized.to_ascii_uppercase().as_str() {
            "ESC" | "ESCAPE" => Ok(Self::Escape),
            "ENTER" | "RETURN" => Ok(Self::Enter),
            "TAB" => Ok(Self::Tab),
            "SPACE" | "SPACEBAR" => Ok(Self::Space),
            "UP" | "ARROWUP" => Ok(Self::ArrowUp),
            "DOWN" | "ARROWDOWN" => Ok(Self::ArrowDown),
            "LEFT" | "ARROWLEFT" => Ok(Self::ArrowLeft),
            "RIGHT" | "ARROWRIGHT" => Ok(Self::ArrowRight),
            "BACKSPACE" | "BKSP" => Ok(Self::Backspace),
            other if other.starts_with('F') && other.len() <= 3 => {
                let number = other[1..]
                    .parse::<u8>()
                    .map_err(|_| ParseKeyCombinationError::UnknownKey(value.to_string()))?;
                if (1..=24).contains(&number) {
                    Ok(Self::Function(number))
                } else {
                    Err(ParseKeyCombinationError::UnknownKey(value.to_string()))
                }
            }
            other if other.len() == 1 => Ok(Self::Character(other.chars().next().unwrap())),
            other => Err(ParseKeyCombinationError::UnknownKey(other.to_string())),
        }
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Key::Character(ch) => write!(f, "{}", ch.to_ascii_uppercase()),
            Key::Function(value) => write!(f, "F{}", value),
            Key::Escape => write!(f, "Esc"),
            Key::Enter => write!(f, "Enter"),
            Key::Tab => write!(f, "Tab"),
            Key::Space => write!(f, "Space"),
            Key::ArrowUp => write!(f, "ArrowUp"),
            Key::ArrowDown => write!(f, "ArrowDown"),
            Key::ArrowLeft => write!(f, "ArrowLeft"),
            Key::ArrowRight => write!(f, "ArrowRight"),
            Key::Backspace => write!(f, "Backspace"),
        }
    }
}

/// Representation of a key activation with associated modifier state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub command: bool,
    pub key: Key,
}

impl KeyEvent {
    pub fn new(key: Key, ctrl: bool, shift: bool, alt: bool, command: bool) -> Self {
        Self {
            ctrl,
            shift,
            alt,
            command,
            key,
        }
    }
}

/// Combination describing a shortcut that can be bound to an action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyCombination {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub command: bool,
    pub key: Key,
}

impl KeyCombination {
    pub fn matches(&self, event: &KeyEvent) -> bool {
        self.ctrl == event.ctrl
            && self.shift == event.shift
            && self.alt == event.alt
            && self.command == event.command
            && self.key == event.key
    }

    pub fn parse(spec: &str) -> Result<Self, ParseKeyCombinationError> {
        let mut ctrl = false;
        let mut shift = false;
        let mut alt = false;
        let mut command = false;
        let mut key: Option<Key> = None;

        for part in spec.split('+') {
            let trimmed = part.trim();
            if trimmed.is_empty() {
                continue;
            }

            match trimmed.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => ctrl = true,
                "shift" => shift = true,
                "alt" | "option" => alt = true,
                "cmd" | "command" | "super" | "meta" => command = true,
                _ => {
                    if key.is_some() {
                        return Err(ParseKeyCombinationError::MultipleKeys(spec.to_string()));
                    }
                    key = Some(Key::parse(trimmed)?);
                }
            }
        }

        let key = key.ok_or_else(|| ParseKeyCombinationError::MissingKey(spec.to_string()))?;

        Ok(Self {
            ctrl,
            shift,
            alt,
            command,
            key,
        })
    }
}

impl fmt::Display for KeyCombination {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        if self.ctrl {
            write!(f, "Ctrl")?;
            first = false;
        }
        if self.shift {
            if !first {
                write!(f, "+")?;
            }
            write!(f, "Shift")?;
            first = false;
        }
        if self.alt {
            if !first {
                write!(f, "+")?;
            }
            write!(f, "Alt")?;
            first = false;
        }
        if self.command {
            if !first {
                write!(f, "+")?;
            }
            write!(f, "Cmd")?;
            first = false;
        }
        if !first {
            write!(f, "+")?;
        }
        write!(f, "{}", self.key)
    }
}

/// Error raised while parsing a key combination specification.
#[derive(Debug)]
pub enum ParseKeyCombinationError {
    MissingKey(String),
    UnknownKey(String),
    MultipleKeys(String),
}

impl fmt::Display for ParseKeyCombinationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingKey(src) => write!(f, "No key specified in binding: {}", src),
            Self::UnknownKey(key) => write!(f, "Unknown key '{}'", key),
            Self::MultipleKeys(src) => write!(f, "Multiple keys specified in binding: {}", src),
        }
    }
}

impl std::error::Error for ParseKeyCombinationError {}

/// Keymap describing the mapping between action identifiers and shortcuts.
#[derive(Debug, Clone)]
pub struct Keymap {
    bindings: HashMap<String, KeyCombination>,
}

impl Default for Keymap {
    fn default() -> Self {
        let mut bindings = HashMap::new();
        bindings.insert(
            QUICK_COMMAND_MENU_ACTION.to_string(),
            KeyCombination {
                ctrl: true,
                shift: true,
                alt: false,
                command: false,
                key: Key::Character('P'),
            },
        );
        bindings.insert(
            SAVE_ACTION.to_string(),
            KeyCombination {
                ctrl: cfg!(not(target_os = "macos")),
                shift: false,
                alt: false,
                command: cfg!(target_os = "macos"),
                key: Key::Character('S'),
            },
        );
        Self { bindings }
    }
}

impl Keymap {
    pub fn binding(&self, action: &str) -> Option<&KeyCombination> {
        self.bindings.get(action)
    }

    pub fn merge(&mut self, other: Keymap) {
        self.bindings.extend(other.bindings);
    }

    pub fn set_binding(&mut self, action: impl Into<String>, combination: Option<KeyCombination>) {
        let action = action.into();
        if let Some(combination) = combination {
            self.bindings.insert(action, combination);
        } else {
            self.bindings.remove(&action);
        }
    }

    pub fn load_from_file(path: impl AsRef<Path>) -> Result<Self, KeymapError> {
        let contents = fs::read_to_string(path.as_ref())?;
        Self::from_toml_str(&contents)
    }

    pub fn from_toml_str(toml_src: &str) -> Result<Self, KeymapError> {
        let parsed: RawKeymap = toml::from_str(toml_src)?;
        let mut bindings = HashMap::new();

        for (action, spec) in parsed.bindings.into_iter() {
            let combination = KeyCombination::parse(&spec)
                .map_err(|err| KeymapError::Parse { action: action.clone(), source: err })?;
            bindings.insert(action, combination);
        }

        Ok(Self { bindings })
    }

    pub fn bindings(&self) -> &HashMap<String, KeyCombination> {
        &self.bindings
    }
}

#[derive(Debug, Deserialize)]
struct RawKeymap {
    #[serde(default)]
    bindings: HashMap<String, String>,
}

impl Default for RawKeymap {
    fn default() -> Self {
        Self {
            bindings: HashMap::new(),
        }
    }
}

/// Errors that can occur while loading key bindings from disk.
#[derive(Debug)]
pub enum KeymapError {
    Io(io::Error),
    Toml(toml::de::Error),
    Parse { action: String, source: ParseKeyCombinationError },
}

impl fmt::Display for KeymapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "Failed to read keymap: {}", err),
            Self::Toml(err) => write!(f, "Failed to parse keymap TOML: {}", err),
            Self::Parse { action, source } => write!(f, "Invalid binding for '{}': {}", action, source),
        }
    }
}

impl std::error::Error for KeymapError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::Toml(err) => Some(err),
            Self::Parse { source, .. } => Some(source),
        }
    }
}

impl From<io::Error> for KeymapError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<toml::de::Error> for KeymapError {
    fn from(value: toml::de::Error) -> Self {
        Self::Toml(value)
    }
}
