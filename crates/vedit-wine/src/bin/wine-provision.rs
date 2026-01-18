//! CLI tool for provisioning and testing Wine prefixes for vedit
//!
//! Working configuration discovered:
//! - 64-bit Wine prefix (win64)
//! - Windows 10 version (required for .NET 4.8)
//! - Windows fonts (corefonts, tahoma) via winetricks
//! - .NET Framework 4.8 via winetricks for C# projects
//! - MSVC Build Tools via msvc-wine for C++ .vcxproj/.sln projects
//!
//! Note: VS Build Tools 2022 installer fails due to WinRT API limitations in Wine.
//! However, msvc-wine can download and install the MSVC toolchain directly.
//!
//! C++ Compilation:
//! - MSBuild 17.14 + MSVC 14.44 (VS 2022) works for .vcxproj files
//! - Requires: msvc-wine toolchain at ~/.local/share/vedit/msvc
//!
//! Usage:
//!   wine-provision create <name> [--arch 32|64]
//!   wine-provision setup-full <name>           # Full setup: prefix + fonts + dotnet + msvc
//!   wine-provision install-fonts <prefix-path>
//!   wine-provision install-dotnet <prefix-path>
//!   wine-provision install-msvc <prefix-path>  # Install MSVC toolchain from msvc-wine
//!   wine-provision set-win10 <prefix-path>
//!   wine-provision test <prefix-path>
//!   wine-provision test-msbuild <prefix-path>
//!   wine-provision test-vcxproj <prefix-path>  # Test C++ .vcxproj build
//!   wine-provision build <prefix-path> <project.vcxproj|solution.sln> [config] [platform]
//!   wine-provision run <prefix-path> <executable> [args...]

use std::path::PathBuf;
use std::process::Command;

fn is_nixos() -> bool {
    std::path::Path::new("/etc/nixos").exists() || std::env::var("NIX_PATH").is_ok()
}

fn has_steam_run() -> bool {
    which::which("steam-run").is_ok()
}

fn run_wine_command(prefix: &PathBuf, args: &[&str]) -> std::io::Result<std::process::ExitStatus> {
    println!("Running: wine {}", args.join(" "));
    println!("  WINEPREFIX={}", prefix.display());

    if is_nixos() && has_steam_run() {
        println!("  Using steam-run wrapper");
        let mut cmd = Command::new("steam-run");
        cmd.arg("wine");
        for arg in args {
            cmd.arg(arg);
        }
        cmd.env("WINEPREFIX", prefix)
            .env("WINEDEBUG", "-all")
            .status()
    } else {
        let mut cmd = Command::new("wine");
        for arg in args {
            cmd.arg(arg);
        }
        cmd.env("WINEPREFIX", prefix)
            .env("WINEDEBUG", "-all")
            .status()
    }
}

fn run_winetricks(prefix: &PathBuf, args: &[&str]) -> std::io::Result<std::process::ExitStatus> {
    println!("Running: winetricks {}", args.join(" "));
    println!("  WINEPREFIX={}", prefix.display());

    if is_nixos() && has_steam_run() {
        println!("  Using steam-run wrapper");
        let mut cmd = Command::new("steam-run");
        cmd.arg("winetricks");
        for arg in args {
            cmd.arg(arg);
        }
        cmd.env("WINEPREFIX", prefix)
            .env("WINEDEBUG", "-all")
            .env("TMPDIR", "/tmp")
            .env("HOME", std::env::var("HOME").unwrap_or_default())
            .status()
    } else {
        let mut cmd = Command::new("winetricks");
        for arg in args {
            cmd.arg(arg);
        }
        cmd.env("WINEPREFIX", prefix)
            .env("WINEDEBUG", "-all")
            .status()
    }
}

