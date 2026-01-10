//! NixOS-specific Wine integration

use crate::environment::{WineEnvironment, WineEnvironmentConfig};
use crate::error::{WineError, WineResult};
use crate::process::{WineProcess, WineProcessConfig};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;
use uuid::Uuid;

/// NixOS Wine manager for declarative Wine environments
pub struct NixWineManager {
    /// Base directory for Nix expressions
    nix_expr_dir: PathBuf,

    /// GC root directory for persistent environments
    gc_root_dir: PathBuf,

    /// Nix profile for Wine packages
    wine_profile: Option<PathBuf>,

    /// Whether running on NixOS
    is_nixos: bool,
}

impl NixWineManager {
    /// Detect if running on NixOS and create manager
    pub fn detect() -> WineResult<Self> {
        let is_nixos = crate::WineManager::is_nixos();

        if !is_nixos {
            return Err(WineError::NixError("Not running on NixOS".to_string()));
        }

        let home = dirs::home_dir()
            .ok_or_else(|| WineError::NixError("Could not determine home directory".to_string()))?;

        let nix_expr_dir = home.join(".vedit").join("nix");
        let gc_root_dir = home.join(".vedit").join("nix-gcroots");

        std::fs::create_dir_all(&nix_expr_dir)?;
        std::fs::create_dir_all(&gc_root_dir)?;

        let wine_profile = home.join(".nix-profile").join("bin");

        Ok(Self {
            nix_expr_dir,
            gc_root_dir,
            wine_profile: Some(wine_profile),
            is_nixos,
        })
    }

    /// Create a Nix-managed Wine environment
    pub async fn create_wine_environment(
        &self,
        project_path: &Path,
        env_id: &str,
        config: WineEnvironmentConfig,
    ) -> WineResult<WineEnvironment> {
        let nix_expr = self.generate_nix_expression(env_id, &config)?;
        let nix_file = self.nix_expr_dir.join(format!("{}.nix", env_id));
        let gc_root = self.gc_root_dir.join(env_id);

        // Write Nix expression
        tokio::fs::write(&nix_file, nix_expr).await?;

        // Build the environment and create GC root
        self.build_environment(&nix_file, &gc_root).await?;

        // Create FHS user environment wrapper
        let fhs_wrapper = self
            .create_fhs_wrapper(env_id, &gc_root, project_path)
            .await?;

        // Initialize the Wine prefix using the FHS environment
        self.initialize_nix_prefix(&fhs_wrapper, project_path, env_id, &config)
            .await?;

        // Create WineEnvironment instance
        let prefix_path = project_path.join(".wine").join(env_id);
        let mut env_vars = std::collections::HashMap::new();
        env_vars.insert(
            "WINEPREFIX".to_string(),
            prefix_path.to_string_lossy().to_string(),
        );
        env_vars.insert(
            "NIX_WINE_WRAPPER".to_string(),
            fhs_wrapper.to_string_lossy().to_string(),
        );

        // Configure DLL overrides
        let dll_overrides = Self::build_dll_overrides(&config.dll_overrides);
        env_vars.insert("WINEDLLOVERRIDES".to_string(), dll_overrides);

        Ok(WineEnvironment {
            id: env_id.to_string(),
            prefix_path,
            project_path: project_path.to_path_buf(),
            config,
            env_vars,
            active_processes: std::collections::HashMap::new(),
        })
    }

    /// Generate Nix expression for Wine environment
    fn generate_nix_expression(
        &self,
        env_id: &str,
        config: &WineEnvironmentConfig,
    ) -> WineResult<String> {
        let wine_packages = self.get_wine_packages(config)?;
        let runtimes = self.get_runtime_packages(&config.runtimes)?;

        let nix_expr = format!(
            r#"
{{ pkgs ? import <nixpkgs> {{}} }}:

pkgs.buildFHSUserEnv {{
  name = "vedit-wine-{env_id}";
  targetPkgs = pkgs: with pkgs; [
    # Wine and core packages
    {wine_packages}

    # Runtimes and dependencies
    {runtimes}

    # Additional utilities
    which
    coreutils
    findutils
    gnugrep
    bash
  ];

  multiPkgs = pkgs: with pkgs; [
    # 32-bit libraries for 32-bit applications
    (pkgsi686Linux.glibc or null)
    (pkgsi686Linux.zlib or null)
    (pkgsi686Linux.freetype or null)
  ];

  profile = ""
    export WINEARCH="{}"
    export WINEDLLOVERRIDES="{}"
    export WINEDEBUG="-all"
  "";

  runScript = "bash";
}}
"#,
            match config.architecture {
                crate::environment::WineArchitecture::Win32 => "win32",
                crate::environment::WineArchitecture::Win64 => "win64",
            },
            Self::build_dll_overrides(&config.dll_overrides),
            env_id = env_id
        );

        Ok(nix_expr)
    }

