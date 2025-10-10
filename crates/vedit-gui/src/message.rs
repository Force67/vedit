use iced::keyboard;
use iced::mouse;
use iced::widget::text_editor::Action as TextEditorAction;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use vedit_core::{Document, FileNode};
use vedit_application::{QuickCommandId, SettingsCategory};
use crate::commands::WorkspaceData;
use crate::commands::DebugSession;
use crate::debugger::DebuggerType;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RightRailTab {
    Workspace,
    Solutions,
    Outline,
    Search,
    Problems,
    Notes,
}

#[derive(Debug, Clone)]
pub enum Message {
    OpenFileRequested,
    FileLoaded(Result<Option<Document>, String>),
    DocumentSelected(usize),
    WorkspaceOpenRequested,
    SolutionOpenRequested,
    SolutionSelected(String),
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
    DebuggerTypeChanged(DebuggerType),
    DebuggerLaunchRequested,
    DebuggerSessionStarted(Result<DebugSession, String>),
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
    FpsUpdate,
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
    WindowMinimize,
    WindowMaximize,
    WindowClose,
    WindowDragStart,
    WindowResizeStart(iced::Point),
    WindowResizeMove(iced::Point),
    WindowResizeEnd,
    FileExplorer(crate::widgets::file_explorer::Message),
    RightRailTabSelected(RightRailTab),
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
