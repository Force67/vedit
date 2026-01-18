use std::collections::BTreeMap;

use crate::quick_commands::{QuickCommand, QuickCommandId};
use vedit_core::Keymap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SettingsCategory {
    Keybindings,
    Wine,
}

pub const SETTINGS_CATEGORIES: &[SettingsCategory] =
    &[SettingsCategory::Keybindings, SettingsCategory::Wine];

impl SettingsCategory {
    pub fn label(self) -> &'static str {
        match self {
            SettingsCategory::Keybindings => "Keybindings",
            SettingsCategory::Wine => "Wine / Proton",
        }
    }
}

#[derive(Debug)]
pub struct SettingsState {
    is_open: bool,
    selected: SettingsCategory,
    binding_inputs: BTreeMap<QuickCommandId, String>,
    binding_errors: BTreeMap<QuickCommandId, Option<String>>,
}

impl SettingsState {
    pub fn new(commands: &[QuickCommand], keymap: &Keymap) -> Self {
        let mut state = Self {
            is_open: false,
            selected: SettingsCategory::Keybindings,
            binding_inputs: BTreeMap::new(),
            binding_errors: BTreeMap::new(),
        };
        state.sync_bindings(commands, keymap);
        state
    }

    pub fn is_open(&self) -> bool {
        self.is_open
    }

    pub fn open(&mut self) {
        self.is_open = true;
    }

    pub fn close(&mut self) {
        self.is_open = false;
    }

    pub fn selected_category(&self) -> SettingsCategory {
        self.selected
    }

    pub fn select_category(&mut self, category: SettingsCategory) {
        self.selected = category;
    }

    pub fn binding_input(&self, id: QuickCommandId) -> &str {
        self.binding_inputs
            .get(&id)
            .map(|value| value.as_str())
            .unwrap_or("")
    }

    pub fn set_binding_input(&mut self, id: QuickCommandId, value: String) {
        self.binding_inputs.insert(id, value);
    }

    pub fn binding_error(&self, id: QuickCommandId) -> Option<&str> {
        self.binding_errors
            .get(&id)
            .and_then(|value| value.as_deref())
    }

    pub fn set_binding_error(&mut self, id: QuickCommandId, error: Option<String>) {
        if error.is_some() {
            self.binding_errors.insert(id, error);
        } else {
            self.binding_errors.remove(&id);
        }
    }

    pub fn sync_bindings(&mut self, commands: &[QuickCommand], keymap: &Keymap) {
        for command in commands.iter().filter(|cmd| cmd.action.is_some()) {
            let entry = keymap
                .binding(command.action.unwrap())
                .map(|combo| combo.to_string())
                .unwrap_or_default();
            self.binding_inputs.insert(command.id, entry);
            self.binding_errors.remove(&command.id);
        }
    }
}