    /// Get Wine packages based on configuration
    fn get_wine_packages(&self, config: &WineEnvironmentConfig) -> WineResult<String> {
        let mut packages = vec![
            "wine".to_string(),
            "wineWowPackages".to_string(), // For both 32-bit and 64-bit support
            "winetricks".to_string(),
        ];

        // Add specific wine version if requested
        if let Some(version) = &config.wine_version {
            packages.push(format!("wine_{}", version));
        }

        Ok(packages.join("\n    "))
    }

    /// Get runtime packages for Nix
    fn get_runtime_packages(&self, runtimes: &[crate::environment::Runtime]) -> WineResult<String> {
        let mut packages = Vec::new();

        for runtime in runtimes {
            let nix_package = match runtime {
                crate::environment::Runtime::DotNet20 => "dotnet-runtime_6",
                crate::environment::Runtime::DotNet35 => "dotnet-runtime_6",
                crate::environment::Runtime::DotNet40 => "dotnet-runtime_6",
                crate::environment::Runtime::DotNet45 => "dotnet-runtime_6",
                crate::environment::Runtime::DotNet48 => "dotnet48",
                crate::environment::Runtime::DotNet60 => "dotnet-runtime_6",
                crate::environment::Runtime::DotNet70 => "dotnet-runtime_7",
                crate::environment::Runtime::DotNet80 => "dotnet-runtime_8",
                crate::environment::Runtime::Vc2015_2022 => "mingw-w64",
                crate::environment::Runtime::DirectX9 => "dxvk",
                crate::environment::Runtime::DirectX11 => "dxvk",
                _ => continue,
            };
            packages.push(nix_package.to_string());
        }

        Ok(packages.join("\n    "))
    }

    /// Build the Nix environment and create GC root
    async fn build_environment(&self, nix_file: &Path, gc_root: &Path) -> WineResult<()> {
        let output = Command::new("nix-build")
            .arg(nix_file)
            .arg("--out-link")
            .arg(gc_root)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(WineError::NixError(format!(
                "Failed to build Nix environment: {}",
                stderr
            )));
        }