fn create_prefix(name: &str, arch: &str) -> Result<PathBuf, String> {
    let prefix_dir = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("vedit")
        .join("wine-prefixes")
        .join(name);

    println!("Creating Wine prefix: {}", prefix_dir.display());
    println!("  Architecture: {}", arch);

    std::fs::create_dir_all(&prefix_dir)
        .map_err(|e| format!("Failed to create directory: {}", e))?;

    // Initialize with wineboot
    let status = if is_nixos() && has_steam_run() {
        println!("  Using steam-run for wineboot");
        Command::new("steam-run")
            .arg("wine")
            .arg("wineboot")
            .arg("--init")
            .env("WINEPREFIX", &prefix_dir)
            .env("WINEARCH", format!("win{}", arch))
            .env("WINEDEBUG", "-all")
            .status()
    } else {
        Command::new("wine")
            .arg("wineboot")
            .arg("--init")
            .env("WINEPREFIX", &prefix_dir)
            .env("WINEARCH", format!("win{}", arch))
            .env("WINEDEBUG", "-all")
            .status()
    };

    match status {
        Ok(s) if s.success() => {
            println!("Prefix created successfully!");
            Ok(prefix_dir)
        }
        Ok(s) => {
            println!(
                "wineboot exited with code {:?}, but prefix may still be usable",
                s.code()
            );
            Ok(prefix_dir)
        }
        Err(e) => Err(format!("Failed to run wineboot: {}", e)),
    }
}

fn install_dotnet48(prefix: &PathBuf) -> Result<(), String> {
    println!("\n=== Installing .NET Framework 4.8 ===\n");

    // First try winetricks dotnet48
    println!("Attempting: winetricks dotnet48");
    let status = run_winetricks(prefix, &["-q", "dotnet48"]);

    match status {
        Ok(s) if s.success() => {
            println!(".NET 4.8 installed successfully via winetricks!");
            return Ok(());
        }
        Ok(s) => {
            println!("winetricks dotnet48 exited with code {:?}", s.code());
        }
        Err(e) => {
            println!("winetricks failed: {}", e);
        }
    }

    // Try direct download and install
    println!("\n--- Trying direct .NET 4.8 installer ---\n");

    let cache_dir = dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("vedit");
    let _ = std::fs::create_dir_all(&cache_dir);
    let dotnet_path = cache_dir.join("ndp48-x86-x64-allos-enu.exe");

    if !dotnet_path.exists() {
        println!("Downloading .NET 4.8...");
        let url = "https://download.visualstudio.microsoft.com/download/pr/7afca223-55d2-470a-8edc-6a1739ae3252/abd170b4b0ec15ad0222a809b761a036/ndp48-x86-x64-allos-enu.exe";
        let status = Command::new("curl")
            .arg("-L")
            .arg("-o")
            .arg(&dotnet_path)
            .arg(url)
            .status()
            .map_err(|e| format!("curl failed: {}", e))?;

        if !status.success() {
            return Err("Failed to download .NET 4.8".to_string());
        }
    }

    println!("Running .NET 4.8 installer...");
    let status = run_wine_command(prefix, &[dotnet_path.to_str().unwrap(), "/q", "/norestart"])
        .map_err(|e| format!("Failed to run .NET installer: {}", e))?;

    if status.success() {
        println!(".NET 4.8 installed successfully!");
        Ok(())
    } else {
        Err(format!(
            ".NET 4.8 installer exited with code {:?}",
            status.code()
        ))
    }
}

fn install_fonts(prefix: &PathBuf) -> Result<(), String> {
    println!("\n=== Installing Windows Fonts ===\n");
    println!("Installing corefonts and tahoma (required for WPF applications)...");

    let status = run_winetricks(prefix, &["-q", "corefonts", "tahoma"]);
    match status {
        Ok(s) if s.success() => {
            println!("Fonts installed successfully!");
            Ok(())
        }
        Ok(s) => {
            println!(
                "Font installation exited with code {:?}, may still be usable",
                s.code()
            );
            Ok(())
        }
        Err(e) => Err(format!("Failed to install fonts: {}", e)),
    }
}

