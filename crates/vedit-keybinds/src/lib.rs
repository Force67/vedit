use serde::{Deserialize, Serialize};
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
    Delete,
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
            "DELETE" | "DEL" => Ok(Self::Delete),
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
            Key::Delete => write!(f, "Delete"),
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
        bindings.insert(
            "command_palette.toggle".to_string(),
            KeyCombination {
                ctrl: cfg!(not(target_os = "macos")),
                shift: false,
                alt: false,
                command: cfg!(target_os = "macos"),
                key: Key::Character('P'),
            },
        );
        bindings.insert(
            "sidebar.toggle".to_string(),
            KeyCombination {
                ctrl: cfg!(not(target_os = "macos")),
                shift: false,
                alt: false,
                command: cfg!(target_os = "macos"),
                key: Key::Character('B'),
            },
        );
        bindings.insert(
            "terminal.toggle".to_string(),
            KeyCombination {
                ctrl: cfg!(not(target_os = "macos")),
                shift: false,
                alt: false,
                command: cfg!(target_os = "macos"),
                key: Key::Character('J'),
            },
        );
        bindings.insert(
            "command_palette.focus".to_string(),
            KeyCombination {
                ctrl: cfg!(not(target_os = "macos")),
                shift: false,
                alt: false,
                command: cfg!(target_os = "macos"),
                key: Key::Character('`'),
            },
        );
        bindings.insert(
            "close_tab".to_string(),
            KeyCombination {
                ctrl: cfg!(not(target_os = "macos")),
                shift: false,
                alt: false,
                command: cfg!(target_os = "macos"),
                key: Key::Character('W'),
            },
        );
        bindings.insert(
            "move_line_up".to_string(),
            KeyCombination {
                ctrl: false,
                shift: false,
                alt: true,
                command: false,
                key: Key::ArrowUp,
            },
        );
        bindings.insert(
            "move_line_down".to_string(),
            KeyCombination {
                ctrl: false,
                shift: false,
                alt: true,
                command: false,
                key: Key::ArrowDown,
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

    pub fn to_toml_string(&self) -> Result<String, toml::ser::Error> {
        let mut raw = RawKeymap::default();
        raw.bindings = self
            .bindings
            .iter()
            .map(|(action, combo)| (action.clone(), combo.to_string()))
            .collect();
        toml::to_string(&raw)
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

#[derive(Debug, Deserialize, Serialize)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_parsing_basic() {
        assert_eq!(Key::parse("a").unwrap(), Key::Character('A'));
        assert_eq!(Key::parse("A").unwrap(), Key::Character('A'));
        assert_eq!(Key::parse("1").unwrap(), Key::Character('1'));
    }

    #[test]
    fn key_parsing_special_keys() {
        assert_eq!(Key::parse("esc").unwrap(), Key::Escape);
        assert_eq!(Key::parse("escape").unwrap(), Key::Escape);
        assert_eq!(Key::parse("enter").unwrap(), Key::Enter);
        assert_eq!(Key::parse("return").unwrap(), Key::Enter);
        assert_eq!(Key::parse("tab").unwrap(), Key::Tab);
        assert_eq!(Key::parse("space").unwrap(), Key::Space);
        assert_eq!(Key::parse("spacebar").unwrap(), Key::Space);
    }

    #[test]
    fn key_parsing_arrows() {
        assert_eq!(Key::parse("up").unwrap(), Key::ArrowUp);
        assert_eq!(Key::parse("arrowup").unwrap(), Key::ArrowUp);
        assert_eq!(Key::parse("down").unwrap(), Key::ArrowDown);
        assert_eq!(Key::parse("arrowdown").unwrap(), Key::ArrowDown);
        assert_eq!(Key::parse("left").unwrap(), Key::ArrowLeft);
        assert_eq!(Key::parse("arrowleft").unwrap(), Key::ArrowLeft);
        assert_eq!(Key::parse("right").unwrap(), Key::ArrowRight);
        assert_eq!(Key::parse("arrowright").unwrap(), Key::ArrowRight);
    }

    #[test]
    fn key_parsing_function_keys() {
        assert_eq!(Key::parse("f1").unwrap(), Key::Function(1));
        assert_eq!(Key::parse("F12").unwrap(), Key::Function(12));
        assert_eq!(Key::parse("f24").unwrap(), Key::Function(24));
    }

    #[test]
    fn key_parsing_invalid_function_keys() {
        assert!(Key::parse("f0").is_err());
        assert!(Key::parse("f25").is_err());
        assert!(Key::parse("f123").is_err());
    }

    #[test]
    fn key_parsing_other_special() {
        assert_eq!(Key::parse("backspace").unwrap(), Key::Backspace);
        assert_eq!(Key::parse("bksp").unwrap(), Key::Backspace);
        assert_eq!(Key::parse("delete").unwrap(), Key::Delete);
        assert_eq!(Key::parse("del").unwrap(), Key::Delete);
    }

    #[test]
    fn key_parsing_errors() {
        assert!(matches!(
            Key::parse("").unwrap_err(),
            ParseKeyCombinationError::UnknownKey(_)
        ));
        assert!(matches!(
            Key::parse("invalid").unwrap_err(),
            ParseKeyCombinationError::UnknownKey(_)
        ));
        assert!(matches!(
            Key::parse("f25").unwrap_err(),
            ParseKeyCombinationError::UnknownKey(_)
        ));
    }

    #[test]
    fn key_display() {
        assert_eq!(format!("{}", Key::Character('a')), "A");
        assert_eq!(format!("{}", Key::Function(5)), "F5");
        assert_eq!(format!("{}", Key::Escape), "Esc");
        assert_eq!(format!("{}", Key::Enter), "Enter");
        assert_eq!(format!("{}", Key::Tab), "Tab");
        assert_eq!(format!("{}", Key::Space), "Space");
        assert_eq!(format!("{}", Key::ArrowUp), "ArrowUp");
        assert_eq!(format!("{}", Key::Backspace), "Backspace");
        assert_eq!(format!("{}", Key::Delete), "Delete");
    }

    #[test]
    fn key_combination_parsing_simple() {
        let combo = KeyCombination::parse("ctrl+s").unwrap();
        assert!(combo.ctrl);
        assert!(!combo.shift);
        assert!(!combo.alt);
        assert!(!combo.command);
        assert_eq!(combo.key, Key::Character('S'));

        let combo = KeyCombination::parse("shift+f5").unwrap();
        assert!(!combo.ctrl);
        assert!(combo.shift);
        assert!(!combo.alt);
        assert!(!combo.command);
        assert_eq!(combo.key, Key::Function(5));
    }

    #[test]
    fn key_combination_parsing_multiple_modifiers() {
        let combo = KeyCombination::parse("ctrl+shift+p").unwrap();
        assert!(combo.ctrl);
        assert!(combo.shift);
        assert!(!combo.alt);
        assert!(!combo.command);
        assert_eq!(combo.key, Key::Character('P'));

        let combo = KeyCombination::parse("ctrl+alt+delete").unwrap();
        assert!(combo.ctrl);
        assert!(!combo.shift);
        assert!(combo.alt);
        assert!(!combo.command);
        assert_eq!(combo.key, Key::Delete);
    }

    #[test]
    fn key_combination_parsing_all_modifiers() {
        let combo = KeyCombination::parse("ctrl+shift+alt+cmd+z").unwrap();
        assert!(combo.ctrl);
        assert!(combo.shift);
        assert!(combo.alt);
        assert!(combo.command);
        assert_eq!(combo.key, Key::Character('Z'));
    }

    #[test]
    fn key_combination_parsing_alternative_names() {
        // Alternative modifier names
        let combo1 = KeyCombination::parse("ctrl+p").unwrap();
        let combo2 = KeyCombination::parse("control+p").unwrap();
        assert_eq!(combo1.ctrl, combo2.ctrl);

        let combo1 = KeyCombination::parse("alt+x").unwrap();
        let combo2 = KeyCombination::parse("option+x").unwrap();
        assert_eq!(combo1.alt, combo2.alt);

        let combo1 = KeyCombination::parse("cmd+c").unwrap();
        let combo2 = KeyCombination::parse("command+c").unwrap();
        let combo3 = KeyCombination::parse("super+c").unwrap();
        let combo4 = KeyCombination::parse("meta+c").unwrap();
        assert_eq!(combo1.command, combo2.command);
        assert_eq!(combo2.command, combo3.command);
        assert_eq!(combo3.command, combo4.command);
    }

    #[test]
    fn key_combination_parsing_whitespace() {
        let combo1 = KeyCombination::parse("ctrl+s").unwrap();
        let combo2 = KeyCombination::parse(" ctrl + s ").unwrap();
        let combo3 = KeyCombination::parse("ctrl+ s").unwrap();
        let combo4 = KeyCombination::parse("ctrl +s").unwrap();

        assert_eq!(combo1.ctrl, combo2.ctrl);
        assert_eq!(combo1.ctrl, combo3.ctrl);
        assert_eq!(combo1.ctrl, combo4.ctrl);
    }

    #[test]
    fn key_combination_parsing_errors() {
        // No key specified
        assert!(matches!(
            KeyCombination::parse("ctrl+shift+").unwrap_err(),
            ParseKeyCombinationError::MissingKey(_)
        ));

        // Multiple keys
        assert!(matches!(
            KeyCombination::parse("ctrl+s+t").unwrap_err(),
            ParseKeyCombinationError::MultipleKeys(_)
        ));

        // Invalid key
        assert!(matches!(
            KeyCombination::parse("ctrl+invalid").unwrap_err(),
            ParseKeyCombinationError::UnknownKey(_)
        ));
    }

    #[test]
    fn key_combination_display() {
        let combo = KeyCombination::parse("ctrl+s").unwrap();
        assert_eq!(format!("{}", combo), "Ctrl+S");

        let combo = KeyCombination::parse("ctrl+shift+p").unwrap();
        assert_eq!(format!("{}", combo), "Ctrl+Shift+P");

        let combo = KeyCombination::parse("cmd+alt+delete").unwrap();
        assert_eq!(format!("{}", combo), "Alt+Cmd+Delete");

        let combo = KeyCombination::parse("shift+f5").unwrap();
        assert_eq!(format!("{}", combo), "Shift+F5");
    }

    #[test]
    fn key_combination_matches() {
        let combo = KeyCombination::parse("ctrl+s").unwrap();

        let matching_event = KeyEvent::new(
            Key::Character('S'),
            true,  // ctrl
            false, // shift
            false, // alt
            false, // command
        );
        assert!(combo.matches(&matching_event));

        let non_matching_event = KeyEvent::new(
            Key::Character('S'),
            false, // ctrl
            true,  // shift
            false, // alt
            false, // command
        );
        assert!(!combo.matches(&non_matching_event));
    }

    #[test]
    fn keymap_default_bindings() {
        let keymap = Keymap::default();

        // Check that default bindings exist
        assert!(keymap.binding(QUICK_COMMAND_MENU_ACTION).is_some());
        assert!(keymap.binding(SAVE_ACTION).is_some());
        assert!(keymap.binding("command_palette.toggle").is_some());
        assert!(keymap.binding("sidebar.toggle").is_some());
        assert!(keymap.binding("terminal.toggle").is_some());
        assert!(keymap.binding("close_tab").is_some());
    }

    #[test]
    fn keymap_platform_specific_defaults() {
        let keymap = Keymap::default();

        #[cfg(target_os = "macos")]
        {
            // On macOS, save should be Command+S
            let save_binding = keymap.binding(SAVE_ACTION).unwrap();
            assert!(save_binding.command);
            assert!(!save_binding.ctrl);
        }

        #[cfg(not(target_os = "macos"))]
        {
            // On other platforms, save should be Ctrl+S
            let save_binding = keymap.binding(SAVE_ACTION).unwrap();
            assert!(save_binding.ctrl);
            assert!(!save_binding.command);
        }
    }

    #[test]
    fn keymap_set_binding() {
        let mut keymap = Keymap::default();

        // Add a new binding
        let combo = KeyCombination::parse("ctrl+shift+x").unwrap();
        keymap.set_binding("test.action", Some(combo.clone()));

        let retrieved = keymap.binding("test.action").unwrap();
        assert!(retrieved.matches(&KeyEvent::new(
            Key::Character('X'),
            true,  // ctrl
            true,  // shift
            false, // alt
            false, // command
        )));

        // Remove a binding
        keymap.set_binding("test.action", None);
        assert!(keymap.binding("test.action").is_none());
    }

    #[test]
    fn keymap_merge() {
        let mut keymap1 = Keymap::default();
        let mut keymap2 = Keymap::default();

        // Add different bindings to each
        keymap1.set_binding("action1", Some(KeyCombination::parse("ctrl+a").unwrap()));
        keymap2.set_binding("action2", Some(KeyCombination::parse("ctrl+b").unwrap()));

        // Merge keymap2 into keymap1
        keymap1.merge(keymap2);

        // Both bindings should exist in keymap1
        assert!(keymap1.binding("action1").is_some());
        assert!(keymap1.binding("action2").is_some());
    }

    #[test]
    fn keymap_toml_serialization() {
        let mut keymap = Keymap::default();
        keymap.set_binding("test.action", Some(KeyCombination::parse("ctrl+x").unwrap()));

        let toml_str = keymap.to_toml_string().unwrap();
        assert!(toml_str.contains("test.action"));
        assert!(toml_str.contains("Ctrl+X"));

        // Parse it back
        let parsed_keymap = Keymap::from_toml_str(&toml_str).unwrap();
        let binding = parsed_keymap.binding("test.action").unwrap();
        assert!(binding.ctrl);
        assert!(!binding.shift);
        assert!(!binding.alt);
        assert!(!binding.command);
        assert_eq!(binding.key, Key::Character('X'));
    }

    #[test]
    fn keymap_toml_parsing_errors() {
        let invalid_toml = "invalid toml content [";
        let result = Keymap::from_toml_str(invalid_toml);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), KeymapError::Toml(_)));

        let invalid_binding = r#"
