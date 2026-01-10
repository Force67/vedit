use crossbeam_channel::{Receiver, Sender};
use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use vedit_config::{DebugTargetRecord, MAX_RECENT_DEBUG_TARGETS};
use vedit_debugger::{DebuggerCommand as VeditCommand, DebuggerEvent as VeditEvent, VeditSession};
use vedit_debugger_gdb::{DebuggerCommand as GdbCommand, DebuggerEvent as GdbEvent, GdbSession};
use vedit_make::Makefile;
use vedit_vs::{Solution, VcxProject};

const IGNORED_DIRECTORIES: [&str; 4] = ["target", ".git", ".hg", ".svn"];
const MAX_CONSOLE_ENTRIES: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DebugTargetSource {
    Vcxproj { project_path: PathBuf },
    Makefile { path: PathBuf },
    Manual,
}

impl fmt::Display for DebugTargetSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DebugTargetSource::Vcxproj { project_path } => {
                write!(f, "vcxproj ({})", display_path(project_path))
            }
            DebugTargetSource::Makefile { path } => {
                write!(f, "makefile ({})", display_path(path))
            }
            DebugTargetSource::Manual => write!(f, "manual"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugTarget {
    pub id: u64,
    pub name: String,
    pub executable: PathBuf,
    pub working_directory: PathBuf,
    pub args: Vec<String>,
    pub source: DebugTargetSource,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DebugTargetIdentity {
    name: String,
    executable: String,
}

impl DebugTargetIdentity {
    fn from_record(record: DebugTargetRecord) -> Option<Self> {
        if record.name.trim().is_empty() || record.executable.trim().is_empty() {
            None
        } else {
            Some(Self {
                name: record.name,
                executable: record.executable,
            })
        }
    }

    fn from_target(target: &DebugTarget) -> Option<Self> {
        let executable = normalize_executable_path(&target.executable);
        if executable.trim().is_empty() {
            return None;
        }

        Some(Self {
            name: target.name.clone(),
            executable,
        })
    }

    fn matches(&self, target: &DebugTarget) -> bool {
        self.executable == normalize_executable_path(&target.executable)
    }
}

impl fmt::Display for DebugTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebuggerBreakpoint {
    pub id: u64,
    pub file: PathBuf,
    pub line: u32,
    pub condition: Option<String>,
    pub enabled: bool,
}

impl DebuggerBreakpoint {
    pub fn display_path(&self, workspace_root: Option<&Path>) -> String {
        if let Some(root) = workspace_root {
            if let Ok(relative) = self.file.strip_prefix(root) {
                return relative.display().to_string();
            }
        }
        self.file.display().to_string()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DebuggerType {
    #[default]
    Gdb,
    Vedit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugSessionStatus {
    Idle,
    Launching,
    Running,
    Paused,
    Exited,
    Failed,
}

impl DebugSessionStatus {
    pub fn label(self) -> &'static str {
        match self {
            DebugSessionStatus::Idle => "Idle",
            DebugSessionStatus::Launching => "Launching",
            DebugSessionStatus::Running => "Running",
            DebugSessionStatus::Paused => "Paused",
            DebugSessionStatus::Exited => "Exited",
            DebugSessionStatus::Failed => "Failed",
        }
    }
}

impl Default for DebugSessionStatus {
    fn default() -> Self {
        DebugSessionStatus::Idle
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebuggerConsoleEntryKind {
    Command,
    Output,
    Error,
    Info,
}

#[derive(Debug, Clone)]
pub struct DebuggerConsoleEntry {
    pub kind: DebuggerConsoleEntryKind,
    pub message: String,
}

impl DebuggerConsoleEntry {
    pub fn command(message: impl Into<String>) -> Self {
        Self {
            kind: DebuggerConsoleEntryKind::Command,
            message: message.into(),
        }
    }

    pub fn output(message: impl Into<String>) -> Self {
        Self {
            kind: DebuggerConsoleEntryKind::Output,
            message: message.into(),
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            kind: DebuggerConsoleEntryKind::Error,
            message: message.into(),
        }
    }

    pub fn info(message: impl Into<String>) -> Self {
        Self {
            kind: DebuggerConsoleEntryKind::Info,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum DebuggerUiEvent {
    SessionStarted { target: Option<String> },
    SessionError { message: String },
}

#[derive(Debug, Default, Clone)]
pub struct ManualTargetDraft {
    pub name: String,
    pub executable: String,
    pub working_directory: String,
    pub arguments: String,
}

#[derive(Debug, Default, Clone)]
pub struct BreakpointDraft {
    pub file: String,
    pub line: String,
    pub condition: String,
}

#[derive(Debug, Clone)]
pub struct DebugLaunchPlan {
    pub target: DebugTarget,
    pub launch_script: Option<String>,
    pub breakpoints: Vec<DebuggerBreakpoint>,
}

#[derive(Debug, Default)]
pub struct DebuggerState {
    workspace_root: Option<PathBuf>,
    targets: Vec<DebugTarget>,
    selected_targets: BTreeSet<u64>,
    recent_target_history: Vec<DebugTargetIdentity>,
    last_target_identity: Option<DebugTargetIdentity>,
    apply_history_selection: bool,
    breakpoints: Vec<DebuggerBreakpoint>,
    next_target_id: u64,
    next_breakpoint_id: u64,
    launch_script: String,
    console: Vec<DebuggerConsoleEntry>,
    console_cursor: usize,
    command_input: String,
    manual_target: ManualTargetDraft,
    breakpoint_draft: BreakpointDraft,
    status: DebugSessionStatus,
    pending_target_name: Option<String>,
    active_target_name: Option<String>,
    menu_open: bool,
    runtime: Option<DebuggerRuntime>,
    target_filter: String,
    debugger_type: DebuggerType,
}

impl DebuggerState {
    pub fn workspace_root(&self) -> Option<&Path> {
        self.workspace_root.as_deref()
    }

    pub fn breakpoints(&self) -> &[DebuggerBreakpoint] {
        &self.breakpoints
    }

    pub fn status(&self) -> DebugSessionStatus {
        self.status
    }

    pub fn launch_script(&self) -> &str {
        &self.launch_script
    }

    pub fn set_launch_script(&mut self, value: String) {
        self.launch_script = value;
    }

    pub fn debugger_type(&self) -> DebuggerType {
        self.debugger_type
    }

    pub fn set_debugger_type(&mut self, debugger_type: DebuggerType) {
        self.debugger_type = debugger_type;
    }

    pub fn command_input(&self) -> &str {
        &self.command_input
    }

    pub fn set_command_input(&mut self, value: String) {
        self.command_input = value;
    }

    pub fn console(&self) -> &[DebuggerConsoleEntry] {
        &self.console
    }

    pub fn take_console_updates(&mut self) -> Vec<DebuggerConsoleEntry> {
        if self.console_cursor >= self.console.len() {
            return Vec::new();
        }
        let updates = self.console[self.console_cursor..].to_vec();
        self.console_cursor = self.console.len();
        updates
    }

    pub fn target_filter(&self) -> &str {
        &self.target_filter
    }

    pub fn menu_open(&self) -> bool {
        self.menu_open
    }

    pub fn toggle_menu(&mut self) {
        self.menu_open = !self.menu_open;
    }

    pub fn close_menu(&mut self) {
        self.menu_open = false;
    }

    pub fn manual_target_draft(&self) -> &ManualTargetDraft {
        &self.manual_target
    }

    pub fn set_manual_target_name(&mut self, value: String) {
        self.manual_target.name = value;
    }

    pub fn set_manual_target_executable(&mut self, value: String) {
        self.manual_target.executable = value;
    }

    pub fn set_manual_target_working_directory(&mut self, value: String) {
        self.manual_target.working_directory = value;
    }

    pub fn set_manual_target_arguments(&mut self, value: String) {
        self.manual_target.arguments = value;
    }

    pub fn set_target_filter(&mut self, value: String) {
        self.target_filter = value;
    }

    pub fn breakpoint_draft(&self) -> &BreakpointDraft {
        &self.breakpoint_draft
    }

    pub fn set_breakpoint_draft_file(&mut self, value: String) {
        self.breakpoint_draft.file = value;
    }

    pub fn set_breakpoint_draft_line(&mut self, value: String) {
        self.breakpoint_draft.line = value;
    }

    pub fn set_breakpoint_draft_condition(&mut self, value: String) {
        self.breakpoint_draft.condition = value;
    }

    pub fn primary_selected_target(&self) -> Option<&DebugTarget> {
        let id = self.selected_targets.iter().next()?;
        self.targets.iter().find(|target| target.id == *id)
    }

    pub fn filtered_targets(&self) -> Vec<&DebugTarget> {
        if self.target_filter.trim().is_empty() {
            return self.targets.iter().collect();
        }

        let filter = self.target_filter.to_ascii_lowercase();
        self.targets
            .iter()
            .filter(|target| {
                target.name.to_ascii_lowercase().contains(&filter)
                    || target
                        .executable
                        .to_string_lossy()
                        .to_ascii_lowercase()
                        .contains(&filter)
            })
            .collect()
    }

    pub fn selected_targets(&self) -> Vec<&DebugTarget> {
        self.selected_targets
            .iter()
            .filter_map(|id| self.targets.iter().find(|target| target.id == *id))
            .collect()
    }

    pub fn selected_target_count(&self) -> usize {
        self.selected_targets.len()
    }

    pub fn set_recent_target_history(
        &mut self,
        recent: Vec<DebugTargetRecord>,
        last: Option<DebugTargetRecord>,
    ) {
        self.recent_target_history = recent
            .into_iter()
            .filter_map(DebugTargetIdentity::from_record)
            .collect();
        if self.recent_target_history.len() > MAX_RECENT_DEBUG_TARGETS {
            self.recent_target_history
                .truncate(MAX_RECENT_DEBUG_TARGETS);
        }
        self.last_target_identity = last.and_then(DebugTargetIdentity::from_record);
        self.apply_history_selection = true;
        self.sort_targets_by_history();
    }

    fn sort_targets_by_history(&mut self) {
        if self.recent_target_history.is_empty() {
            return;
        }

        let history = self.recent_target_history.clone();
        self.targets.sort_by(|a, b| {
            let pos_a = history.iter().position(|entry| entry.matches(a));
            let pos_b = history.iter().position(|entry| entry.matches(b));
            match (pos_a, pos_b) {
                (Some(a_idx), Some(b_idx)) => a_idx.cmp(&b_idx),
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (None, None) => Ordering::Equal,
            }
        });
    }

    fn apply_history_selection_if_needed(&mut self) {
        if self.apply_history_selection {
            self.apply_history_selection = false;
            if let Some(identity) = &self.last_target_identity {
                if let Some(target) = self.targets.iter().find(|target| identity.matches(target)) {
                    self.selected_targets.clear();
                    self.selected_targets.insert(target.id);
                    self.prune_selected_targets();
                    return;
                }
            }
        }

        self.prune_selected_targets();
    }

    fn touch_recent_history(&mut self, target: &DebugTarget) {
        let Some(identity) = DebugTargetIdentity::from_target(target) else {
            return;
        };

        if let Some(position) = self
            .recent_target_history
            .iter()
            .position(|entry| entry == &identity)
        {
            if position != 0 {
                self.recent_target_history.remove(position);
                self.recent_target_history.insert(0, identity.clone());
            }
        } else {
            self.recent_target_history.insert(0, identity.clone());
            if self.recent_target_history.len() > MAX_RECENT_DEBUG_TARGETS {
                self.recent_target_history
                    .truncate(MAX_RECENT_DEBUG_TARGETS);
            }
        }

        self.last_target_identity = Some(identity);
        self.sort_targets_by_history();
    }

    pub fn selection_summary(&self) -> String {
        let selected = self.selected_targets();
        if selected.is_empty() {
            return "Debug: (none)".to_string();
        }

        let first = selected[0];
        if selected.len() == 1 {
            format!("Debug: {}", first.name)
        } else {
            format!("Debug: {} (+{})", first.name, selected.len() - 1)
        }
    }

    pub fn is_target_selected(&self, id: u64) -> bool {
        self.selected_targets.contains(&id)
    }

    pub fn set_target_selected(&mut self, id: u64, selected: bool) {
        if !self.targets.iter().any(|target| target.id == id) {
            return;
        }

        if selected {
            self.selected_targets.insert(id);
        } else {
            self.selected_targets.remove(&id);
        }

        if self.selected_targets.is_empty() {
            if let Some(first) = self.targets.first() {
                self.selected_targets.insert(first.id);
            }
        }
    }

    fn prune_selected_targets(&mut self) {
        self.selected_targets
            .retain(|id| self.targets.iter().any(|target| target.id == *id));

        if self.selected_targets.is_empty() {
            if let Some(first) = self.targets.first() {
                self.selected_targets.insert(first.id);
            }
        }
    }

    pub fn refresh_targets(&mut self, workspace_root: Option<&str>) -> Result<(), String> {
        self.workspace_root = workspace_root.map(PathBuf::from);

        let manual_targets: Vec<DebugTarget> = self
            .targets
            .iter()
            .cloned()
            .filter(|target| matches!(target.source, DebugTargetSource::Manual))
            .collect();

        self.targets = manual_targets;
        self.recalculate_next_target_id();
        self.prune_selected_targets();

        let workspace_root = match self.workspace_root.clone() {
            Some(root) => root,
            None => {
                self.sort_targets_by_history();
                self.apply_history_selection_if_needed();
                return Ok(());
            }
        };

        let mut vcx_projects = BTreeSet::new();
        let mut makefiles = BTreeSet::new();
        let mut warnings = Vec::new();
        scan_workspace(
            &workspace_root,
            &mut vcx_projects,
            &mut makefiles,
            &mut warnings,
        );

        for warning in warnings {
            self.push_console(DebuggerConsoleEntry::error(warning));
        }

        for project_path in vcx_projects {
            match VcxProject::from_path(&project_path) {
                Ok(project) => {
                    if !project.produces_executable {
                        continue;
                    }
                    let id = self.allocate_target_id();
                    let target = DebugTarget {
                        id,
                        name: project.name.clone(),
                        executable: guess_vcx_executable(&project_path, &project.name),
                        working_directory: project_path
                            .parent()
                            .map(Path::to_path_buf)
                            .unwrap_or_else(|| workspace_root.clone()),
                        args: Vec::new(),
                        source: DebugTargetSource::Vcxproj {
                            project_path: project_path.clone(),
                        },
                        notes: Some(format!(
                            "Target generated from {}. Adjust the executable if your configuration differs.",
                            display_path(&project_path)
                        )),
                    };
                    self.targets.push(target);
                }
                Err(err) => {
                    self.push_console(DebuggerConsoleEntry::error(err.to_string()));
                }
            }
        }

        for makefile_path in makefiles {
            match Makefile::from_path(&makefile_path) {
                Ok(makefile) => {
                    let name = makefile
                        .path
                        .parent()
                        .and_then(|parent| parent.file_name())
                        .and_then(|name| name.to_str())
                        .map(|name| format!("{} (make)", name))
                        .unwrap_or_else(|| "Makefile target".to_string());
                    let id = self.allocate_target_id();
                    let parent = makefile
                        .path
                        .parent()
                        .map(Path::to_path_buf)
                        .unwrap_or_else(|| workspace_root.clone());
                    let executable = guess_makefile_executable(&makefile.path);
                    if looks_like_library(&executable) {
                        continue;
                    }
                    let target = DebugTarget {
                        id,
                        name,
                        executable,
                        working_directory: parent,
                        args: Vec::new(),
                        source: DebugTargetSource::Makefile {
                            path: makefile.path.clone(),
                        },
                        notes: Some(
                            "Executable path guessed from Makefile. Update before launching."
                                .to_string(),
                        ),
                    };
                    self.targets.push(target);
                }
                Err(err) => {
                    self.push_console(DebuggerConsoleEntry::error(err.to_string()));
                }
            }
        }

        self.sort_targets_by_history();
        self.apply_history_selection_if_needed();

        self.push_console(DebuggerConsoleEntry::info(format!(
            "Discovered {} debug target(s).",
            self.targets.len()
        )));

        Ok(())
    }

    pub fn commit_manual_target(&mut self) -> Result<(), String> {
        let name = self.manual_target.name.trim().to_string();
        if name.is_empty() {
            return Err("Enter a name for the target".to_string());
        }

        let executable_raw = self.manual_target.executable.trim();
        if executable_raw.is_empty() {
            return Err("Provide an executable path".to_string());
        }

        let mut executable = PathBuf::from(executable_raw);
        if executable.is_relative() {
            if let Some(root) = &self.workspace_root {
                executable = root.join(executable);
            }
        }

        let working_directory = if self.manual_target.working_directory.trim().is_empty() {
            executable
                .parent()
                .map(Path::to_path_buf)
                .or_else(|| self.workspace_root.clone())
                .unwrap_or_else(|| PathBuf::from("."))
        } else {
            let mut path = PathBuf::from(self.manual_target.working_directory.trim());
            if path.is_relative() {
                if let Some(root) = &self.workspace_root {
                    path = root.join(path);
                }
            }
            path
        };

        let args = self
            .manual_target
            .arguments
            .split_whitespace()
            .map(|value| value.to_string())
            .collect::<Vec<_>>();

        let id = self.allocate_target_id();
        let target = DebugTarget {
            id,
            name,
            executable,
            working_directory,
            args,
            source: DebugTargetSource::Manual,
            notes: Some("Manually configured target".to_string()),
        };
        self.targets.push(target);
        self.selected_targets.insert(id);
        self.prune_selected_targets();
        self.sort_targets_by_history();
        self.manual_target = ManualTargetDraft::default();
        Ok(())
    }

    pub fn commit_breakpoint_from_draft(&mut self) -> Result<(), String> {
        let file_raw = self.breakpoint_draft.file.trim();
        if file_raw.is_empty() {
            return Err("Enter a file path".to_string());
        }

        let line = self
            .breakpoint_draft
            .line
            .trim()
            .parse::<u32>()
            .map_err(|_| "Line must be a positive number".to_string())?;

        let mut file = PathBuf::from(file_raw);
        if file.is_relative() {
            if let Some(root) = &self.workspace_root {
                file = root.join(file);
            }
        }

        if let Some(existing) = self
            .breakpoints
            .iter_mut()
            .find(|breakpoint| breakpoint.file == file && breakpoint.line == line)
        {
            existing.enabled = true;
            existing.condition = if self.breakpoint_draft.condition.trim().is_empty() {
                None
            } else {
                Some(self.breakpoint_draft.condition.trim().to_string())
            };
            self.breakpoint_draft = BreakpointDraft::default();
            return Ok(());
        }

        let id = self.allocate_breakpoint_id();
        let breakpoint = DebuggerBreakpoint {
            id,
            file,
            line,
            condition: if self.breakpoint_draft.condition.trim().is_empty() {
                None
            } else {
                Some(self.breakpoint_draft.condition.trim().to_string())
            },
            enabled: true,
        };
        self.breakpoints.push(breakpoint);
        self.breakpoint_draft = BreakpointDraft::default();
        Ok(())
    }

    pub fn toggle_breakpoint(&mut self, id: u64) {
        if let Some(breakpoint) = self
            .breakpoints
            .iter_mut()
            .find(|breakpoint| breakpoint.id == id)
        {
            breakpoint.enabled = !breakpoint.enabled;
        }
    }

    pub fn remove_breakpoint(&mut self, id: u64) {
        self.breakpoints.retain(|breakpoint| breakpoint.id != id);
    }

    pub fn set_breakpoint_condition(&mut self, id: u64, condition: String) {
        if let Some(breakpoint) = self
            .breakpoints
            .iter_mut()
            .find(|breakpoint| breakpoint.id == id)
        {
            let trimmed = condition.trim();
            if trimmed.is_empty() {
                breakpoint.condition = None;
            } else {
                breakpoint.condition = Some(trimmed.to_string());
            }
        }
    }

    pub fn submit_command(&mut self) -> Result<(), String> {
        let trimmed = self.command_input.trim();
        if trimmed.is_empty() {
            return Ok(());
        }
        if self.runtime.is_none() {
            return Err("No active debugger session".to_string());
        }
        let command = trimmed.to_string();
        let prefix = match self.debugger_type {
            DebuggerType::Gdb => "(gdb)",
            DebuggerType::Vedit => "(vedit)",
        };
        self.push_console(DebuggerConsoleEntry::command(format!(
            "{} {}",
            prefix, trimmed
        )));
        self.command_input.clear();
        if let Some(runtime) = &self.runtime {
            match self.debugger_type {
                DebuggerType::Gdb => runtime.send_gdb(GdbCommand::SendRaw(command)),
                DebuggerType::Vedit => {
                    // For now, just send continue for vedit debugger
                    runtime.send_vedit(VeditCommand::Continue);
                }
            }
        }
        Ok(())
    }

    pub fn prepare_launches(&mut self) -> Result<Vec<DebugLaunchPlan>, String> {
        let selected_targets = self
            .selected_targets()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        if selected_targets.is_empty() {
            return Err("Select at least one debug target".to_string());
        }

        self.status = DebugSessionStatus::Launching;

        let script = if self.launch_script.trim().is_empty() {
            None
        } else {
            Some(self.launch_script.clone())
        };

        let breakpoints: Vec<DebuggerBreakpoint> = self
            .breakpoints
            .iter()
            .filter(|breakpoint| breakpoint.enabled)
            .cloned()
            .collect();

        let mut plans = Vec::new();
        for target in selected_targets {
            let debugger_name = match self.debugger_type {
                DebuggerType::Gdb => "gdb",
                DebuggerType::Vedit => "vedit",
            };
            self.push_console(DebuggerConsoleEntry::info(format!(
                "Preparing {} launch for {}",
                debugger_name, target.name
            )));
            plans.push(DebugLaunchPlan {
                target: target.clone(),
                launch_script: script.clone(),
                breakpoints: breakpoints.clone(),
            });
        }

        Ok(plans)
    }

    pub fn begin_launch_for(&mut self, target: &DebugTarget) {
        self.pending_target_name = Some(target.name.clone());
        self.active_target_name = None;
        self.touch_recent_history(target);
    }

    pub fn stop_session(&mut self) {
        if self.status != DebugSessionStatus::Idle {
            if let Some(runtime) = &self.runtime {
                match runtime {
                    DebuggerRuntime::Gdb { .. } => runtime.send_gdb(GdbCommand::Kill),
                    DebuggerRuntime::Vedit { .. } => runtime.send_vedit(VeditCommand::Kill),
                }
            }
            self.runtime = None;
            self.status = DebugSessionStatus::Idle;
            self.pending_target_name = None;
            self.active_target_name = None;
            self.push_console(DebuggerConsoleEntry::info("Debugger session stopped"));
        }
    }

    pub fn attach_gdb_runtime(&mut self, session: GdbSession) {
        self.runtime = Some(DebuggerRuntime::new_gdb(session));
        self.status = DebugSessionStatus::Launching;
        if self.active_target_name.is_none() {
            self.active_target_name = self.pending_target_name.clone();
        }
    }

    pub fn attach_vedit_runtime(&mut self, session: VeditSession) {
        self.runtime = Some(DebuggerRuntime::new_vedit(session));
        self.status = DebugSessionStatus::Launching;
        if self.active_target_name.is_none() {
            self.active_target_name = self.pending_target_name.clone();
        }
    }

    pub fn has_runtime(&self) -> bool {
        self.runtime.is_some()
    }

    pub fn process_runtime_events(&mut self) -> Vec<DebuggerUiEvent> {
        let mut ui_events = Vec::new();

        loop {
            let event = match self.runtime.as_ref().and_then(|runtime| runtime.try_recv()) {
                Some(event) => event,
                None => break,
            };

            match event {
                DebuggerUiEvent::SessionStarted { target: _ } => {
                    self.status = DebugSessionStatus::Running;
                    let debugger_name = match self.debugger_type {
                        DebuggerType::Gdb => "gdb",
                        DebuggerType::Vedit => "vedit",
                    };
                    self.push_console(DebuggerConsoleEntry::info(format!(
                        "{} session started",
                        debugger_name
                    )));
                    ui_events.push(DebuggerUiEvent::SessionStarted {
                        target: self.active_target_name.clone(),
                    });
                }
                DebuggerUiEvent::SessionError { message } => {
                    self.push_console(DebuggerConsoleEntry::error(message.clone()));
                    ui_events.push(DebuggerUiEvent::SessionError { message });
                }
            }
        }

        ui_events
    }

    pub fn push_console(&mut self, entry: DebuggerConsoleEntry) {
        self.console.push(entry);
        if self.console.len() > MAX_CONSOLE_ENTRIES {
            let overflow = self.console.len() - MAX_CONSOLE_ENTRIES;
            self.console.drain(0..overflow);
            if self.console_cursor >= overflow {
                self.console_cursor -= overflow;
            } else {
                self.console_cursor = 0;
            }
        }
    }

    fn allocate_target_id(&mut self) -> u64 {
        let id = self.next_target_id;
        self.next_target_id = self.next_target_id.wrapping_add(1);
        id
    }

    fn allocate_breakpoint_id(&mut self) -> u64 {
        let id = self.next_breakpoint_id;
        self.next_breakpoint_id = self.next_breakpoint_id.wrapping_add(1);
        id
    }

    fn recalculate_next_target_id(&mut self) {
        self.next_target_id = self
            .targets
            .iter()
            .map(|target| target.id)
            .max()
            .unwrap_or(0)
            .wrapping_add(1);
    }
}

fn normalize_executable_path(path: &Path) -> String {
    let display = path.to_string_lossy().to_string();
    if cfg!(windows) {
        display.replace('\\', "/")
    } else {
        display
    }
}

#[derive(Clone, Debug)]
enum DebuggerRuntime {
    Gdb {
        commands: Sender<GdbCommand>,
        events: Receiver<GdbEvent>,
    },
    Vedit {
        commands: Sender<VeditCommand>,
        events: Receiver<VeditEvent>,
    },
}

impl DebuggerRuntime {
    fn new_gdb(session: GdbSession) -> Self {
        Self::Gdb {
            commands: session.command_sender(),
            events: session.event_receiver(),
        }
    }

    fn new_vedit(session: VeditSession) -> Self {
        Self::Vedit {
            commands: session.command_sender(),
            events: session.event_receiver(),
        }
    }

    fn send_gdb(&self, command: GdbCommand) {
        if let Self::Gdb { commands, .. } = self {
            let _ = commands.send(command);
        }
    }

    fn send_vedit(&self, command: VeditCommand) {
        if let Self::Vedit { commands, .. } = self {
            let _ = commands.send(command);
        }
    }

    fn try_recv(&self) -> Option<DebuggerUiEvent> {
        match self {
            Self::Gdb { events, .. } => events.try_recv().ok().map(|event| match event {
                GdbEvent::Started => DebuggerUiEvent::SessionStarted { target: None },
                GdbEvent::Stdout(line) => DebuggerUiEvent::SessionError {
                    message: format!("stdout: {}", line),
                },
                GdbEvent::Stderr(line) => DebuggerUiEvent::SessionError {
                    message: format!("stderr: {}", line),
                },
                GdbEvent::Exited(code) => DebuggerUiEvent::SessionError {
                    message: format!("exited with code {}", code),
                },
                GdbEvent::Error(err) => DebuggerUiEvent::SessionError { message: err },
            }),
            Self::Vedit { events, .. } => events.try_recv().ok().map(|event| match event {
                VeditEvent::Started => DebuggerUiEvent::SessionStarted { target: None },
                VeditEvent::Stopped { reason } => DebuggerUiEvent::SessionError {
                    message: format!("stopped: {:?}", reason),
                },
                VeditEvent::Exited(code) => DebuggerUiEvent::SessionError {
                    message: format!("exited with code {}", code),
                },
                VeditEvent::Error(err) => DebuggerUiEvent::SessionError { message: err },
                VeditEvent::MemoryRead(_) => DebuggerUiEvent::SessionError {
                    message: "memory read".to_string(),
                },
                VeditEvent::Disassembly(_) => DebuggerUiEvent::SessionError {
                    message: "disassembly".to_string(),
                },
            }),
        }
    }
}

fn scan_workspace(
    root: &Path,
    vcx_projects: &mut BTreeSet<PathBuf>,
    makefiles: &mut BTreeSet<PathBuf>,
    warnings: &mut Vec<String>,
) {
    let read_dir = match fs::read_dir(root) {
        Ok(read_dir) => read_dir,
        Err(err) => {
            warnings.push(format!("Unable to read {}: {}", root.display(), err));
            return;
        }
    };

    for entry in read_dir {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                warnings.push(format!("Failed to read directory entry: {}", err));
                continue;
            }
        };

        let path = entry.path();
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(err) => {
                warnings.push(format!(
                    "Failed to resolve file type for {}: {}",
                    path.display(),
                    err
                ));
                continue;
            }
        };

        if file_type.is_dir() {
            if should_ignore_dir(&path) {
                continue;
            }
            scan_workspace(&path, vcx_projects, makefiles, warnings);
            continue;
        }

        if !file_type.is_file() {
            continue;
        }

        if is_solution(&path) {
            match Solution::from_path(&path) {
                Ok(solution) => {
                    for project in solution.projects {
                        vcx_projects.insert(project.absolute_path.clone());
                    }
                }
                Err(err) => warnings.push(err.to_string()),
            }
            continue;
        }

        if is_vcxproj(&path) {
            vcx_projects.insert(path.clone());
            continue;
        }

        if is_makefile(&path) {
            makefiles.insert(path.clone());
        }
    }
}

fn should_ignore_dir(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(OsStr::to_str) else {
        return false;
    };
    IGNORED_DIRECTORIES
        .iter()
        .any(|ignored| name.eq_ignore_ascii_case(ignored))
}

fn is_solution(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .map(|ext| ext.eq_ignore_ascii_case("sln"))
        == Some(true)
}

fn is_vcxproj(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .map(|ext| ext.eq_ignore_ascii_case("vcxproj"))
        == Some(true)
}

fn is_makefile(path: &Path) -> bool {
    path.file_name().and_then(OsStr::to_str).map(|name| {
        let lowercase = name.to_ascii_lowercase();
        lowercase == "makefile" || lowercase.ends_with(".mk") || lowercase == "gnu makefile"
    }) == Some(true)
}

fn guess_vcx_executable(project_path: &Path, project_name: &str) -> PathBuf {
    let parent = project_path.parent().unwrap_or_else(|| Path::new("."));
    let mut candidate_names = Vec::new();
    #[cfg(windows)]
    let exe_name = format!("{}.exe", project_name);
    #[cfg(not(windows))]
    let exe_name = project_name.to_string();

    candidate_names.push(parent.join(&exe_name));
    candidate_names.push(parent.join("Debug").join(&exe_name));
    candidate_names.push(parent.join("RelWithDebInfo").join(&exe_name));
    candidate_names.push(parent.join("Release").join(&exe_name));

    for candidate in candidate_names {
        if candidate.exists() {
            return candidate;
        }
    }

    parent.join(exe_name)
}

fn guess_makefile_executable(path: &Path) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let mut candidate = parent.join("a.out");
    if candidate.exists() {
        return candidate;
    }

    if let Some(stem) = path.file_stem().and_then(OsStr::to_str) {
        #[cfg(windows)]
        let exe_name = format!("{}.exe", stem);
        #[cfg(not(windows))]
        let exe_name = stem.to_string();

        let fallback = parent.join(exe_name);
        if fallback.exists() {
            return fallback;
        }
        candidate = fallback;
    }

    candidate
}

fn display_path(path: &Path) -> String {
    path.display().to_string()
}

fn looks_like_library(path: &Path) -> bool {
    let file_name = path
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or_default()
        .to_ascii_lowercase();

    let ext = path
        .extension()
        .and_then(OsStr::to_str)
        .map(|ext| ext.to_ascii_lowercase());

    if let Some(ext) = ext.as_deref() {
        return matches!(
            ext,
            "a" | "la" | "lib" | "dll" | "so" | "dylib" | "rlib" | "bc" | "o"
        );
    }

    file_name.starts_with("lib")
}
