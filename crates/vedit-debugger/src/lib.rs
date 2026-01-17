use crossbeam_channel::{Receiver, Sender, unbounded};
use iced_x86::{Decoder, DecoderOptions, Formatter, Instruction, NasmFormatter};
use nix::sys::ptrace;
use nix::sys::signal::{Signal, kill};
use nix::sys::wait::{WaitPidFlag, WaitStatus, waitpid};
use nix::unistd::{ForkResult, Pid, fork};
use std::collections::HashMap;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use thiserror::Error;

static SESSION_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

#[derive(Debug, Error)]
pub enum DebuggerError {
    #[error("Failed to spawn process: {0}")]
    Spawn(#[from] std::io::Error),
    #[error("Ptrace error: {0}")]
    Ptrace(#[from] nix::errno::Errno),
    #[error("Process not found")]
    ProcessNotFound,
    #[error("Debugger process exited unexpectedly")]
    ProcessExited,
}

#[derive(Debug, Clone)]
pub struct Breakpoint {
    pub address: u64,
    pub original_byte: u8,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct LaunchConfig {
    pub executable: PathBuf,
    pub working_directory: PathBuf,
    pub arguments: Vec<String>,
    pub breakpoints: Vec<u64>, // addresses for now
}

#[derive(Debug, Clone)]
pub enum DebuggerCommand {
    Continue,
    Step,
    Kill,
    ReadMemory(u64, usize),  // address, size
    Disassemble(u64, usize), // address, instruction count
    AddBreakpoint(u64),      // address
    RemoveBreakpoint(u64),   // address
    ListBreakpoints,
}

#[derive(Debug, Clone)]
pub enum DebuggerEvent {
    Started,
    Stopped { reason: StopReason },
    Exited(i32),
    Error(String),
    MemoryRead(Vec<u8>),
    Disassembly(Vec<String>),
    BreakpointAdded { address: u64, success: bool },
    BreakpointRemoved { address: u64, success: bool },
    BreakpointList(Vec<Breakpoint>),
}

#[derive(Debug, Clone)]
pub enum StopReason {
    Breakpoint,
    Step,
    Signal(Signal),
}

#[derive(Clone, Debug)]
pub struct VeditSession {
    id: u64,
    command_sender: Sender<DebuggerCommand>,
    event_receiver: Receiver<DebuggerEvent>,
}

impl VeditSession {
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

pub fn spawn_session(config: LaunchConfig) -> Result<VeditSession, DebuggerError> {
    let (command_sender, command_receiver) = unbounded();
    let (event_sender, event_receiver) = unbounded();

    let child_pid = unsafe {
        match fork()? {
            ForkResult::Parent { child } => child,
            ForkResult::Child => {
                // In child process
                ptrace::traceme().map_err(|e| {
                    eprintln!("traceme failed: {:?}", e);
                    e
                })?;

                // Set up the command
                let mut cmd = Command::new(&config.executable);
                cmd.args(&config.arguments)
                    .current_dir(&config.working_directory)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null());

                // Use exec to replace the process
                let err = cmd.exec();
                eprintln!("exec failed: {:?}", err);
                std::process::exit(1);
            }
        }
    };

    // Wait for the child to stop after traceme
    match waitpid(child_pid, Some(WaitPidFlag::WSTOPPED))? {
        WaitStatus::Stopped(_, Signal::SIGTRAP) => {
            // Good, child is stopped
        }
        _status => {
            return Err(DebuggerError::ProcessExited);
        }
    }

    let breakpoints = Arc::new(Mutex::new(HashMap::new()));

    // Set up breakpoints
    for addr in &config.breakpoints {
        if let Ok(original) = set_breakpoint(child_pid, *addr) {
            breakpoints.lock().unwrap().insert(
                *addr,
                Breakpoint {
                    address: *addr,
                    original_byte: original,
                    enabled: true,
                },
            );
        }
    }

    let event_sender_clone = event_sender.clone();
    thread::spawn(move || {
        let _ = event_sender_clone.send(DebuggerEvent::Started);
    });

    let command_event_sender = event_sender.clone();
    let breakpoints_for_commands = breakpoints.clone();
    thread::spawn(move || {
        while let Ok(command) = command_receiver.recv() {
            match command {
                DebuggerCommand::Continue => {
                    if let Err(err) = ptrace::cont(child_pid, None) {
                        let _ = command_event_sender.send(DebuggerEvent::Error(err.to_string()));
                        break;
                    }
                }
                DebuggerCommand::Step => {
                    if let Err(err) = ptrace::step(child_pid, None) {
                        let _ = command_event_sender.send(DebuggerEvent::Error(err.to_string()));
                        break;
                    }
                }
                DebuggerCommand::Kill => {
                    let _ = kill(child_pid, Signal::SIGKILL);
                    break;
                }
                DebuggerCommand::ReadMemory(addr, size) => {
                    match read_memory(child_pid, addr, size) {
                        Ok(data) => {
                            let _ = command_event_sender.send(DebuggerEvent::MemoryRead(data));
                        }
                        Err(err) => {
                            let _ =
                                command_event_sender.send(DebuggerEvent::Error(err.to_string()));
                        }
                    }
                }
                DebuggerCommand::Disassemble(addr, count) => {
                    match disassemble_memory(child_pid, addr, count) {
                        Ok(instructions) => {
                            let _ =
                                command_event_sender.send(DebuggerEvent::Disassembly(instructions));
                        }
                        Err(err) => {
                            let _ =
                                command_event_sender.send(DebuggerEvent::Error(err.to_string()));
                        }
                    }
                }
                DebuggerCommand::AddBreakpoint(addr) => {
                    let mut bps = breakpoints_for_commands.lock().unwrap();
                    if bps.contains_key(&addr) {
                        // Breakpoint already exists at this address
                        let _ = command_event_sender.send(DebuggerEvent::BreakpointAdded {
                            address: addr,
                            success: true,
                        });
                    } else {
                        match set_breakpoint(child_pid, addr) {
                            Ok(original_byte) => {
                                bps.insert(
                                    addr,
                                    Breakpoint {
                                        address: addr,
                                        original_byte,
                                        enabled: true,
                                    },
                                );
                                let _ = command_event_sender.send(DebuggerEvent::BreakpointAdded {
                                    address: addr,
                                    success: true,
                                });
                            }
                            Err(err) => {
                                let _ = command_event_sender.send(DebuggerEvent::Error(format!(
                                    "Failed to set breakpoint at 0x{:x}: {}",
                                    addr, err
                                )));
                                let _ = command_event_sender.send(DebuggerEvent::BreakpointAdded {
                                    address: addr,
                                    success: false,
                                });
                            }
                        }
                    }
                }
                DebuggerCommand::RemoveBreakpoint(addr) => {
                    let mut bps = breakpoints_for_commands.lock().unwrap();
                    if let Some(bp) = bps.remove(&addr) {
                        match restore_breakpoint(child_pid, &bp) {
                            Ok(()) => {
                                let _ =
                                    command_event_sender.send(DebuggerEvent::BreakpointRemoved {
                                        address: addr,
                                        success: true,
                                    });
                            }
                            Err(err) => {
                                // Put it back since we failed to restore
                                bps.insert(addr, bp);
                                let _ = command_event_sender.send(DebuggerEvent::Error(format!(
                                    "Failed to remove breakpoint at 0x{:x}: {}",
                                    addr, err
                                )));
                                let _ =
                                    command_event_sender.send(DebuggerEvent::BreakpointRemoved {
                                        address: addr,
                                        success: false,
                                    });
                            }
                        }
                    } else {
                        // No breakpoint at this address
                        let _ = command_event_sender.send(DebuggerEvent::BreakpointRemoved {
                            address: addr,
                            success: false,
                        });
                    }
                }
                DebuggerCommand::ListBreakpoints => {
                    let bps = breakpoints_for_commands.lock().unwrap();
                    let list: Vec<Breakpoint> = bps.values().cloned().collect();
                    let _ = command_event_sender.send(DebuggerEvent::BreakpointList(list));
                }
            }
        }
    });

    let wait_sender = event_sender.clone();
    let breakpoints_for_wait = breakpoints.clone();
    thread::spawn(move || {
        loop {
            match waitpid(child_pid, None) {
                Ok(WaitStatus::Exited(_, code)) => {
                    let _ = wait_sender.send(DebuggerEvent::Exited(code));
                    break;
                }
                Ok(WaitStatus::Stopped(_, signal)) => {
                    let reason = match signal {
                        Signal::SIGTRAP => {
                            // Check if we hit a breakpoint
                            if let Ok(pc) = get_program_counter(child_pid) {
                                if let Some(bp) =
                                    breakpoints_for_wait.lock().unwrap().get(&(pc - 1))
                                {
                                    // Restore original byte and step back
                                    if let Err(_) = restore_breakpoint(child_pid, bp) {
                                        let _ = wait_sender.send(DebuggerEvent::Error(
                                            "Failed to restore breakpoint".to_string(),
                                        ));
                                        break;
                                    }
                                    // Step to execute the original instruction
                                    if let Err(_) = ptrace::step(child_pid, None) {
                                        let _ = wait_sender.send(DebuggerEvent::Error(
                                            "Failed to step".to_string(),
                                        ));
                                        break;
                                    }
                                    // Re-set the breakpoint
                                    if let Err(_) = set_breakpoint(child_pid, bp.address) {
                                        let _ = wait_sender.send(DebuggerEvent::Error(
                                            "Failed to re-set breakpoint".to_string(),
                                        ));
                                        break;
                                    }
                                    StopReason::Breakpoint
                                } else {
                                    StopReason::Step
                                }
                            } else {
                                StopReason::Signal(signal)
                            }
                        }
                        _ => StopReason::Signal(signal),
                    };
                    let _ = wait_sender.send(DebuggerEvent::Stopped { reason });
                }
                Ok(WaitStatus::Signaled(_, signal, _)) => {
                    let _ = wait_sender.send(DebuggerEvent::Exited(signal as i32));
                    break;
                }
                Err(err) => {
                    let _ = wait_sender.send(DebuggerEvent::Error(err.to_string()));
                    break;
                }
                _ => continue,
            }
        }
    });

    Ok(VeditSession {
        id: SESSION_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        command_sender,
        event_receiver,
    })
}

fn set_breakpoint(pid: Pid, addr: u64) -> Result<u8, nix::errno::Errno> {
    let original_word: i64 = ptrace::read(pid, addr as *mut _)?;
    let original_byte = (original_word & 0xFF) as u8;
    let modified_word = (original_word & !0xFF) | 0xCC;
    ptrace::write(pid, addr as *mut _, modified_word)?;
    Ok(original_byte)
}

fn restore_breakpoint(pid: Pid, bp: &Breakpoint) -> Result<(), nix::errno::Errno> {
    let current_word: i64 = ptrace::read(pid, bp.address as *mut _)?;
    let restored_word = (current_word & !0xFF) | (bp.original_byte as i64);
    ptrace::write(pid, bp.address as *mut _, restored_word)?;
    Ok(())
}

fn get_program_counter(pid: Pid) -> Result<u64, nix::errno::Errno> {
    #[cfg(target_arch = "x86_64")]
    {
        let regs = ptrace::getregs(pid)?;
        Ok(regs.rip)
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        Err(nix::errno::Errno::ENOTSUP)
    }
}

fn read_memory(pid: Pid, addr: u64, size: usize) -> Result<Vec<u8>, nix::errno::Errno> {
    let mut data = Vec::with_capacity(size);
    for i in 0..size {
        let word: i64 = ptrace::read(pid, (addr + i as u64) as *mut _)?;
        data.push(word as u8);
    }
    Ok(data)
}

fn disassemble_memory(pid: Pid, addr: u64, count: usize) -> Result<Vec<String>, nix::errno::Errno> {
    // Read some memory around the address
    let memory_size = 1024; // Read 1KB for disassembly
    let memory = read_memory(pid, addr, memory_size)?;

    // Create decoder
    let mut decoder = Decoder::new(64, &memory, DecoderOptions::NONE);
    decoder.set_ip(addr);

    // Create formatter
    let mut formatter = NasmFormatter::new();
    formatter.options_mut().set_digit_separator("_");
    formatter.options_mut().set_first_operand_char_index(10);

    let mut instructions = Vec::new();
    let mut instruction = Instruction::default();

    for _ in 0..count {
        if !decoder.can_decode() {
            break;
        }

        decoder.decode_out(&mut instruction);

        let mut output = String::new();
        formatter.format(&instruction, &mut output);
        instructions.push(format!("{:016X} {}", instruction.ip(), output));
    }

    Ok(instructions)
}
