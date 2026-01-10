//! Remote desktop integration for Wine applications

use crate::error::{WineError, WineResult};
use serde::{Deserialize, Serialize};
use std::process::Stdio;
use tokio::process::{Child, Command};
use uuid::Uuid;

/// Remote desktop manager for Wine applications
pub struct RemoteDesktop {
    /// Active remote desktop sessions
    sessions: std::collections::HashMap<Uuid, RemoteDesktopSession>,

    /// Configuration
    config: RemoteDesktopConfig,
}

/// Remote desktop configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteDesktopConfig {
    /// Default desktop type
    pub default_type: DesktopType,

    /// Port range to use
    pub port_range: (u16, u16),

    /// Default resolution
    pub default_resolution: (u32, u32),

    /// Security settings
    pub security: SecuritySettings,

    /// Performance settings
    pub performance: PerformanceSettings,
}

impl Default for RemoteDesktopConfig {
    fn default() -> Self {
        Self {
            default_type: DesktopType::Vnc,
            port_range: (5900, 5999),
            default_resolution: (1920, 1080),
            security: SecuritySettings::default(),
            performance: PerformanceSettings::default(),
        }
    }
}

/// Security settings for remote desktop
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecuritySettings {
    /// Generate random passwords
    pub generate_random_passwords: bool,

    /// Password length
    pub password_length: usize,

    /// Allow connections from localhost only
    pub localhost_only: bool,

    /// Connection timeout in seconds
    pub connection_timeout: u64,
}

impl Default for SecuritySettings {
    fn default() -> Self {
        Self {
            generate_random_passwords: true,
            password_length: 12,
            localhost_only: true,
            connection_timeout: 300,
        }
    }
}

/// Performance settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceSettings {
    /// Enable compression
    pub compression: bool,

    /// JPEG quality for image compression (1-100)
    pub jpeg_quality: u8,

    /// Enable cursor shadow
    pub cursor_shadow: bool,

    /// Enable desktop effects
    pub desktop_effects: bool,
}

impl Default for PerformanceSettings {
    fn default() -> Self {
        Self {
            compression: true,
            jpeg_quality: 80,
            cursor_shadow: false,
            desktop_effects: false,
        }
    }
}

/// Remote desktop types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DesktopType {
    /// VNC server
    Vnc,
    /// RDP server
    Rdp,
    /// X11 forwarding
    X11,
    /// Wayland remote desktop (experimental)
    Wayland,
}

/// Remote desktop session
pub struct RemoteDesktopSession {
    /// Unique session identifier
    pub id: Uuid,

    /// Session type
    pub session_type: DesktopType,

    /// Port number
    pub port: u16,

    /// Resolution
    pub resolution: (u32, u32),

    /// Password (if any)
    pub password: Option<String>,

    /// Process handle for the desktop server
    server_process: Option<Child>,

    /// Session start time
    pub start_time: std::time::Instant,

    /// Connection URL for clients
    pub connection_url: String,

    /// Wine process this session is attached to
    pub wine_process_id: Option<Uuid>,
}

impl RemoteDesktop {
    /// Create new remote desktop manager
    pub fn new(config: RemoteDesktopConfig) -> Self {
        Self {
            sessions: std::collections::HashMap::new(),
            config,
        }
    }

    /// Create a new remote desktop session
    pub async fn create_session(
        &mut self,
        session_type: DesktopType,
        wine_process_id: Option<Uuid>,
        resolution: Option<(u32, u32)>,
    ) -> WineResult<Uuid> {
        let session_id = Uuid::new_v4();
        let resolution = resolution.unwrap_or(self.config.default_resolution);
        let port = self.find_available_port()?;

        let password = if self.config.security.generate_random_passwords {
            Some(self.generate_password())
        } else {
            None
        };

        let session = match session_type {
            DesktopType::Vnc => {
                self.create_vnc_session(session_id, port, resolution, password.clone())
                    .await?
            }
            DesktopType::Rdp => {
                self.create_rdp_session(session_id, port, resolution, password.clone())
                    .await?
            }
            DesktopType::X11 => {
                self.create_x11_session(session_id, port, resolution, password.clone())
                    .await?
            }
            DesktopType::Wayland => {
                self.create_wayland_session(session_id, port, resolution, password.clone())
                    .await?
            }
        };

        let connection_url = self.build_connection_url(&session_type, port, &password);

        let remote_session = RemoteDesktopSession {
            id: session_id,
            session_type,
            port,
            resolution,
            password,
            server_process: Some(session),
            start_time: std::time::Instant::now(),
            connection_url,
            wine_process_id,
        };

        self.sessions.insert(session_id, remote_session);
        Ok(session_id)
    }

