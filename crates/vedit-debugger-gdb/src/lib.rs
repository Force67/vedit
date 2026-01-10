use crossbeam_channel::{Receiver, Sender, unbounded};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use thiserror::Error;

static SESSION_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

#[derive(Debug, Error)]
pub enum DebuggerError {
    #[error("Failed to spawn gdb: {0}")]
    Spawn(#[from] std::io::Error),
    #[error("Debugger stdin unavailable")]
    NoStdin,
    #[error("Debugger process exited unexpectedly")]
    ProcessExited,
}

#[derive(Debug, Clone)]
pub struct Breakpoint {
    pub file: PathBuf,
    pub line: u32,
    pub condition: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LaunchConfig {
    pub executable: PathBuf,
    pub working_directory: PathBuf,
    pub arguments: Vec<String>,
    pub breakpoints: Vec<Breakpoint>,
    pub launch_script: Option<String>,
    pub gdb_path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub enum DebuggerCommand {
    SendRaw(String),
    Continue,
    Kill,
}

#[derive(Debug, Clone)]
pub enum DebuggerEvent {
    Started,
    Stdout(String),
    Stderr(String),
    Exited(i32),
    Error(String),
}

#[derive(Clone, Debug)]
pub struct GdbSession {
    id: u64,
    command_sender: Sender<DebuggerCommand>,
    event_receiver: Receiver<DebuggerEvent>,
}

impl GdbSession {
    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn command_sender(&self) -> Sender<DebuggerCommand> {
        self.command_sender.clone()
    }

    pub fn event_receiver(&self) -> Receiver<DebuggerEvent> {
        self.event_receiver.clone()
    }
}

pub fn spawn_session(config: LaunchConfig) -> Result<GdbSession, DebuggerError> {
    let gdb = config
        .gdb_path
        .clone()
        .unwrap_or_else(|| PathBuf::from("gdb"));

    let mut command = Command::new(&gdb);
    command
        .arg("-q")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .current_dir(&config.working_directory);

    let mut child = command.spawn()?;
    let stdin = child.stdin.take().ok_or(DebuggerError::NoStdin)?;
    let stdout = child.stdout.take().ok_or(DebuggerError::ProcessExited)?;
    let stderr = child.stderr.take().ok_or(DebuggerError::ProcessExited)?;

    let (command_sender, command_receiver) = unbounded();
    let (event_sender, event_receiver) = unbounded();

    let stdin = Arc::new(Mutex::new(stdin));
    let child_arc = Arc::new(Mutex::new(child));

    let stdout_sender = event_sender.clone();
    thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            match line {
                Ok(line) => {
                    if stdout_sender.send(DebuggerEvent::Stdout(line)).is_err() {
                        break;
                    }
                }
                Err(err) => {
                    let _ = stdout_sender.send(DebuggerEvent::Error(err.to_string()));
                    break;
                }
            }
        }
    });

    let stderr_sender = event_sender.clone();
    thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            match line {
                Ok(line) => {
                    if stderr_sender.send(DebuggerEvent::Stderr(line)).is_err() {
                        break;
                    }
                }
                Err(err) => {
                    let _ = stderr_sender.send(DebuggerEvent::Error(err.to_string()));
                    break;
                }
            }
        }
    });

    initialise_session(&stdin, &event_sender, &config);

    let stdin_for_commands = stdin.clone();
    let child_for_commands = child_arc.clone();
    let command_event_sender = event_sender.clone();
    thread::spawn(move || {
        while let Ok(command) = command_receiver.recv() {
            match command {
                DebuggerCommand::SendRaw(value) => {
                    if let Err(err) = send_line(&stdin_for_commands, &value) {
                        let _ = command_event_sender.send(DebuggerEvent::Error(err.to_string()));
                        break;
                    }
                }
                DebuggerCommand::Continue => {
                    if let Err(err) = send_line(&stdin_for_commands, "continue") {
                        let _ = command_event_sender.send(DebuggerEvent::Error(err.to_string()));
                        break;
                    }
                }
                DebuggerCommand::Kill => {
                    if let Ok(mut child) = child_for_commands.lock() {
                        let _ = child.kill();
                    }
                    break;
                }
            }
        }
    });

    let wait_sender = event_sender.clone();
    thread::spawn(move || {
        let status = {
            let mut child = child_arc.lock().expect("child lock poisoned");
            child.wait()
        };
        match status {
            Ok(status) => {
                let code = status.code().unwrap_or(-1);
                let _ = wait_sender.send(DebuggerEvent::Exited(code));
            }
            Err(err) => {
                let _ = wait_sender.send(DebuggerEvent::Error(err.to_string()));
            }
        }
    });

    Ok(GdbSession {
        id: SESSION_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        command_sender,
        event_receiver,
    })
}

fn initialise_session(
    stdin: &Arc<Mutex<ChildStdin>>,
    event_sender: &Sender<DebuggerEvent>,
    config: &LaunchConfig,
) {
    let mut failures = Vec::new();
    if let Err(err) = send_line(stdin, &format!("file {}", quote_path(&config.executable))) {
        failures.push(err.to_string());
    }

    if let Err(err) = send_line(
        stdin,
        &format!("cd {}", quote_path(&config.working_directory)),
    ) {
        failures.push(err.to_string());
    }

    for breakpoint in &config.breakpoints {
        let mut command = format!("break {}:{}", quote_path(&breakpoint.file), breakpoint.line);
        if let Some(condition) = &breakpoint.condition {
            if !condition.trim().is_empty() {
                command.push_str(" if ");
                command.push_str(condition);
            }
        }
        if let Err(err) = send_line(stdin, &command) {
            failures.push(err.to_string());
        }
    }

    if let Some(script) = &config.launch_script {
        for line in script.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Err(err) = send_line(stdin, trimmed) {
                failures.push(err.to_string());
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    if !config.arguments.is_empty() {
        let args = config
            .arguments
            .iter()
            .map(|arg| quote_arg(arg))
            .collect::<Vec<_>>()
            .join(" ");
        if let Err(err) = send_line(stdin, &format!("set args {}", args)) {
            failures.push(err.to_string());
        }
    }

    if let Err(err) = send_line(stdin, "run") {
        failures.push(err.to_string());
    }

    if failures.is_empty() {
        let _ = event_sender.send(DebuggerEvent::Started);
    } else {
        for failure in failures {
            let _ = event_sender.send(DebuggerEvent::Error(failure));
        }
    }
}

fn send_line(stdin: &Arc<Mutex<ChildStdin>>, line: &str) -> Result<(), std::io::Error> {
    let mut writer = stdin.lock().expect("gdb stdin poisoned");
    writer.write_all(line.as_bytes())?;
    writer.write_all(b"\n")?;
    writer.flush()
}

fn quote_path(path: impl AsRef<Path>) -> String {
    let text = path.as_ref().to_string_lossy();
    if text
        .chars()
        .all(|c| c.is_alphanumeric() || ".-_/".contains(c))
    {
        text.to_string()
    } else {
        format!("\"{}\"", text.replace('"', "\\\""))
    }
}

fn quote_arg(arg: &str) -> String {
    if arg
        .chars()
        .all(|c| c.is_alphanumeric() || ".-_/".contains(c))
    {
        arg.to_string()
    } else {
        format!("\"{}\"", arg.replace('"', "\\\""))
    }
}
