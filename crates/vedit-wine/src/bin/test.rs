//! Test binary for vedit-wine integration

use std::path::PathBuf;
use tokio;
use vedit_wine::environment::{WindowsVersion, WineArchitecture};
use vedit_wine::{WineEnvironmentConfig, WineManager, WineProcessConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("🍷 Testing vedit-wine integration");

    // Test Wine availability
    if !vedit_wine::WineManager::is_wine_available() {
        println!("❌ Wine is not available on this system");
        return Ok(());
    }

    if vedit_wine::WineManager::is_nixos() {
        println!("✅ Running on NixOS - using Nix integration");
    } else {
        println!("ℹ️ Not running on NixOS - using standard Wine integration");
    }

    // Create Wine manager
    let mut wine_manager = WineManager::new()?;
    println!("✅ Wine manager created successfully");

    // Create test project directory
    let project_path = PathBuf::from("/tmp/vedit-wine-test");
    std::fs::create_dir_all(&project_path)?;
    println!(
        "📁 Created test project directory: {}",
        project_path.display()
    );

    // Create Wine environment
    let env_config = WineEnvironmentConfig {
        wine_version: None,
        windows_version: WindowsVersion::Windows10,
        dll_overrides: std::collections::HashMap::new(),
        runtimes: vec![],
        display: vedit_wine::environment::DisplayConfig::default(),
        audio: vedit_wine::environment::AudioConfig::default(),
        architecture: WineArchitecture::Win64,
    };

    let env_id = wine_manager
        .create_environment(&project_path, "test-env", env_config)
        .await?;
    println!("✅ Created Wine environment: {}", env_id);

    // Get environment info
    let environment = wine_manager.environments().get(&env_id).unwrap();
    let env_info = environment.info();
    println!("📊 Environment info:");
    println!("   ID: {}", env_info.id);
    println!("   Prefix: {}", env_info.prefix_path.display());
    println!("   Architecture: {:?}", env_info.architecture);
    println!("   Windows version: {:?}", env_info.windows_version);

    // Test spawning a simple Windows application (notepad.exe if available)
    let test_exe = PathBuf::from("C:\\windows\\system32\\notepad.exe");

    println!("🚀 Testing process spawning...");
    match wine_manager
        .spawn_app(&env_id, &test_exe, &[], WineProcessConfig::default())
        .await
    {
        Ok(process_id) => {
            println!("✅ Process spawned successfully: {}", process_id);

            // Wait a bit then kill it
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

            if let Some(process) = wine_manager.active_processes().get_mut(&process_id) {
                process.kill().await?;
                println!("🛑 Test process terminated");
            }
        }
        Err(e) => {
            println!(
                "⚠️ Failed to spawn test process (this is expected if notepad.exe is not available): {}",
                e
            );
        }
    }

    // Test configuration
    let config = wine_manager.config;
    println!("⚙️ Wine configuration:");
    println!("   Default Wine version: {:?}", config.default_wine_version);
    println!(
        "   Default Windows version: {:?}",
        config.default_windows_version
    );
    println!("   Default architecture: {:?}", config.default_architecture);

    // Test remote desktop functionality
    println!("🖥️ Testing remote desktop functionality...");
    let mut remote_desktop =
        vedit_wine::RemoteDesktop::new(vedit_wine::RemoteDesktopConfig::default());

    match remote_desktop
        .create_session(
            vedit_wine::remote_desktop::DesktopType::Vnc,
            None,
            Some((800, 600)),
        )
        .await
    {
        Ok(session_id) => {
            println!("✅ VNC session created: {}", session_id);

            if let Some(conn_info) = remote_desktop.get_connection_info(&session_id) {
                println!("   Connection URL: {}", conn_info.connection_url);
                println!("   Port: {}", conn_info.port);
                println!(
                    "   Resolution: {}x{}",
                    conn_info.resolution.0, conn_info.resolution.1
                );
            }

            // Clean up session
            remote_desktop.close_session(&session_id).await?;
            println!("🧹 VNC session closed");
        }
        Err(e) => {
            println!(
                "⚠️ Failed to create VNC session (this may be expected if Xvfb/x11vnc are not available): {}",
                e
            );
        }
    }

    // Cleanup
    std::fs::remove_dir_all(&project_path)?;
    println!("🧹 Cleaned up test directory");

    println!("🎉 Wine integration test completed!");
    Ok(())
}