fn set_win10(prefix: &PathBuf) -> Result<(), String> {
    println!("\n=== Setting Windows Version to Windows 10 ===\n");

    let status = run_wine_command(prefix, &["winecfg", "-v", "win10"]);
    match status {
        Ok(s) if s.success() => {
            println!("Windows version set to Windows 10!");
            Ok(())
        }
        Ok(s) => Err(format!("winecfg exited with code {:?}", s.code())),
        Err(e) => Err(format!("Failed to run winecfg: {}", e)),
    }
}

fn find_mingw_dlls() -> Vec<PathBuf> {
    // Search for MinGW DLLs in nix store
    let mut dlls = Vec::new();

    // Try to find common MinGW DLLs
    let patterns = [
        ("libmcfgthread", "mcfgthread"),
        ("libstdc++", "gcc"),
        ("libgcc_s_seh", "gcc"),
    ];

    for (dll_prefix, _pkg_hint) in patterns {
        // Use find to locate DLLs
        let output = std::process::Command::new("find")
            .args([
                "/nix/store",
                "-maxdepth",
                "4",
                "-name",
                &format!("{}*.dll", dll_prefix),
                "-path",
                "*mingw*",
            ])
            .output();

        if let Ok(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let path = PathBuf::from(line.trim());
                if path.exists()
                    && !dlls
                        .iter()
                        .any(|p: &PathBuf| p.file_name() == path.file_name())
                {
                    dlls.push(path);
                    break; // Just take the first match for each pattern
                }
            }
        }
    }

    dlls
}

fn install_mingw_dlls(prefix: &PathBuf) -> Result<(), String> {
    println!("\n=== Installing MinGW Runtime DLLs ===\n");

    let system32 = prefix.join("drive_c/windows/system32");
    if !system32.exists() {
        return Err(format!("system32 not found: {}", system32.display()));
    }

    let dlls = find_mingw_dlls();
    if dlls.is_empty() {
        println!("No MinGW DLLs found in nix store.");
        println!("Try: nix-shell -p pkgsCross.mingwW64.buildPackages.gcc");
        return Err("MinGW DLLs not found".to_string());
    }

    for dll in &dlls {
        let dest = system32.join(dll.file_name().unwrap());
        match std::fs::copy(dll, &dest) {
            Ok(_) => println!("✓ Installed {}", dll.file_name().unwrap().to_string_lossy()),
            Err(e) => println!("✗ Failed to copy {}: {}", dll.display(), e),
        }
    }

    println!("\nMinGW runtime DLLs installed to system32");
    Ok(())
}

fn test_cpp(prefix: &PathBuf) -> Result<(), String> {
    println!("\n=== Testing MinGW C++ Compilation ===\n");

    // Create a test C++ file
    let test_dir = prefix.join("drive_c/test-mingw-cpp");
    std::fs::create_dir_all(&test_dir).map_err(|e| format!("Failed to create test dir: {}", e))?;

    let cpp_file = test_dir.join("test.cpp");
    let cpp_code = r#"#include <iostream>
int main() {
    std::cout << "Hello from MinGW C++!" << std::endl;
    return 0;
}
"#;
    std::fs::write(&cpp_file, cpp_code).map_err(|e| format!("Failed to write test.cpp: {}", e))?;

    let exe_file = test_dir.join("test.exe");

    // Try to compile with MinGW
    println!("Compiling test.cpp with MinGW...");
    let compile = std::process::Command::new("nix-shell")
        .args([
            "-p",
            "pkgsCross.mingwW64.buildPackages.gcc",
            "--run",
            &format!(
                "x86_64-w64-mingw32-g++ -static-libgcc -static-libstdc++ -o {} {}",
                exe_file.display(),
                cpp_file.display()
            ),
        ])
        .status();

    match compile {
        Ok(s) if s.success() => {
            println!("✓ Compilation successful");
        }
        Ok(s) => {
            return Err(format!("Compilation failed with code {:?}", s.code()));
        }
        Err(e) => {
            return Err(format!("Failed to run nix-shell: {}", e));
        }
    }

    // Run the compiled exe in Wine
    println!("\nRunning test.exe in Wine...");
    let status = run_wine_command(prefix, &["C:\\test-mingw-cpp\\test.exe"]);

    match status {
        Ok(s) if s.success() => {
            println!("\n✓ MinGW C++ compilation and execution works!");
            Ok(())
        }
        Ok(s) => Err(format!("test.exe exited with code {:?}", s.code())),
        Err(e) => Err(format!("Failed to run test.exe: {}", e)),
    }
}

