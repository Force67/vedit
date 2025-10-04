use crate::commands::WorkspaceData;
use crate::quick_commands::QuickCommandId;
use crate::settings::SettingsCategory;
use iced::keyboard;
use iced::mouse;
use crate::widgets::text_editor::Action as TextEditorAction;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use vedit_core::{Document, FileNode};

#[derive(Debug, Clone)]
pub enum Message {
    OpenFileRequested,
    FileLoaded(Result<Option<Document>, String>),
    DocumentSelected(usize),
    WorkspaceOpenRequested,
    SolutionOpenRequested,
    WorkspaceLoaded(Result<Option<WorkspaceData>, String>),
    SolutionLoaded(Result<Option<WorkspaceData>, String>),
    WorkspaceFileActivated(String),
    WorkspaceDirectoryToggled(String),
    BufferAction(TextEditorAction),
    BufferScrollChanged(f32),
    DocumentSaved(Result<Option<String>, String>),
    WorkspaceConfigSaved(Result<String, String>),
    SettingsOpened,
    SettingsClosed,
    SettingsCategorySelected(SettingsCategory),
    SettingsBindingChanged(QuickCommandId, String),
    SettingsBindingApplied(QuickCommandId),
    SettingsBindingsSaveRequested,
    SettingsBindingsSaved(Result<String, String>),
    SettingsKeymapPathRequested,
    SettingsKeymapPathSelected(Result<Option<String>, String>),
    Keyboard(keyboard::Event),
    CommandPaletteInputChanged(String),
    CommandPaletteCommandInvoked(QuickCommandId),
    CommandPaletteClosed,
    CommandPromptToggled,
    MouseWheelScrolled(mouse::ScrollDelta),
}

#[derive(Clone)]
pub struct WorkspaceSnapshot {
    pub version: u64,
    pub tree: Arc<Vec<FileNode>>,
}

impl WorkspaceSnapshot {
    pub fn new(version: u64, tree: Arc<Vec<FileNode>>) -> Self {
        Self { version, tree }
    }
}

impl fmt::Debug for WorkspaceSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WorkspaceSnapshot")
            .field("version", &self.version)
            .field("tree_entries", &self.tree.len())
            .finish()
    }
}

impl Hash for WorkspaceSnapshot {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.version.hash(state);
        (Arc::as_ptr(&self.tree) as usize).hash(state);
    }
}
