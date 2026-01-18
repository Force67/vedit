//! Wine debugging support
//!
//! This module provides debugging capabilities for Windows executables
//! running under Wine/Proton using either winedbg or native Linux GDB.

use crate::error::{WineError, WineResult};
use crate::msbuild::{linux_to_wine_path, wine_to_linux_path};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

/// Type of debugger to use for Wine debugging
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WineDebuggerType {
    /// Use winedbg - Wine's built-in debugger
    /// Pros: Native Windows debugging experience, better symbol support
    /// Cons: Less powerful than GDB, Wine-specific
    Winedbg,
    /// Use native Linux GDB to debug the Wine process
    /// Pros: Full GDB power, better for mixed debugging
    /// Cons: Requires some setup, paths need translation
    NativeGdb,
}

impl Default for WineDebuggerType {
    fn default() -> Self {
        Self::Winedbg
    }
}

/// Configuration for a Wine debug session
#[derive(Debug, Clone)]
pub struct WineDebugConfig {
    /// Type of debugger to use
    pub debugger_type: WineDebuggerType,
    /// Path to the Windows executable (Linux path)
    pub executable: PathBuf,
    /// Working directory (Linux path)
    pub working_directory: Option<PathBuf>,
    /// Command-line arguments for the executable
    pub arguments: Vec<String>,
    /// Environment variables to set
    pub environment: HashMap<String, String>,
    /// Initial breakpoints (source file, line number)
    pub breakpoints: Vec<WineBreakpoint>,
    /// Wine environment ID to use
    pub environment_id: String,
    /// Wine prefix path
    pub wine_prefix: PathBuf,
    /// Wine executable path (wine or proton run)
    pub wine_executable: PathBuf,
}

/// A breakpoint for Wine debugging
#[derive(Debug, Clone)]
pub struct WineBreakpoint {
    /// Source file path (Linux path)
    pub file: PathBuf,
    /// Line number
    pub line: u32,
    /// Optional condition expression
    pub condition: Option<String>,
    /// Whether the breakpoint is enabled
    pub enabled: bool,
}

/// Events emitted by a Wine debug session
#[derive(Debug, Clone)]
pub enum WineDebugEvent {
    /// Debug session started
    Started,
    /// Debugger output (stdout/stderr)
    Output(String),
    /// Breakpoint hit
    BreakpointHit {
        file: PathBuf,
        line: u32,
        function: Option<String>,
    },
    /// Program stopped (e.g., SIGTRAP, SIGSEGV)
    Stopped {
        reason: String,
        address: Option<u64>,
    },
    /// Variable value retrieved
    VariableValue {
        name: String,
        value: String,
        type_name: Option<String>,
    },
    /// Stack frame information
    StackFrame {
        level: u32,
        function: String,
        file: Option<PathBuf>,
        line: Option<u32>,
        address: u64,
    },
    /// Program continued execution
    Continued,
    /// Program exited
    Exited { exit_code: i32 },
    /// Debug session ended
    Ended,
    /// Error occurred
    Error(String),
}

/// Commands that can be sent to a Wine debug session
#[derive(Debug, Clone)]
pub enum WineDebugCommand {
    /// Continue execution
    Continue,
    /// Step over (next line)
    StepOver,
    /// Step into (enter function)
    StepInto,
    /// Step out (exit function)
    StepOut,
    /// Pause execution
    Pause,
    /// Set a breakpoint
    SetBreakpoint(WineBreakpoint),
    /// Remove a breakpoint
    RemoveBreakpoint { file: PathBuf, line: u32 },
    /// Evaluate an expression
    Evaluate(String),
    /// Get local variables
    GetLocals,
    /// Get stack trace
    GetStackTrace,
    /// Terminate the debug session
    Terminate,
}

/// A Wine debug session
pub struct WineDebugSession {
    /// Debug configuration
    config: WineDebugConfig,
    /// Child process (the debugger)
    process: Option<Child>,
    /// Channel to send commands
    command_tx: Option<mpsc::Sender<WineDebugCommand>>,
}

impl WineDebugSession {
    /// Create a new debug session with the given configuration
    pub fn new(config: WineDebugConfig) -> Self {
        Self {
            config,
            process: None,
            command_tx: None,
        }
    }