fn get_msvc_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("vedit")
        .join("msvc")
}

fn install_msvc(prefix: &PathBuf) -> Result<(), String> {
    println!("\n=== Installing MSVC Toolchain ===\n");

    let msvc_dir = get_msvc_dir();
    let vs_root = prefix.join("drive_c/Program Files/Microsoft Visual Studio/2022/BuildTools");

    // Check if msvc-wine has been downloaded
    if !msvc_dir.join("VC").exists() {
        println!("MSVC toolchain not found at {}", msvc_dir.display());
        println!("\nTo download MSVC, run these commands:");
        println!("  git clone https://github.com/mstorsjo/msvc-wine /tmp/msvc-wine");
        println!("  cd /tmp/msvc-wine");
        println!(
            "  nix-shell -p msitools python3 --run 'python3 vsdownload.py --accept-license --dest {}'",
            msvc_dir.display()
        );
        println!("  ./install.sh {}", msvc_dir.display());
        return Err("MSVC toolchain not downloaded".to_string());
    }

    println!("Found MSVC at {}", msvc_dir.display());
    println!("Installing to Wine prefix...");

    // Create VS installation directory
    std::fs::create_dir_all(&vs_root).map_err(|e| format!("Failed to create VS dir: {}", e))?;

    // Copy MSBuild
    let src_msbuild = msvc_dir.join("MSBuild");
    let dst_msbuild = vs_root.join("MSBuild");
    if src_msbuild.exists() && !dst_msbuild.exists() {
        copy_dir_recursive(&src_msbuild, &dst_msbuild)?;
        println!("✓ Copied MSBuild");
    }

    // Copy VC toolchain
    let src_vc = msvc_dir.join("VC");
    let dst_vc = vs_root.join("VC");
    if src_vc.exists() && !dst_vc.exists() {
        copy_dir_recursive(&src_vc, &dst_vc)?;
        println!("✓ Copied VC toolchain");
    }

    // Copy Windows Kits
    let src_kits = msvc_dir.join("Windows Kits");
    let dst_kits = vs_root.join("Windows Kits");
    if src_kits.exists() && !dst_kits.exists() {
        copy_dir_recursive(&src_kits, &dst_kits)?;
        println!("✓ Copied Windows SDK");
    }

    // Copy runtime DLLs to system32
    let system32 = prefix.join("drive_c/windows/system32");
    copy_runtime_dlls(&msvc_dir, &system32)?;

    println!("\n✓ MSVC toolchain installed");
    println!(
        "  MSBuild: C:\\Program Files\\Microsoft Visual Studio\\2022\\BuildTools\\MSBuild\\Current\\Bin\\amd64\\MSBuild.exe"
    );

    Ok(())
}

fn copy_dir_recursive(src: &PathBuf, dst: &PathBuf) -> Result<(), String> {
    std::fs::create_dir_all(dst)
        .map_err(|e| format!("Failed to create {}: {}", dst.display(), e))?;

    for entry in
        std::fs::read_dir(src).map_err(|e| format!("Failed to read {}: {}", src.display(), e))?
    {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)
                .map_err(|e| format!("Failed to copy {:?}: {}", src_path, e))?;
        }
    }
    Ok(())
}