    /// Create VNC session
    async fn create_vnc_session(
        &self,
        _session_id: Uuid,
        port: u16,
        resolution: (u32, u32),
        password: Option<String>,
    ) -> WineResult<Child> {
        tracing::info!(
            "Creating VNC session on port {} with resolution {:?}",
            port,
            resolution
        );

        // Use Xvfb to create virtual display
        let display_num = port - 5900;
        let display_str = format!(":{}", display_num);

        // Start Xvfb
        // TODO(Vince): The Xvfb process is spawned but not tracked. It needs to be stored
        // and killed when the VNC session ends, otherwise Xvfb processes will leak.
        let _xvfb_process = Command::new("Xvfb")
            .arg(&display_str)
            .arg("-screen")
            .arg("0")
            .arg(&format!("{}x{}x24", resolution.0, resolution.1))
            .arg("-ac")
            .spawn()?;

        // Give Xvfb a moment to start
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Start x11vnc server
        let mut vnc_cmd = Command::new("x11vnc");
        vnc_cmd
            .arg("-display")
            .arg(&display_str)
            .arg("-forever")
            .arg("-nopw")
            .arg("-quiet")
            .arg("-rfbport")
            .arg(port.to_string());

        if let Some(pwd) = password {
            vnc_cmd.arg("-passwd").arg(pwd);
        }

        if self.config.security.localhost_only {
            vnc_cmd.arg("-localhost");
        }

        if self.config.performance.compression {
            vnc_cmd.arg("-compress").arg("level").arg("6");
            vnc_cmd
                .arg("-quality")
                .arg(self.config.performance.jpeg_quality.to_string());
        }

        let vnc_process = vnc_cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        // Give VNC server a moment to start
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

        Ok(vnc_process)
    }