    /// Start the debug session
    pub async fn start(&mut self) -> WineResult<mpsc::Receiver<WineDebugEvent>> {
        let (event_tx, event_rx) = mpsc::channel(256);
        let (command_tx, command_rx) = mpsc::channel(64);

        match self.config.debugger_type {
            WineDebuggerType::Winedbg => {
                self.start_winedbg(event_tx, command_rx).await?;
            }
            WineDebuggerType::NativeGdb => {
                self.start_native_gdb(event_tx, command_rx).await?;
            }
        }

        self.command_tx = Some(command_tx);

        Ok(event_rx)
    }

    /// Start debugging with winedbg
    async fn start_winedbg(
        &mut self,
        event_tx: mpsc::Sender<WineDebugEvent>,
        mut command_rx: mpsc::Receiver<WineDebugCommand>,
    ) -> WineResult<()> {
        let wine_exe = &self.config.wine_executable;
        let wine_path = linux_to_wine_path(&self.config.executable, &self.config.wine_prefix);

        let mut cmd = Command::new(wine_exe);
        cmd.arg("winedbg")
            .arg("--gdb") // Use GDB protocol for better integration
            .arg(&wine_path);

        // Add arguments
        for arg in &self.config.arguments {
            cmd.arg(arg);
        }

        // Set environment
        cmd.env("WINEPREFIX", &self.config.wine_prefix);
        for (key, value) in &self.config.environment {
            cmd.env(key, value);
        }

        // Set working directory
        if let Some(ref wd) = self.config.working_directory {
            cmd.current_dir(wd);
        }

        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| {
            WineError::DebugSessionFailed(format!("Failed to start winedbg: {}", e))
        })?;

        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        let stdin = child.stdin.take().unwrap();

        self.process = Some(child);

        // Spawn task to handle output
        let event_tx_clone = event_tx.clone();
        let prefix_path = self.config.wine_prefix.clone();
        tokio::spawn(async move {
            let _ = event_tx_clone.send(WineDebugEvent::Started).await;

            let mut stdout_reader = BufReader::new(stdout).lines();
            let mut stderr_reader = BufReader::new(stderr).lines();

            loop {
                tokio::select! {
                    line = stdout_reader.next_line() => {
                        match line {
                            Ok(Some(text)) => {
                                let event = parse_winedbg_output(&text, &prefix_path);
                                if event_tx_clone.send(event).await.is_err() {
                                    break;
                                }
                            }
                            Ok(None) => break,
                            Err(_) => break,
                        }
                    }
                    line = stderr_reader.next_line() => {
                        match line {
                            Ok(Some(text)) => {
                                let _ = event_tx_clone.send(WineDebugEvent::Output(text)).await;
                            }
                            Ok(None) => break,
                            Err(_) => break,
                        }
                    }
                }
            }

            let _ = event_tx_clone.send(WineDebugEvent::Ended).await;
        });

        // Spawn task to handle commands
        let event_tx_clone = event_tx.clone();
        let prefix_path = self.config.wine_prefix.clone();
        tokio::spawn(async move {
            let mut stdin = stdin;
            while let Some(cmd) = command_rx.recv().await {
                let gdb_cmd = wine_debug_command_to_gdb(&cmd, &prefix_path);
                if let Err(e) = stdin.write_all(format!("{}\n", gdb_cmd).as_bytes()).await {
                    let _ = event_tx_clone
                        .send(WineDebugEvent::Error(format!(
                            "Failed to send command: {}",
                            e
                        )))
                        .await;
                    break;
                }
                let _ = stdin.flush().await;

                if matches!(cmd, WineDebugCommand::Terminate) {
                    break;
                }
            }
        });