fn copy_runtime_dlls(msvc_dir: &PathBuf, system32: &PathBuf) -> Result<(), String> {
    // Find and copy debug CRT DLLs
    let debug_crt = msvc_dir.join("VC/Redist/MSVC");
    if debug_crt.exists() {
        for entry in walkdir::WalkDir::new(&debug_crt)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.is_file()
                && path.extension().map(|e| e == "dll").unwrap_or(false)
                && path.to_string_lossy().contains("x64")
            {
                let filename = path.file_name().unwrap();
                let dst = system32.join(filename);
                if !dst.exists() {
                    std::fs::copy(path, &dst).ok();
                }
            }
        }
    }

    // Copy UCRT debug DLLs
    let ucrt_debug = msvc_dir.join("Windows Kits/10/bin");
    if ucrt_debug.exists() {
        for entry in walkdir::WalkDir::new(&ucrt_debug)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.is_file()
                && path
                    .file_name()
                    .map(|n| n == "ucrtbased.dll")
                    .unwrap_or(false)
                && path.to_string_lossy().contains("x64")
            {
                let dst = system32.join("ucrtbased.dll");
                if !dst.exists() {
                    std::fs::copy(path, &dst).ok();
                }
                break;
            }
        }
    }

    println!("✓ Copied runtime DLLs");
    Ok(())
}

fn build_project(
    prefix: &PathBuf,
    project: &str,
    config: &str,
    platform: &str,
) -> Result<(), String> {
    println!("\n=== Building {} ===\n", project);

    let msbuild = r"C:\Program Files\Microsoft Visual Studio\2022\BuildTools\MSBuild\Current\Bin\amd64\MSBuild.exe";
    let vs_root = r"C:\Program Files\Microsoft Visual Studio\2022\BuildTools";

    let config_arg = format!("/p:Configuration={}", config);
    let platform_arg = format!("/p:Platform={}", platform);
    let vsroot_arg = format!("/p:VSInstallRoot={}", vs_root);

    let status = run_wine_command(
        prefix,
        &[msbuild, project, &config_arg, &platform_arg, &vsroot_arg],
    )
    .map_err(|e| format!("Failed to run MSBuild: {}", e))?;

    if status.success() {
        println!("\n✓ Build succeeded!");
        Ok(())
    } else {
        Err(format!("Build failed with code {:?}", status.code()))
    }
}

fn test_vcxproj(prefix: &PathBuf) -> Result<(), String> {
    println!("\n=== Testing MSVC C++ Build ===\n");

    // Create test project
    let test_dir = prefix.join("drive_c/test-msvc-cpp");
    std::fs::create_dir_all(&test_dir).map_err(|e| format!("Failed to create test dir: {}", e))?;

    // Write test source
    let cpp_code = r#"#include <iostream>
int main() {
    std::cout << "Hello from MSVC in Wine!" << std::endl;
    return 0;
}
"#;
    std::fs::write(test_dir.join("main.cpp"), cpp_code)
        .map_err(|e| format!("Failed to write main.cpp: {}", e))?;

    // Write vcxproj
    let vcxproj = r#"<?xml version="1.0" encoding="utf-8"?>
<Project DefaultTargets="Build" ToolsVersion="17.0" xmlns="http://schemas.microsoft.com/developer/msbuild/2003">
  <ItemGroup Label="ProjectConfigurations">
    <ProjectConfiguration Include="Debug|x64">
      <Configuration>Debug</Configuration>
      <Platform>x64</Platform>
    </ProjectConfiguration>
  </ItemGroup>
  <PropertyGroup Label="Globals">
    <ProjectGuid>{12345678-1234-1234-1234-123456789ABC}</ProjectGuid>
    <WindowsTargetPlatformVersion>10.0</WindowsTargetPlatformVersion>
  </PropertyGroup>
  <Import Project="$(VCTargetsPath)\Microsoft.Cpp.Default.props" />
  <PropertyGroup Label="Configuration">
    <ConfigurationType>Application</ConfigurationType>
    <UseDebugLibraries>true</UseDebugLibraries>
    <PlatformToolset>v143</PlatformToolset>
  </PropertyGroup>
  <Import Project="$(VCTargetsPath)\Microsoft.Cpp.props" />
  <ItemGroup>
    <ClCompile Include="main.cpp" />
  </ItemGroup>
  <Import Project="$(VCTargetsPath)\Microsoft.Cpp.targets" />
</Project>
"#;
    std::fs::write(test_dir.join("test.vcxproj"), vcxproj)
        .map_err(|e| format!("Failed to write vcxproj: {}", e))?;

    // Build it
    build_project(prefix, r"C:\test-msvc-cpp\test.vcxproj", "Debug", "x64")?;

    // Run it
    println!("\nRunning compiled executable...");
    let status = run_wine_command(prefix, &[r"C:\test-msvc-cpp\x64\Debug\test.exe"]);

    match status {
        Ok(s) if s.success() => {
            println!("\n✓ MSVC C++ compilation and execution works!");
            Ok(())
        }
        Ok(s) => Err(format!("test.exe exited with code {:?}", s.code())),
        Err(e) => Err(format!("Failed to run test.exe: {}", e)),
    }
}