bindings = { "test.action" = "invalid+key+combination" }
"#;
        let result = Keymap::from_toml_str(invalid_binding);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), KeymapError::Parse { .. }));
    }

    #[test]
    fn keymap_file_operations() {
        use std::fs;
        use std::env::temp_dir;

        let mut keymap = Keymap::default();
        keymap.set_binding("file.test", Some(KeyCombination::parse("ctrl+t").unwrap()));

        // Create temporary file
        let temp_dir = temp_dir();
        let file_path = temp_dir.join("test_keymap.toml");

        // Save to file
        let toml_content = keymap.to_toml_string().unwrap();
        fs::write(&file_path, toml_content).unwrap();

        // Load from file
        let loaded_keymap = Keymap::load_from_file(&file_path).unwrap();
        let binding = loaded_keymap.binding("file.test").unwrap();
        assert_eq!(binding.key, Key::Character('T'));
        assert!(binding.ctrl);

        // Cleanup
        fs::remove_file(&file_path).unwrap();
    }

    #[test]
    fn keymap_error_display() {
        let missing_key_error = ParseKeyCombinationError::MissingKey("ctrl+".to_string());
        let error_str = format!("{}", missing_key_error);
        assert!(error_str.contains("No key specified"));

        let unknown_key_error = ParseKeyCombinationError::UnknownKey("invalid".to_string());
        let error_str = format!("{}", unknown_key_error);
        assert!(error_str.contains("Unknown key 'invalid'"));

        let multiple_keys_error = ParseKeyCombinationError::MultipleKeys("ctrl+a+b".to_string());
        let error_str = format!("{}", multiple_keys_error);
        assert!(error_str.contains("Multiple keys specified"));
    }

    #[test]
    fn keymap_error_chaining() {
        let parse_error = ParseKeyCombinationError::UnknownKey("test".to_string());
        let keymap_error = KeymapError::Parse {
            action: "test.action".to_string(),
            source: parse_error,
        };

        let display_str = format!("{}", keymap_error);
        assert!(display_str.contains("test.action"));
        assert!(display_str.contains("Unknown key"));

        // Test error source
        use std::error::Error;
        assert!(keymap_error.source().is_some());
    }

    #[test]
    fn complex_combination_parsing() {
        // Test complex but valid combinations
        let combos = vec![
            ("ctrl+shift+alt+cmd+f1", true, true, true, true, Key::Function(1)),
            ("control+option+super+delete", true, false, true, true, Key::Delete),
            ("ctrl+meta+backspace", true, false, false, true, Key::Backspace),
        ];

        for (combo_str, ctrl, shift, alt, cmd, key) in combos {
            let combo = KeyCombination::parse(combo_str).unwrap();
            assert_eq!(combo.ctrl, ctrl, "Failed for {}", combo_str);
            assert_eq!(combo.shift, shift, "Failed for {}", combo_str);
            assert_eq!(combo.alt, alt, "Failed for {}", combo_str);
            assert_eq!(combo.command, cmd, "Failed for {}", combo_str);
            assert_eq!(combo.key, key, "Failed for {}", combo_str);
        }
    }

    #[test]
    fn edge_case_key_combinations() {
        // Test keys with modifiers that might have case sensitivity issues
        let combo1 = KeyCombination::parse("CTRL+S").unwrap();
        let combo2 = KeyCombination::parse("ctrl+s").unwrap();
        assert_eq!(combo1.ctrl, combo2.ctrl);

        // Test function key case sensitivity
        let combo1 = KeyCombination::parse("F5").unwrap();
        let combo2 = KeyCombination::parse("f5").unwrap();
        assert_eq!(combo1.key, combo2.key);
    }

    #[test]
    fn constants_are_valid() {
        // Test that our constants parse correctly
        let quick_combo = KeyCombination::parse("ctrl+shift+p").unwrap();
        let keymap = Keymap::default();
        let quick_binding = keymap.binding(QUICK_COMMAND_MENU_ACTION).unwrap();
        assert_eq!(quick_combo.ctrl, quick_binding.ctrl);
        assert_eq!(quick_combo.shift, quick_binding.shift);
        assert_eq!(quick_combo.key, quick_binding.key);

        let save_combo = KeyCombination::parse("ctrl+s").unwrap();
        let save_binding = keymap.binding(SAVE_ACTION).unwrap();
        // Note: save_combo uses ctrl, but save_binding might use cmd on macOS
        assert_eq!(save_combo.key, save_binding.key);
    }
}