        Ok(())
    }

    /// Start debugging with native Linux GDB
    async fn start_native_gdb(
        &mut self,
        event_tx: mpsc::Sender<WineDebugEvent>,
        mut command_rx: mpsc::Receiver<WineDebugCommand>,
    ) -> WineResult<()> {
        // For native GDB, we need to:
        // 1. Start Wine with the executable
        // 2. Attach GDB to the Wine process

        let wine_exe = &self.config.wine_executable;
        let wine_path = linux_to_wine_path(&self.config.executable, &self.config.wine_prefix);

        let mut cmd = Command::new("gdb");
        cmd.arg("--interpreter=mi3") // Use MI protocol for structured output
            .arg("-ex")
            .arg(format!(
                "set environment WINEPREFIX={}",
                self.config.wine_prefix.display()
            ))
            .arg("-ex")
            .arg(format!("file {}", wine_exe.display()))
            .arg("-ex")
            .arg(format!(
                "set args {} {}",
                wine_path,
                self.config.arguments.join(" ")
            ));

        // Set initial breakpoints
        for bp in &self.config.breakpoints {
            if bp.enabled {
                let wine_file = linux_to_wine_path(&bp.file, &self.config.wine_prefix);
                cmd.arg("-ex")
                    .arg(format!("break {}:{}", wine_file, bp.line));
            }
        }

        // Start the program
        cmd.arg("-ex").arg("run");

        // Set environment
        for (key, value) in &self.config.environment {
            cmd.env(key, value);
        }

        // Set working directory
        if let Some(ref wd) = self.config.working_directory {
            cmd.current_dir(wd);
        }

        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| WineError::DebugSessionFailed(format!("Failed to start GDB: {}", e)))?;

        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        let stdin = child.stdin.take().unwrap();

        self.process = Some(child);

        // Spawn task to handle output
        let event_tx_clone = event_tx.clone();
        let prefix_path = self.config.wine_prefix.clone();
        tokio::spawn(async move {
            let _ = event_tx_clone.send(WineDebugEvent::Started).await;

            let mut stdout_reader = BufReader::new(stdout).lines();
            let mut stderr_reader = BufReader::new(stderr).lines();

            loop {
                tokio::select! {
                    line = stdout_reader.next_line() => {
                        match line {
                            Ok(Some(text)) => {
                                let event = parse_gdb_mi_output(&text, &prefix_path);
                                if event_tx_clone.send(event).await.is_err() {
                                    break;
                                }
                            }
                            Ok(None) => break,
                            Err(_) => break,
                        }
                    }
                    line = stderr_reader.next_line() => {
                        match line {
                            Ok(Some(text)) => {
                                let _ = event_tx_clone.send(WineDebugEvent::Output(text)).await;
                            }
                            Ok(None) => break,
                            Err(_) => break,
                        }
                    }
                }
            }

            let _ = event_tx_clone.send(WineDebugEvent::Ended).await;
        });

        // Spawn task to handle commands
        let event_tx_clone = event_tx.clone();
        let prefix_path = self.config.wine_prefix.clone();
        tokio::spawn(async move {
            let mut stdin = stdin;
            while let Some(cmd) = command_rx.recv().await {
                let gdb_cmd = wine_debug_command_to_gdb_mi(&cmd, &prefix_path);
                if let Err(e) = stdin.write_all(format!("{}\n", gdb_cmd).as_bytes()).await {
                    let _ = event_tx_clone
                        .send(WineDebugEvent::Error(format!(
                            "Failed to send command: {}",
                            e
                        )))
                        .await;
                    break;
                }
                let _ = stdin.flush().await;

                if matches!(cmd, WineDebugCommand::Terminate) {
                    break;
                }
            }
        });

        Ok(())
    }

    /// Send a command to the debug session
    pub async fn send_command(&self, command: WineDebugCommand) -> WineResult<()> {
        if let Some(ref tx) = self.command_tx {
            tx.send(command).await.map_err(|e| {
                WineError::DebugSessionFailed(format!("Failed to send command: {}", e))
            })
        } else {
            Err(WineError::DebugSessionFailed(
                "Debug session not started".to_string(),
            ))
        }
    }

    /// Terminate the debug session
    pub async fn terminate(&mut self) -> WineResult<()> {
        if let Some(ref tx) = self.command_tx {
            let _ = tx.send(WineDebugCommand::Terminate).await;
        }

        if let Some(ref mut process) = self.process {
            let _ = process.kill().await;
        }

        self.process = None;
        self.command_tx = None;

        Ok(())
    }

    /// Get the debugger type being used
    pub fn debugger_type(&self) -> WineDebuggerType {
        self.config.debugger_type
    }

    /// Get the executable path
    pub fn executable(&self) -> &PathBuf {
        &self.config.executable
    }

    /// Get the wine prefix path
    pub fn wine_prefix(&self) -> &PathBuf {
        &self.config.wine_prefix
    }

    /// Translate a Linux path to Wine path for breakpoints
    pub fn translate_path_to_wine(&self, linux_path: &PathBuf) -> String {
        linux_to_wine_path(linux_path, &self.config.wine_prefix)
    }

    /// Translate a Wine path from debugger output to Linux path
    pub fn translate_path_to_linux(&self, wine_path: &str) -> PathBuf {
        wine_to_linux_path(wine_path, &self.config.wine_prefix)
    }
}