fn setup_full(name: &str) -> Result<PathBuf, String> {
    println!("\n=== Full Wine Prefix Setup ===\n");
    println!("This will:");
    println!("  1. Create a 64-bit Wine prefix");
    println!("  2. Set Windows version to Windows 10");
    println!("  3. Install Windows fonts (corefonts, tahoma)");
    println!("  4. Install .NET Framework 4.8 (for C# MSBuild)");
    println!("  5. Install MSVC toolchain (for C++ .vcxproj)");
    println!();

    // Create prefix
    let prefix = create_prefix(name, "64")?;

    // Set Windows 10 version
    set_win10(&prefix)?;

    // Install fonts
    install_fonts(&prefix)?;

    // Install .NET Framework 4.8
    install_dotnet48(&prefix)?;

    // Install MSVC toolchain (if available)
    if get_msvc_dir().join("VC").exists() {
        install_msvc(&prefix)?;
    } else {
        println!("\n⚠ MSVC toolchain not found - C++ builds will not work");
        println!("  Run: wine-provision install-msvc <prefix> after downloading msvc-wine");
    }

    println!("\n=== Setup Complete ===\n");
    println!("Prefix: {}", prefix.display());
    println!("C# MSBuild: C:\\windows\\Microsoft.NET\\Framework64\\v4.0.30319\\MSBuild.exe");
    println!(
        "C++ MSBuild: C:\\Program Files\\Microsoft Visual Studio\\2022\\BuildTools\\MSBuild\\Current\\Bin\\amd64\\MSBuild.exe"
    );

    Ok(prefix)
}

fn test_msbuild(prefix: &PathBuf) -> Result<(), String> {
    println!("\n=== Testing MSBuild ===\n");

    let msbuild_path = "C:\\windows\\Microsoft.NET\\Framework64\\v4.0.30319\\MSBuild.exe";

    println!("Testing: {} /version", msbuild_path);
    let status = run_wine_command(prefix, &[msbuild_path, "/version"]);

    match status {
        Ok(s) if s.success() => {
            println!("\n✓ MSBuild is working!");
            Ok(())
        }
        Ok(s) => Err(format!("MSBuild exited with code {:?}", s.code())),
        Err(e) => Err(format!("Failed to run MSBuild: {}", e)),
    }
}

