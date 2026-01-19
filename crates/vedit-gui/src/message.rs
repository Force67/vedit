// Message enum contains all application messages - some variants are defined
// for future features or API completeness and may not yet be constructed
#![allow(dead_code)]

use crate::commands::DebugSession;
use crate::commands::WorkspaceData;
use crate::debugger::DebuggerType;
use iced::keyboard;
use iced::mouse;
use iced::widget::text_editor::Action as TextEditorAction;
use vedit_application::{QuickCommandId, SettingsCategory};
use vedit_core::Document;
// use crate::widgets::wine::{WineState, WineTab, WineArchitecture, WineWindowsVersion, WineDesktopType}; // Temporarily disabled

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RightRailTab {
    Workspace,
    Solutions,
    Outline,
    Search,
    Problems,
    Notes,
    Wine,
}

#[derive(Debug, Clone)]
pub enum Message {
    OpenFileRequested,
    FileLoaded(Result<Option<Document>, String>),
    DocumentSelected(usize),
    CloseDocument(usize),
    WorkspaceOpenRequested,
    SolutionOpenRequested,
    SolutionSelected(String),
    WorkspaceLoaded(Result<Option<WorkspaceData>, String>),
    SolutionLoaded(Result<Option<WorkspaceData>, String>),
    WorkspaceFileActivated(String),
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
    EditorLogShowRequested,
    MouseWheelScrolled(mouse::ScrollDelta),
    NotificationDismissed(u64),
    WindowIdDiscovered(iced::window::Id),
    WindowMinimize,
    WindowMaximize,
    WindowClose,
    WindowDragStart,
    WindowResizeStart(iced::Point),
    WindowResizeEnd,
    RefreshRateDetected(f32, f32), // (highest_refresh, current_refresh)
    FileExplorer(crate::widgets::file_explorer::Message),
    RightRailTabSelected(RightRailTab),
    // Search dialog messages
    SearchOpen,
    SearchClose,
    SearchQueryChanged(String),
    SearchExecute,
    SearchDebounceTick,
    SearchHighlightTick,
    SearchNext,
    SearchPrevious,
    SearchCaseSensitive(bool),
    SearchWholeWord(bool),
    SearchUseRegex(bool),
    SearchToggleReplace,
    ReplaceTextChanged(String),
    ReplaceOne,
    ReplaceAll,
    // Debug dot messages
    DebugDotAdd(usize),
    DebugDotRemove(usize),
    DebugDotToggle(usize),
    DebugDotsClear,
    GutterClicked(usize), // Line number clicked in gutter

    // Editor context menu messages
    EditorContextMenuShow(f32, f32, Option<crate::widgets::text_editor::HoverPosition>), // (x, y, position)
    EditorContextMenuHide,
    EditorContextMenuAddStickyNote,
    EditorContextMenuCut,
    EditorContextMenuCopy,
    EditorContextMenuPaste,
    EditorContextMenuSelectAll,
    EditorContextMenuGotoDefinition,

    // Session management messages
    SessionLoad(Result<crate::session::SessionState, String>),
    SessionSave(Result<(), String>),
    WindowStateUpdate(crate::session::WindowState),
    WorkspaceStateUpdate(crate::session::WorkspaceState),
    WorkspaceRestoreFromPath(std::path::PathBuf, crate::session::SessionState),
    FilesRestoreRequested(Vec<std::path::PathBuf>),
    AdditionalFilesRestoreRequested(Vec<std::path::PathBuf>),

    // Window state tracking messages
    WindowChanged(u32, u32), // width, height
    WindowMoved(i32, i32),   // x, y
    WindowEvent(iced::window::Event),

    // Solution explorer tree messages
    SolutionTreeToggle(String), // Node ID to expand/collapse

    // Wine integration messages
    // Simple Wine integration messages
    WineCreateEnvironmentDialog,
    WineEnvNameChanged(String),
    WineExePathChanged(String),
    WineArgsChanged(String),
    WineEnvironmentToggled(String),
    WineCreateEnvironment,
    WineSpawnProcess,

    // Hover-to-definition messages
    EditorHover(crate::widgets::text_editor::HoverPosition, f32, f32), // (position, x, y)
    HoverTooltipShow(HoverInfo),
    HoverTooltipHide,
    HoverGotoDefinition(std::path::PathBuf, usize, usize), // (file, line, column)
    HoverDelayTick,                                        // Timer tick to check hover delay
    HoverCursorMoved(f32, f32),                            // Cursor position for tooltip stickiness
    SymbolIndexRefresh,
    SymbolIndexUpdated(Result<usize, String>), // number of files indexed

    // Navigation history (back/forward like VS)
    NavigateBack,
    NavigateForward,

    // Solution context menu messages
    SolutionContextMenuShow {
        target: SolutionContextTarget,
        x: f32,
        y: f32,
    },
    SolutionContextMenuHide,
    SolutionContextMenuBuild(std::path::PathBuf),
    SolutionContextMenuRebuild(std::path::PathBuf),
    SolutionContextMenuClean(std::path::PathBuf),
    SolutionContextMenuDebug(std::path::PathBuf),
    SolutionContextMenuOpenFolder(std::path::PathBuf),

    // Build messages
    BuildStarted {
        target: String,
        configuration: String,
        platform: String,
    },
    BuildOutput(String),
    BuildWarning {
        file: Option<String>,
        line: Option<u32>,
        message: String,
    },
    BuildError {
        file: Option<String>,
        line: Option<u32>,
        message: String,
    },
    BuildCompleted {
        success: bool,
        duration: std::time::Duration,
    },
    BuildCancelled,
    BuildCancelRequested,
    /// Result from Wine MSBuild
    WineBuildResult(Result<crate::commands::WineBuildResult, String>),

    // Wine/Proton environment messages
    WineEnvironmentDiscoveryRequested,
    WineEnvironmentDiscovered(vedit_wine::EnvironmentDiscovery),
    WineEnvironmentSelected(String),
    WineEnvironmentSettingsOpened,
    // Wine prefix management
    WinePrefixCreateStart,
    WinePrefixNameChanged(String),
    WinePrefixWineBinarySelected(usize),
    WinePrefixCreateConfirm,
    WinePrefixCreated(Result<vedit_wine::WinePrefix, String>),
    WinePrefixSelected(usize),
    WinePrefixDelete(usize),
    WinePrefixCancelCreate,
    // VS Build Tools installation
    VsBuildToolsInstallStart(usize), // prefix index
    VsBuildToolsDownloadProgress(u8),
    VsBuildToolsInstallProgress(String),
    VsBuildToolsInstallComplete(usize), // prefix index
    VsBuildToolsInstallFailed(String),
    // MSVC Download (using msvc-wine)
    MsvcDownloadStart,
    MsvcDownloadProgress(String),
    MsvcDownloadComplete(Result<std::path::PathBuf, String>),
}

/// Target for solution context menu actions
#[derive(Debug, Clone)]
pub enum SolutionContextTarget {
    /// A Visual Studio solution (.sln)
    Solution(std::path::PathBuf),

    /// A Visual Studio project (.vcxproj, .csproj, etc.)
    Project(std::path::PathBuf),
}

/// Information about a hover target for the tooltip
#[derive(Debug, Clone)]
pub struct HoverInfo {
    pub symbol_name: String,
    pub definition: vedit_symbols::DefinitionLocation,
    pub tooltip_x: f32,
    pub tooltip_y: f32,
}