/// Parse winedbg GDB-protocol output into events
fn parse_winedbg_output(line: &str, _prefix_path: &Path) -> WineDebugEvent {
    // Basic GDB output parsing
    if line.starts_with("*stopped") {
        // Parse stop reason
        if line.contains("breakpoint-hit") {
            // Extract file and line if available
            WineDebugEvent::BreakpointHit {
                file: PathBuf::from("unknown"),
                line: 0,
                function: None,
            }
        } else {
            WineDebugEvent::Stopped {
                reason: line.to_string(),
                address: None,
            }
        }
    } else if line.starts_with("*running") {
        WineDebugEvent::Continued
    } else if line.starts_with("^exit") || line.contains("exited") {
        WineDebugEvent::Exited { exit_code: 0 }
    } else {
        WineDebugEvent::Output(line.to_string())
    }
}

/// Parse GDB MI (Machine Interface) output into events
fn parse_gdb_mi_output(line: &str, prefix_path: &Path) -> WineDebugEvent {
    // GDB MI output format:
    // *stopped,reason="breakpoint-hit",frame={...}
    // *running,thread-id="all"
    // ^done,value="..."
    // ~"output string\n"
    // @"target output\n"
    // &"log output\n"

    if line.starts_with('*') {
        // Async record (execution state change)
        if line.starts_with("*stopped") {
            if line.contains("breakpoint-hit") {
                // Parse breakpoint info
                // *stopped,reason="breakpoint-hit",disp="keep",bkptno="1",frame={...}
                let file = extract_mi_string(line, "fullname")
                    .map(|s| wine_to_linux_path(&s, prefix_path))
                    .unwrap_or_else(|| PathBuf::from("unknown"));
                let line_num = extract_mi_number(line, "line").unwrap_or(0);
                let function = extract_mi_string(line, "func");

                WineDebugEvent::BreakpointHit {
                    file,
                    line: line_num,
                    function,
                }
            } else if line.contains("exited") {
                let exit_code = extract_mi_number(line, "exit-code")
                    .map(|n| n as i32)
                    .unwrap_or(0);
                WineDebugEvent::Exited { exit_code }
            } else {
                let reason =
                    extract_mi_string(line, "reason").unwrap_or_else(|| "unknown".to_string());
                let address = extract_mi_number(line, "addr").map(|n| n as u64);
                WineDebugEvent::Stopped { reason, address }
            }
        } else if line.starts_with("*running") {
            WineDebugEvent::Continued
        } else {
            WineDebugEvent::Output(line.to_string())
        }
    } else if line.starts_with('~') {
        // Console output
        let text = line
            .trim_start_matches('~')
            .trim_matches('"')
            .replace("\\n", "\n")
            .replace("\\t", "\t");
        WineDebugEvent::Output(text)
    } else if line.starts_with('^') {
        // Result record
        if line.starts_with("^error") {
            let msg = extract_mi_string(line, "msg").unwrap_or_else(|| line.to_string());
            WineDebugEvent::Error(msg)
        } else {
            WineDebugEvent::Output(line.to_string())
        }
    } else {
        WineDebugEvent::Output(line.to_string())
    }
}

/// Extract a string value from GDB MI output
fn extract_mi_string(line: &str, key: &str) -> Option<String> {
    let pattern = format!("{}=\"", key);
    if let Some(start) = line.find(&pattern) {
        let start = start + pattern.len();
        let rest = &line[start..];
        if let Some(end) = rest.find('"') {
            return Some(rest[..end].to_string());
        }
    }
    None
}