fn test_prefix(prefix: &PathBuf) -> Result<(), String> {
    println!("\n=== Testing Wine Prefix ===\n");
    println!("Prefix: {}", prefix.display());

    // Check if prefix exists
    if !prefix.exists() {
        return Err(format!("Prefix does not exist: {}", prefix.display()));
    }

    // Check for system.reg
    let system_reg = prefix.join("system.reg");
    if system_reg.exists() {
        println!("✓ system.reg exists");
    } else {
        println!("✗ system.reg missing");
    }

    // Check for drive_c
    let drive_c = prefix.join("drive_c");
    if drive_c.exists() {
        println!("✓ drive_c exists");
    } else {
        println!("✗ drive_c missing");
    }

    // Check .NET installation
    let dotnet_dir = drive_c.join("windows/Microsoft.NET/Framework64");
    if dotnet_dir.exists() {
        println!("✓ .NET Framework directory exists");
        if let Ok(entries) = std::fs::read_dir(&dotnet_dir) {
            for entry in entries.flatten() {
                println!("  - {}", entry.file_name().to_string_lossy());
            }
        }
    } else {
        println!("✗ .NET Framework directory missing");
    }

    // Try running a simple command
    println!("\nTesting wine cmd.exe /c echo test...");
    let status = run_wine_command(prefix, &["cmd.exe", "/c", "echo", "Wine is working!"]);
    match status {
        Ok(s) if s.success() => println!("✓ Wine cmd.exe works"),
        Ok(s) => println!("✗ Wine cmd.exe exited with code {:?}", s.code()),
        Err(e) => println!("✗ Wine cmd.exe failed: {}", e),
    }

    // Check MSBuild
    let msbuild_path = drive_c.join("windows/Microsoft.NET/Framework64/v4.0.30319/MSBuild.exe");
    if msbuild_path.exists() {
        println!("✓ MSBuild.exe exists");
        println!("  Path: C:\\windows\\Microsoft.NET\\Framework64\\v4.0.30319\\MSBuild.exe");
    } else {
        println!("✗ MSBuild.exe not found (run install-dotnet to install .NET Framework)");
    }

    Ok(())
}

fn run_executable(prefix: &PathBuf, exe: &str, args: &[String]) -> Result<(), String> {
    println!("\n=== Running Executable ===\n");
    println!("Prefix: {}", prefix.display());
    println!("Executable: {}", exe);
    println!("Args: {:?}", args);

    let mut wine_args: Vec<&str> = vec![exe];
    for arg in args {
        wine_args.push(arg);
    }

    let status =
        run_wine_command(prefix, &wine_args).map_err(|e| format!("Failed to run: {}", e))?;

    println!("\nExit code: {:?}", status.code());
    Ok(())
}

