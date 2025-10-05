use crossbeam_channel::{unbounded, Receiver};
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::thread;

const DEFAULT_ROWS: u16 = 24;
const DEFAULT_COLS: u16 = 80;
const MAX_LINES: usize = 2000;

#[derive(Debug)]
pub struct ConsoleState {
    tabs: Vec<ConsoleTab>,
    visible: bool,
    active_tab: Option<u64>,
    next_id: u64,
}

impl ConsoleState {
    pub fn new() -> Self {
        Self {
            tabs: Vec::new(),
            visible: false,
            active_tab: None,
            next_id: 1,
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    pub fn tabs(&self) -> &[ConsoleTab] {
        &self.tabs
    }

    pub fn active_tab_id(&self) -> Option<u64> {
        self.active_tab
    }

    pub fn select_tab(&mut self, id: u64) -> bool {
        if self.tabs.iter().any(|tab| tab.id == id) {
            self.active_tab = Some(id);
            true
        } else {
            false
        }
    }

    pub fn tab_mut(&mut self, id: u64) -> Option<&mut ConsoleTab> {
        self.tabs.iter_mut().find(|tab| tab.id == id)
    }

    pub fn active_tab(&self) -> Option<&ConsoleTab> {
        let id = self.active_tab?;
        self.tabs.iter().find(|tab| tab.id == id)
    }

    pub fn shell_tab_count(&self) -> usize {
        self
            .tabs
            .iter()
            .filter(|tab| tab.kind == ConsoleKind::Shell)
            .count()
    }

    pub fn select_shell_at(&mut self, index: usize) {
        let mut cursor = 0;
        for tab in &self.tabs {
            if tab.kind == ConsoleKind::Shell {
                if cursor == index {
                    self.active_tab = Some(tab.id);
                    return;
                }
                cursor += 1;
            }
        }
    }

    pub fn spawn_shell_tab(&mut self) -> Result<u64, String> {
        let id = self.allocate_id();
        let runtime = ConsoleRuntime::spawn_shell()?;
        let tab = ConsoleTab::new_shell(id, runtime);
        self.tabs.push(tab);
        self.active_tab = Some(id);
        Ok(id)
    }

    pub fn create_debug_tab(&mut self, title: String) -> u64 {
        let id = self.allocate_id();
        let tab = ConsoleTab::new_debug(id, title);
        self.tabs.push(tab);
        self.active_tab = Some(id);
        id
    }

    pub fn process_events(&mut self) {
        for tab in &mut self.tabs {
            let mut events = Vec::new();
            if let Some(runtime) = tab.runtime() {
                while let Some(event) = runtime.try_recv() {
                    events.push(event);
                }
            }

            for event in events {
                match event {
                    ConsoleRuntimeEvent::Output(text) => {
                        tab.append_output(&text);
                    }
                    ConsoleRuntimeEvent::Exit(code) => {
                        tab.handle_exit(code);
                    }
                    ConsoleRuntimeEvent::Error(message) => {
                        tab.append_line(ConsoleLineKind::Error, &message);
                        tab.handle_exit(-1);
                    }
                }
            }
        }
    }

    pub fn push_lines(&mut self, id: u64, entries: Vec<(ConsoleLineKind, String)>) {
        if let Some(tab) = self.tab_mut(id) {
            for (kind, text) in entries {
                tab.append_line(kind, &text);
            }
        }
    }

    pub fn set_input(&mut self, id: u64, value: String) {
        if let Some(tab) = self.tab_mut(id) {
            tab.set_input(value);
        }
    }

    pub fn submit_input(&mut self, id: u64) -> Result<(), String> {
        if let Some(tab) = self.tab_mut(id) {
            tab.submit_input()
        } else {
            Err("Console tab not found".to_string())
        }
    }

    fn allocate_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        id
    }
}

#[derive(Clone, Debug)]
pub struct ConsoleLine {
    pub kind: ConsoleLineKind,
    pub text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConsoleLineKind {
    Output,
    Error,
    Info,
    Command,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConsoleStatus {
    Running,
    Exited(i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsoleKind {
    Shell,
    Debug,
}

pub struct ConsoleTab {
    id: u64,
    title: String,
    input: String,
    lines: Vec<ConsoleLine>,
    pending: String,
    status: ConsoleStatus,
    kind: ConsoleKind,
    runtime: Option<ConsoleRuntime>,
}

impl std::fmt::Debug for ConsoleTab {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConsoleTab")
            .field("id", &self.id)
            .field("title", &self.title)
            .field("input", &self.input)
            .field("lines", &self.lines.len())
            .field("pending", &self.pending)
            .field("status", &self.status)
            .field("kind", &self.kind)
            .finish()
    }
}

impl ConsoleTab {
    fn new_shell(id: u64, runtime: ConsoleRuntime) -> Self {
        Self {
            id,
            title: format!("Shell {}", id),
            input: String::new(),
            lines: Vec::new(),
            pending: String::new(),
            status: ConsoleStatus::Running,
            kind: ConsoleKind::Shell,
            runtime: Some(runtime),
        }
    }

    fn new_debug(id: u64, title: String) -> Self {
        Self {
            id,
            title,
            input: String::new(),
            lines: Vec::new(),
            pending: String::new(),
            status: ConsoleStatus::Running,
            kind: ConsoleKind::Debug,
            runtime: None,
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn input(&self) -> &str {
        &self.input
    }

    pub fn lines(&self) -> &[ConsoleLine] {
        &self.lines
    }

    pub fn status(&self) -> ConsoleStatus {
        self.status.clone()
    }

    pub fn kind(&self) -> ConsoleKind {
        self.kind
    }

    pub fn runtime(&self) -> Option<&ConsoleRuntime> {
        self.runtime.as_ref()
    }

    fn append_output(&mut self, text: &str) {
        self.append_stream(text, ConsoleLineKind::Output);
    }

    fn append_line(&mut self, kind: ConsoleLineKind, text: &str) {
        let mut text = text;
        if text.is_empty() {
            text = "";
        }
        self.append_stream(text, kind);
        if !self.pending.is_empty() {
            let pending = std::mem::take(&mut self.pending);
            self.push_line(kind, pending);
        }
    }

    fn append_stream(&mut self, text: &str, kind: ConsoleLineKind) {
        self.pending.push_str(text);
        while let Some(pos) = self.pending.find('\n') {
            let line = self.pending[..pos].to_string();
            self.pending = self.pending[pos + 1..].to_string();
            self.push_line(kind, line);
        }
    }

    fn push_line(&mut self, kind: ConsoleLineKind, text: String) {
        if text.is_empty() {
            self.lines.push(ConsoleLine { kind, text });
        } else {
            self.lines.push(ConsoleLine { kind, text });
        }
        if self.lines.len() > MAX_LINES {
            let overflow = self.lines.len() - MAX_LINES;
            self.lines.drain(0..overflow);
        }
    }

    fn handle_exit(&mut self, code: i32) {
        if !self.pending.is_empty() {
            let pending = std::mem::take(&mut self.pending);
            self.push_line(ConsoleLineKind::Output, pending);
        }
        self.status = ConsoleStatus::Exited(code);
        self.runtime = None;
    }

    fn set_input(&mut self, value: String) {
        self.input = value;
    }

    fn submit_input(&mut self) -> Result<(), String> {
        if self.kind != ConsoleKind::Shell {
            return Err("Console is read-only".to_string());
        }

        if !matches!(self.status, ConsoleStatus::Running) {
            return Err("Shell is not running".to_string());
        }

        let command = self.input.clone();
        self.input.clear();
        if let Some(runtime) = &self.runtime {
            runtime.send_line(&command)?;
        }
        self.append_line(ConsoleLineKind::Command, &command);
        Ok(())
    }
}

pub struct ConsoleRuntime {
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    events: Receiver<ConsoleRuntimeEvent>,
    child: Arc<Mutex<Box<dyn portable_pty::Child + Send + Sync>>>,
}

impl std::fmt::Debug for ConsoleRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConsoleRuntime")
            .field("child", &"pty-child")
            .finish()
    }
}

impl ConsoleRuntime {
    fn spawn_shell() -> Result<Self, String> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: DEFAULT_ROWS,
                cols: DEFAULT_COLS,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|err| err.to_string())?;

        let mut command = CommandBuilder::new("bash");
        command.arg("--noprofile");
        command.arg("--norc");
        command.env("TERM", "xterm-256color");

        let child = pair
            .slave
            .spawn_command(command)
            .map_err(|err| err.to_string())?;
        drop(pair.slave);

        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|err| err.to_string())?;

        let writer = pair
            .master
            .take_writer()
            .map_err(|err| err.to_string())?;

        let (event_sender, event_receiver) = unbounded();

        let child_arc: Arc<Mutex<Box<dyn portable_pty::Child + Send + Sync>>> =
            Arc::new(Mutex::new(child));
        let writer_arc: Arc<Mutex<Box<dyn Write + Send>>> = Arc::new(Mutex::new(writer));

        let reader_sender = event_sender.clone();
        thread::spawn(move || {
            let mut reader = reader;
            let mut buffer = [0u8; 4096];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(read) => {
                        let text = String::from_utf8_lossy(&buffer[..read]).to_string();
                        if reader_sender.send(ConsoleRuntimeEvent::Output(text)).is_err() {
                            break;
                        }
                    }
                    Err(err) => {
                        let _ = reader_sender.send(ConsoleRuntimeEvent::Error(err.to_string()));
                        break;
                    }
                }
            }
        });

