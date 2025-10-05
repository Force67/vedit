use iced::keyboard;
use iced::mouse;
use crate::widgets::text_editor::Action as TextEditorAction;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use vedit_core::{Document, FileNode};
use vedit_application::{QuickCommandId, SettingsCategory};
use crate::commands::WorkspaceData;
use vedit_debugger::GdbSession;

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
    WorkspaceMetadataSaved(Result<String, String>),
    StickyNoteCreateRequested,
    StickyNoteContentChanged(u64, String),
    StickyNoteDeleted(u64),
    SettingsOpened,
    SettingsClosed,
    SettingsCategorySelected(SettingsCategory),
    SettingsBindingChanged(QuickCommandId, String),
    SettingsBindingApplied(QuickCommandId),
    SettingsBindingsSaveRequested,
    SettingsBindingsSaved(Result<String, String>),
    SettingsKeymapPathRequested,
    SettingsKeymapPathSelected(Result<Option<String>, String>),
    DebuggerTargetsRefreshRequested,
    DebuggerMenuToggled,
    DebuggerTargetToggled(u64, bool),
    DebuggerTargetFilterChanged(String),
    DebuggerLaunchRequested,
    DebuggerSessionStarted(Result<GdbSession, String>),
    DebuggerStopRequested,
    DebuggerGdbCommandInputChanged(String),
    DebuggerGdbCommandSubmitted,
    DebuggerBreakpointToggled(u64),
    DebuggerBreakpointRemoved(u64),
    DebuggerBreakpointConditionChanged(u64, String),
    DebuggerBreakpointDraftFileChanged(String),
    DebuggerBreakpointDraftLineChanged(String),
    DebuggerBreakpointDraftConditionChanged(String),
    DebuggerBreakpointDraftSubmitted,
    DebuggerManualTargetNameChanged(String),
    DebuggerManualTargetExecutableChanged(String),
    DebuggerManualTargetWorkingDirectoryChanged(String),
    DebuggerManualTargetArgumentsChanged(String),
    DebuggerManualTargetSaved,
    DebuggerLaunchScriptChanged(String),
    DebuggerTick,
    Keyboard(keyboard::Event),
    CommandPaletteInputChanged(String),
    CommandPaletteCommandInvoked(QuickCommandId),
    CommandPaletteClosed,
    CommandPromptToggled,
    ConsoleVisibilityToggled,
    ConsoleTabSelected(u64),
    ConsoleNewRequested,
    ConsoleInputChanged(u64, String),
    ConsoleInputSubmitted(u64),
    MouseWheelScrolled(mouse::ScrollDelta),
    NotificationDismissed(u64),
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