    /// Create RDP session
    async fn create_rdp_session(
        &self,
        _session_id: Uuid,
        port: u16,
        resolution: (u32, u32),
        password: Option<String>,
    ) -> WineResult<Child> {
        tracing::info!(
            "Creating RDP session on port {} with resolution {:?}",
            port,
            resolution
        );

        // Use xrdp or XRDP for RDP server
        let mut rdp_cmd = Command::new("xrdp");
        rdp_cmd
            .arg("-n")
            .arg("-p")
            .arg(port.to_string())
            .arg("-t")
            .arg(&format!("{}x{}", resolution.0, resolution.1));

        if let Some(pwd) = password {
            rdp_cmd.arg("-P").arg(pwd);
        }

        if self.config.security.localhost_only {
            rdp_cmd.arg("-127.0.0.1");
        }

        rdp_cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                WineError::RemoteDesktopError(format!("Failed to start RDP server: {}", e))
            })
    }

    /// Create X11 forwarding session
    async fn create_x11_session(
        &self,
        _session_id: Uuid,
        port: u16,
        resolution: (u32, u32),
        _password: Option<String>, // X11 forwarding doesn't use password auth
    ) -> WineResult<Child> {
        tracing::info!("Creating X11 forwarding session on port {}", port);

        // Create a new X server with Xephyr
        let mut xephyr_cmd = Command::new("Xephyr");
        xephyr_cmd
            .arg(&format!(":{}", port))
            .arg("-screen")
            .arg(&format!("{}x{}", resolution.0, resolution.1))
            .arg("-resizeable")
            .arg("-ac");

        if self.config.security.localhost_only {
            xephyr_cmd.arg("-nolisten").arg("tcp");
        }

        xephyr_cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| WineError::RemoteDesktopError(format!("Failed to start Xephyr: {}", e)))
    }

    /// Create Wayland remote session
    async fn create_wayland_session(
        &self,
        _session_id: Uuid,
        port: u16,
        _resolution: (u32, u32), // TODO(Vince): wayvnc doesn't accept resolution arg; need compositor config
        password: Option<String>,
    ) -> WineResult<Child> {
        tracing::warn!("Wayland remote desktop is experimental");

        // Use wayvnc for Wayland VNC
        let mut wayvnc_cmd = Command::new("wayvnc");
        wayvnc_cmd
            .arg("127.0.0.1")
            .arg(port.to_string())
            .arg("--disable-auth");

        if let Some(pwd) = password {
            wayvnc_cmd.arg("--password").arg(pwd);
        }

        wayvnc_cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| WineError::RemoteDesktopError(format!("Failed to start WayVNC: {}", e)))
    }

    /// Find an available port in the configured range
    fn find_available_port(&self) -> WineResult<u16> {
        use std::net::TcpListener;

        for port in self.config.port_range.0..=self.config.port_range.1 {
            if let Ok(_) = TcpListener::bind(("127.0.0.1", port)) {
                return Ok(port);
            }
        }

        Err(WineError::RemoteDesktopError(
            "No available ports found".to_string(),
        ))
    }

    /// Generate a random password
    fn generate_password(&self) -> String {
        use rand::Rng;
        const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

        let mut rng = rand::rng();
        (0..self.config.security.password_length)
            .map(|_| {
                let idx = rng.random_range(0..CHARSET.len());
                CHARSET[idx] as char
            })
            .collect()
    }

    /// Build connection URL for clients
    fn build_connection_url(
        &self,
        session_type: &DesktopType,
        port: u16,
        password: &Option<String>,
    ) -> String {
        match session_type {
            DesktopType::Vnc => {
                let mut url = format!("vnc://127.0.0.1:{}", port);
                if let Some(pwd) = password {
                    url = format!("vnc://:{}@127.0.0.1:{}", pwd, port);
                }
                url
            }
            DesktopType::Rdp => {
                format!("rdp://127.0.0.1:{}", port)
            }
            DesktopType::X11 => {
                format!("x11://127.0.0.1:{}", port)
            }
            DesktopType::Wayland => {
                format!("wayvnc://127.0.0.1:{}", port)
            }
        }
    }

    /// Get session by ID
    pub fn get_session(&self, session_id: &Uuid) -> Option<&RemoteDesktopSession> {
        self.sessions.get(session_id)
    }

    /// Get all active sessions
    pub fn sessions(&self) -> &std::collections::HashMap<Uuid, RemoteDesktopSession> {
        &self.sessions
    }

    /// Close a remote desktop session
    pub async fn close_session(&mut self, session_id: &Uuid) -> WineResult<()> {
        if let Some(session) = self.sessions.remove(session_id) {
            if let Some(mut process) = session.server_process {
                process.kill().await.map_err(|e| {
                    WineError::RemoteDesktopError(format!("Failed to kill server process: {}", e))
                })?;
            }
            tracing::info!("Closed remote desktop session: {}", session_id);
        }
        Ok(())
    }

    /// Close all sessions
    pub async fn close_all_sessions(&mut self) -> WineResult<()> {
        let session_ids: Vec<Uuid> = self.sessions.keys().copied().collect();
        for session_id in session_ids {
            self.close_session(&session_id).await?;
        }
        Ok(())
    }

    /// Check if a session is still running
    pub async fn is_session_running(&mut self, session_id: &Uuid) -> bool {
        if let Some(session) = self.sessions.get_mut(session_id) {
            if let Some(process) = &mut session.server_process {
                match process.try_wait() {
                    Ok(Some(_)) => {
                        // Process has exited
                        return false;
                    }
                    Ok(None) => {
                        // Process is still running
                        return true;
                    }
                    Err(_) => {
                        // Error checking status
                        return false;
                    }
                }
            }
        }
        false
    }

    /// Get connection info for a session
    pub fn get_connection_info(&self, session_id: &Uuid) -> Option<ConnectionInfo> {
        self.sessions.get(session_id).map(|session| ConnectionInfo {
            session_type: session.session_type.clone(),
            connection_url: session.connection_url.clone(),
            port: session.port,
            resolution: session.resolution,
            password: session.password.clone(),
            uptime: session.start_time.elapsed(),
        })
    }
}

/// Connection information for remote desktop clients
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    pub session_type: DesktopType,
    pub connection_url: String,
    pub port: u16,
    pub resolution: (u32, u32),
    pub password: Option<String>,
    pub uptime: std::time::Duration,
}