        tracing::info!(
            "Successfully built Nix environment at: {}",
            gc_root.display()
        );
        Ok(())
    }

    /// Create FHS wrapper script
    async fn create_fhs_wrapper(
        &self,
        env_id: &str,
        gc_root: &Path,
        project_path: &Path,
    ) -> WineResult<PathBuf> {
        let wrapper_path = project_path
            .join(".wine")
            .join(format!("{}-wrapper.sh", env_id));

        let wrapper_script = format!(
            r#"#!/usr/bin/env bash
# FHS wrapper for Wine environment {}

# Set up the FHS environment
export PATH="{}/bin:$PATH"
export LD_LIBRARY_PATH="{}/lib:$LD_LIBRARY_PATH"

# Wine-specific environment
export WINEPREFIX="{}/.wine/{}"
export WINEARCH="win64"
export WINEDLLOVERRIDES="mscoree=;mshtml="

# Execute the command
exec "$@"
"#,
            env_id,
            gc_root.display(),
            gc_root.display(),
            project_path.display(),
            env_id
        );

        tokio::fs::write(&wrapper_path, wrapper_script).await?;

        // Make executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = tokio::fs::metadata(&wrapper_path).await?.permissions();
            perms.set_mode(0o755);
            tokio::fs::set_permissions(&wrapper_path, perms).await?;
        }

        Ok(wrapper_path)
    }

    /// Initialize Wine prefix using Nix environment
    async fn initialize_nix_prefix(
        &self,
        fhs_wrapper: &Path,
        project_path: &Path,
        env_id: &str,
        config: &WineEnvironmentConfig,
    ) -> WineResult<()> {
        let prefix_path = project_path.join(".wine").join(env_id);
        std::fs::create_dir_all(&prefix_path)?;

        // Run wineboot through the FHS wrapper
        let output = Command::new(fhs_wrapper)
            .args(&["wineboot", "--init"])
            .env("WINEPREFIX", &prefix_path)
            .env(
                "WINEARCH",
                match config.architecture {
                    crate::environment::WineArchitecture::Win32 => "win32",
                    crate::environment::WineArchitecture::Win64 => "win64",
                },
            )
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(WineError::NixError(format!(
                "Failed to initialize Wine prefix: {}",
                stderr
            )));
        }

        // Apply configuration using winetricks through FHS
        for runtime in &config.runtimes {
            self.install_runtime_nix(fhs_wrapper, &prefix_path, runtime)
                .await?;
        }

        tracing::info!(
            "Successfully initialized Nix Wine prefix: {}",
            prefix_path.display()
        );
        Ok(())
    }

    /// Install runtime using winetricks in Nix environment
    async fn install_runtime_nix(
        &self,
        fhs_wrapper: &Path,
        prefix_path: &Path,
        runtime: &crate::environment::Runtime,
    ) -> WineResult<()> {
        let runtime_name = match runtime {
            crate::environment::Runtime::DotNet48 => "dotnet48",
            crate::environment::Runtime::Vc2015_2022 => "vc2015_2022",
            crate::environment::Runtime::DirectX9 => "d3dx9",
            crate::environment::Runtime::DirectX11 => "d3dcompiler_47",
            _ => return Ok(()),
        };

        tracing::info!("Installing runtime via Nix: {}", runtime_name);

        let output = Command::new(fhs_wrapper)
            .args(&["winetricks", "-q", runtime_name])
            .env("WINEPREFIX", prefix_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::warn!("Failed to install runtime {}: {}", runtime_name, stderr);
        } else {
            tracing::info!("Successfully installed runtime: {}", runtime_name);
        }

        Ok(())
    }

    /// Build DLL overrides string
    fn build_dll_overrides(
        overrides: &std::collections::HashMap<String, crate::environment::DllOverride>,
    ) -> String {
        overrides
            .iter()
            .map(|(dll, override_type)| {
                let override_str = match override_type {
                    crate::environment::DllOverride::Native => "n",
                    crate::environment::DllOverride::Builtin => "b",
                    crate::environment::DllOverride::Disable => "",
                    crate::environment::DllOverride::NativeBuiltin => "n,b",
                    crate::environment::DllOverride::BuiltinNative => "b,n",
                };
                format!("{}={}", dll, override_str)
            })
            .collect::<Vec<_>>()
            .join(";")
    }

    /// Spawn a process using Nix-managed Wine environment
    pub async fn spawn_process(
        &self,
        environment: &WineEnvironment,
        exe_path: &Path,
        args: &[String],
        config: WineProcessConfig,
    ) -> WineResult<WineProcess> {
        let fhs_wrapper = environment
            .env_vars
            .get("NIX_WINE_WRAPPER")
            .ok_or_else(|| WineError::NixError("NIX_WINE_WRAPPER not set".to_string()))?;

        let process_id = Uuid::new_v4();
        let mut cmd = Command::new(fhs_wrapper);

        // Set up environment
        for (key, value) in &environment.env_vars {
            cmd.env(key, value);
        }

        // Add process-specific environment variables
        for (key, value) in &config.env_vars {
            cmd.env(key, value);
        }

        // Set working directory
        if let Some(working_dir) = &config.working_directory {
            cmd.current_dir(working_dir);
        } else {
            cmd.current_dir(exe_path.parent().unwrap_or_else(|| Path::new(".")));
        }

        // Build wine command
        let mut wine_args = vec!["wine".to_string()];
        wine_args.push(exe_path.to_string_lossy().to_string());
        wine_args.extend_from_slice(args);

        cmd.args(wine_args);

        // Configure output capture
        if config.capture_output {
            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::piped());
        }

        tracing::info!(
            "Spawning Nix Wine process: {:?} with args: {:?}",
            exe_path,
            args
        );

        let child = cmd.spawn().map_err(|e| {
            WineError::ProcessSpawnFailed(format!("Failed to spawn NIX wine process: {}", e))
        })?;

        Ok(WineProcess {
            id: process_id,
            exe_path: exe_path.to_path_buf(),
            args: args.to_vec(),
            status: crate::process::ProcessStatus::Starting,
            start_time: std::time::Instant::now(),
            environment_id: environment.id.clone(),
            config,
            child: Some(child),
        })
    }

    /// Clean up Nix environments that are no longer needed
    pub async fn cleanup_environments(&self) -> WineResult<()> {
        // Remove unused GC roots
        let output = Command::new("nix-collect-garbage")
            .arg("-d")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::warn!("Failed to run nix-collect-garbage: {}", stderr);
        }

        Ok(())
    }

    /// Update Wine packages in Nix environment
    pub async fn update_packages(&self, env_id: &str) -> WineResult<()> {
        let nix_file = self.nix_expr_dir.join(format!("{}.nix", env_id));
        let gc_root = self.gc_root_dir.join(env_id);

        // Rebuild the environment
        self.build_environment(&nix_file, &gc_root).await?;

        tracing::info!("Successfully updated Nix Wine environment: {}", env_id);
        Ok(())
    }

    /// Get information about Nix integration
    pub fn info(&self) -> NixInfo {
        NixInfo {
            is_nixos: self.is_nixos,
            nix_expr_dir: self.nix_expr_dir.clone(),
            gc_root_dir: self.gc_root_dir.clone(),
            wine_profile: self.wine_profile.clone(),
        }
    }
}

/// Information about Nix integration
#[derive(Debug, Clone)]
pub struct NixInfo {
    pub is_nixos: bool,
    pub nix_expr_dir: PathBuf,
    pub gc_root_dir: PathBuf,
    pub wine_profile: Option<PathBuf>,
}

/// Nix-managed environment
pub struct NixEnvironment {
    pub id: String,
    pub gc_root: PathBuf,
    pub fhs_wrapper: PathBuf,
    pub prefix_path: PathBuf,
}