        let wait_sender = event_sender.clone();
        let child_for_wait = child_arc.clone();
        thread::spawn(move || {
            if let Ok(mut child) = child_for_wait.lock() {
                match child.wait() {
                    Ok(status) => {
                        let code = status.exit_code() as i32;
                        let _ = wait_sender.send(ConsoleRuntimeEvent::Exit(code));
                    }
                    Err(err) => {
                        let _ = wait_sender.send(ConsoleRuntimeEvent::Error(err.to_string()));
                    }
                }
            }
        });

        Ok(Self {
            writer: writer_arc,
            events: event_receiver,
            child: child_arc,
        })
    }

    fn send_line(&self, value: &str) -> Result<(), String> {
        let mut writer =
            self.writer
                .lock()
                .map_err(|_| "terminal writer poisoned".to_string())?;
        writer
            .write_all(value.as_bytes())
            .map_err(|err| err.to_string())?;
        writer.write_all(b"\n").map_err(|err| err.to_string())?;
        writer.flush().map_err(|err| err.to_string())
    }

    fn try_recv(&self) -> Option<ConsoleRuntimeEvent> {
        self.events.try_recv().ok()
    }
}

impl Drop for ConsoleRuntime {
    fn drop(&mut self) {
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
        }
    }
}

#[derive(Debug, Clone)]
pub enum ConsoleRuntimeEvent {
    Output(String),
    Exit(i32),
    Error(String),
}