fn print_help() {
    println!("Wine Prefix Provisioning Tool for vedit");
    println!();
    println!("Quick Start:");
    println!("  wine-provision setup-full <name>    # Create fully provisioned prefix");
    println!();
    println!("Individual Commands:");
    println!("  wine-provision create <name> [--arch 32|64]");
    println!("  wine-provision set-win10 <prefix-path>");
    println!("  wine-provision install-fonts <prefix-path>");
    println!("  wine-provision install-dotnet <prefix-path>");
    println!("  wine-provision install-msvc <prefix-path>");
    println!("  wine-provision test <prefix-path>");
    println!("  wine-provision test-msbuild <prefix-path>");
    println!("  wine-provision test-vcxproj <prefix-path>");
    println!("  wine-provision build <prefix-path> <project> [config] [platform]");
    println!("  wine-provision run <prefix-path> <executable> [args...]");
    println!();
    println!("C# Projects:");
    println!("  - .NET Framework 4.8 MSBuild works for .csproj files");
    println!("  - MSBuild: C:\\windows\\Microsoft.NET\\Framework64\\v4.0.30319\\MSBuild.exe");
    println!();
    println!("C++ Projects (.vcxproj/.sln):");
    println!("  - Requires MSVC toolchain via msvc-wine");
    println!("  - Download: git clone https://github.com/mstorsjo/msvc-wine /tmp/msvc-wine");
    println!("  - Install:  cd /tmp/msvc-wine && nix-shell -p msitools python3 --run \\");
    println!(
        "              'python3 vsdownload.py --accept-license --dest {}'",
        get_msvc_dir().display()
    );
    println!("              ./install.sh {}", get_msvc_dir().display());
    println!();
    println!("Environment:");
    println!("  NixOS: {}", is_nixos());
    println!("  steam-run: {}", has_steam_run());
    println!("  MSVC toolchain: {}", get_msvc_dir().join("VC").exists());
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        print_help();
        return;
    }

    let result = match args[1].as_str() {
        "create" => {
            if args.len() < 3 {
                Err("Usage: wine-provision create <name> [--arch 32|64]".to_string())
            } else {
                let name = &args[2];
                let arch = if args.len() > 4 && args[3] == "--arch" {
                    &args[4]
                } else {
                    "64"
                };
                create_prefix(name, arch).map(|p| println!("Created: {}", p.display()))
            }
        }
        "setup-full" => {
            if args.len() < 3 {
                Err("Usage: wine-provision setup-full <name>".to_string())
            } else {
                let name = &args[2];
                setup_full(name).map(|p| println!("Full setup complete: {}", p.display()))
            }
        }
        "set-win10" => {
            if args.len() < 3 {
                Err("Usage: wine-provision set-win10 <prefix-path>".to_string())
            } else {
                let prefix = PathBuf::from(&args[2]);
                set_win10(&prefix)
            }
        }
        "install-fonts" => {
            if args.len() < 3 {
                Err("Usage: wine-provision install-fonts <prefix-path>".to_string())
            } else {
                let prefix = PathBuf::from(&args[2]);
                install_fonts(&prefix)
            }
        }
        "install-dotnet" => {
            if args.len() < 3 {
                Err("Usage: wine-provision install-dotnet <prefix-path>".to_string())
            } else {
                let prefix = PathBuf::from(&args[2]);
                install_dotnet48(&prefix)
            }
        }
        "test" => {
            if args.len() < 3 {
                Err("Usage: wine-provision test <prefix-path>".to_string())
            } else {
                let prefix = PathBuf::from(&args[2]);
                test_prefix(&prefix)
            }
        }
        "test-msbuild" => {
            if args.len() < 3 {
                Err("Usage: wine-provision test-msbuild <prefix-path>".to_string())
            } else {
                let prefix = PathBuf::from(&args[2]);
                test_msbuild(&prefix)
            }
        }
        "install-mingw-dlls" => {
            if args.len() < 3 {
                Err("Usage: wine-provision install-mingw-dlls <prefix-path>".to_string())
            } else {
                let prefix = PathBuf::from(&args[2]);
                install_mingw_dlls(&prefix)
            }
        }
        "test-cpp" => {
            if args.len() < 3 {
                Err("Usage: wine-provision test-cpp <prefix-path>".to_string())
            } else {
                let prefix = PathBuf::from(&args[2]);
                test_cpp(&prefix)
            }
        }
        "install-msvc" => {
            if args.len() < 3 {
                Err("Usage: wine-provision install-msvc <prefix-path>".to_string())
            } else {
                let prefix = PathBuf::from(&args[2]);
                install_msvc(&prefix)
            }
        }
        "test-vcxproj" => {
            if args.len() < 3 {
                Err("Usage: wine-provision test-vcxproj <prefix-path>".to_string())
            } else {
                let prefix = PathBuf::from(&args[2]);
                test_vcxproj(&prefix)
            }
        }
        "build" => {
            if args.len() < 4 {
                Err(
                    "Usage: wine-provision build <prefix-path> <project> [config] [platform]"
                        .to_string(),
                )
            } else {
                let prefix = PathBuf::from(&args[2]);
                let project = &args[3];
                let config = args.get(4).map(|s| s.as_str()).unwrap_or("Debug");
                let platform = args.get(5).map(|s| s.as_str()).unwrap_or("x64");
                build_project(&prefix, project, config, platform)
            }
        }
        "run" => {
            if args.len() < 4 {
                Err("Usage: wine-provision run <prefix-path> <executable> [args...]".to_string())
            } else {
                let prefix = PathBuf::from(&args[2]);
                let exe = &args[3];
                let exe_args: Vec<String> = args[4..].to_vec();
                run_executable(&prefix, exe, &exe_args)
            }
        }
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        _ => {
            print_help();
            Err(format!("Unknown command: {}", args[1]))
        }
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