/// Extract a numeric value from GDB MI output
fn extract_mi_number(line: &str, key: &str) -> Option<u32> {
    extract_mi_string(line, key).and_then(|s| s.parse().ok())
}

/// Convert a WineDebugCommand to a GDB command string (for winedbg --gdb)
fn wine_debug_command_to_gdb(cmd: &WineDebugCommand, prefix_path: &Path) -> String {
    match cmd {
        WineDebugCommand::Continue => "c".to_string(),
        WineDebugCommand::StepOver => "n".to_string(),
        WineDebugCommand::StepInto => "s".to_string(),
        WineDebugCommand::StepOut => "finish".to_string(),
        WineDebugCommand::Pause => "\x03".to_string(), // Ctrl+C
        WineDebugCommand::SetBreakpoint(bp) => {
            let wine_file = linux_to_wine_path(&bp.file, prefix_path);
            if let Some(ref cond) = bp.condition {
                format!("break {}:{} if {}", wine_file, bp.line, cond)
            } else {
                format!("break {}:{}", wine_file, bp.line)
            }
        }
        WineDebugCommand::RemoveBreakpoint { file, line } => {
            let wine_file = linux_to_wine_path(file, prefix_path);
            format!("clear {}:{}", wine_file, line)
        }
        WineDebugCommand::Evaluate(expr) => format!("print {}", expr),
        WineDebugCommand::GetLocals => "info locals".to_string(),
        WineDebugCommand::GetStackTrace => "bt".to_string(),
        WineDebugCommand::Terminate => "quit".to_string(),
    }
}

/// Convert a WineDebugCommand to a GDB MI command string
fn wine_debug_command_to_gdb_mi(cmd: &WineDebugCommand, prefix_path: &Path) -> String {
    match cmd {
        WineDebugCommand::Continue => "-exec-continue".to_string(),
        WineDebugCommand::StepOver => "-exec-next".to_string(),
        WineDebugCommand::StepInto => "-exec-step".to_string(),
        WineDebugCommand::StepOut => "-exec-finish".to_string(),
        WineDebugCommand::Pause => "-exec-interrupt".to_string(),
        WineDebugCommand::SetBreakpoint(bp) => {
            let wine_file = linux_to_wine_path(&bp.file, prefix_path);
            if let Some(ref cond) = bp.condition {
                format!("-break-insert -c \"{}\" {}:{}", cond, wine_file, bp.line)
            } else {
                format!("-break-insert {}:{}", wine_file, bp.line)
            }
        }
        WineDebugCommand::RemoveBreakpoint { file, line } => {
            let wine_file = linux_to_wine_path(file, prefix_path);
            format!("-break-delete {}:{}", wine_file, line)
        }
        WineDebugCommand::Evaluate(expr) => format!("-data-evaluate-expression \"{}\"", expr),
        WineDebugCommand::GetLocals => "-stack-list-locals --all-values".to_string(),
        WineDebugCommand::GetStackTrace => "-stack-list-frames".to_string(),
        WineDebugCommand::Terminate => "-gdb-exit".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_gdb_mi_breakpoint() {
        let line = r#"*stopped,reason="breakpoint-hit",disp="keep",bkptno="1",frame={addr="0x00401000",func="main",args=[],file="main.cpp",fullname="Z:\\home\\user\\project\\main.cpp",line="10"}"#;
        let prefix = PathBuf::from("/home/user/.wine");
        let event = parse_gdb_mi_output(line, &prefix);
        match event {
            WineDebugEvent::BreakpointHit {
                file,
                line,
                function,
            } => {
                assert!(file.to_string_lossy().contains("main.cpp"));
                assert_eq!(line, 10);
                assert_eq!(function, Some("main".to_string()));
            }
            _ => panic!("Expected BreakpointHit event"),
        }
    }

    #[test]
    fn test_debug_command_to_gdb_mi() {
        let prefix = PathBuf::from("/home/user/.wine");
        assert_eq!(
            wine_debug_command_to_gdb_mi(&WineDebugCommand::Continue, &prefix),
            "-exec-continue"
        );
        assert_eq!(
            wine_debug_command_to_gdb_mi(&WineDebugCommand::StepOver, &prefix),
            "-exec-next"
        );
    }
}
