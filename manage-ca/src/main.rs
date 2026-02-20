use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(target_os = "linux")]
use walkdir::WalkDir;

#[derive(Parser)]
#[command(name = "manage-ca")]
#[command(about = "Manage Local CA certificates for System and Browsers", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Path to the CA certificate file
    #[arg(short, long, global = true, default_value = "certs/ca/ca.crt")]
    cert: PathBuf,

    /// Name of the certificate in the system store (without extension)
    #[arg(short, long, global = true, default_value = "local-dev-ca")]
    name: String,
}

#[derive(Subcommand)]
enum Commands {
    /// Install the CA to System and configure Browsers
    Install,
    /// Uninstall the CA from System and Browsers
    Uninstall,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Install => install_ca(&cli.cert, &cli.name)?,
        Commands::Uninstall => uninstall_ca(&cli.cert, &cli.name)?,
    }

    Ok(())
}

fn install_ca(cert_path: &Path, name: &str) -> Result<()> {
    println!("=== Installing CA ===");

    if !cert_path.exists() {
        anyhow::bail!("CA certificate not found at {:?}", cert_path);
    }

    install_system_ca(cert_path, name)?;
    configure_browsers(cert_path, name)?;

    println!("\n=== Installation Complete ===");
    println!("Please restart your browsers.");
    Ok(())
}

fn uninstall_ca(cert_path: &Path, name: &str) -> Result<()> {
    println!("=== Uninstalling CA ===");

    uninstall_system_ca(cert_path, name)?;

    #[cfg(target_os = "linux")]
    uninstall_nss_browsers(cert_path, name)?;

    println!("\n=== Uninstallation Complete ===");
    println!("Please restart your browsers.");
    Ok(())
}

// =============================================================================
// System Store: Linux
// =============================================================================

#[cfg(target_os = "linux")]
enum LinuxCaMethod {
    /// Debian/Ubuntu: update-ca-certificates
    Debian,
    /// Fedora/RHEL/Arch: update-ca-trust
    Trust,
}

#[cfg(target_os = "linux")]
fn detect_ca_method() -> Result<LinuxCaMethod> {
    if which("update-ca-certificates") {
        Ok(LinuxCaMethod::Debian)
    } else if which("update-ca-trust") {
        Ok(LinuxCaMethod::Trust)
    } else {
        anyhow::bail!(
            "Neither 'update-ca-certificates' nor 'update-ca-trust' found.\n\
             Install ca-certificates:\n\
             \n  Ubuntu/Debian: sudo apt install ca-certificates\
             \n  Fedora/RHEL:   sudo dnf install ca-certificates\
             \n  Arch:          sudo pacman -S ca-certificates-utils"
        );
    }
}

#[cfg(target_os = "linux")]
fn which(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .output()
        .is_ok_and(|o| o.status.success())
}

#[cfg(target_os = "linux")]
fn install_system_ca(cert_path: &Path, name: &str) -> Result<()> {
    println!("-> System Store (Linux):");

    let method = detect_ca_method()?;

    let dest_dir = match method {
        LinuxCaMethod::Debian => Path::new("/usr/local/share/ca-certificates"),
        LinuxCaMethod::Trust => Path::new("/etc/pki/ca-trust/source/anchors"),
    };

    if !dest_dir.exists() {
        fs::create_dir_all(dest_dir).context("Failed to create certificate dir")?;
    }

    let dest_path = dest_dir.join(format!("{}.crt", name));
    println!("   Copying cert to {:?}", dest_path);
    fs::copy(cert_path, &dest_path).context("Failed to copy certificate")?;

    println!("   Updating certificates...");
    let status = match method {
        LinuxCaMethod::Debian => Command::new("update-ca-certificates").status(),
        LinuxCaMethod::Trust => Command::new("update-ca-trust").arg("extract").status(),
    }
    .context("Failed to update certificates")?;

    if !status.success() {
        anyhow::bail!("Certificate update command failed");
    }

    println!("   ✓ System CA installed");
    Ok(())
}

#[cfg(target_os = "linux")]
fn uninstall_system_ca(_cert_path: &Path, name: &str) -> Result<()> {
    println!("-> System Store (Linux):");
    let method = detect_ca_method()?;

    let dest_dir = match method {
        LinuxCaMethod::Debian => Path::new("/usr/local/share/ca-certificates"),
        LinuxCaMethod::Trust => Path::new("/etc/pki/ca-trust/source/anchors"),
    };

    let dest_path = dest_dir.join(format!("{}.crt", name));

    if dest_path.exists() {
        println!("   Removing {:?}", dest_path);
        fs::remove_file(&dest_path).context("Failed to remove certificate file")?;

        println!("   Updating certificates...");
        let status = match method {
            LinuxCaMethod::Debian => Command::new("update-ca-certificates")
                .arg("--fresh")
                .status(),
            LinuxCaMethod::Trust => Command::new("update-ca-trust").arg("extract").status(),
        }
        .context("Failed to update certificates")?;

        if !status.success() {
            anyhow::bail!("Certificate update command failed");
        }
        println!("   ✓ System CA removed");
    } else {
        println!("   Certificate not found, skipping removal");
    }

    Ok(())
}

// =============================================================================
// System Store: macOS
// =============================================================================

#[cfg(target_os = "macos")]
fn install_system_ca(cert_path: &Path, _name: &str) -> Result<()> {
    println!("-> System Store (macOS Keychain):");

    let status = Command::new("security")
        .args([
            "add-trusted-cert",
            "-d",
            "-r",
            "trustRoot",
            "-k",
            "/Library/Keychains/System.keychain",
        ])
        .arg(cert_path)
        .status()
        .context("Failed to execute 'security' command")?;

    if !status.success() {
        anyhow::bail!("security add-trusted-cert failed. Are you running with sudo?");
    }

    println!("   ✓ CA added to System Keychain");
    Ok(())
}

#[cfg(target_os = "macos")]
fn uninstall_system_ca(cert_path: &Path, _name: &str) -> Result<()> {
    println!("-> System Store (macOS Keychain):");

    if !cert_path.exists() {
        println!("   Certificate file not found at {:?}, skipping", cert_path);
        return Ok(());
    }

    let status = Command::new("security")
        .args(["remove-trusted-cert", "-d"])
        .arg(cert_path)
        .status()
        .context("Failed to execute 'security' command")?;

    if !status.success() {
        eprintln!("   ⚠ security remove-trusted-cert returned error (cert may not exist)");
    } else {
        println!("   ✓ CA removed from System Keychain");
    }

    Ok(())
}

// =============================================================================
// System Store: Windows
// =============================================================================

#[cfg(target_os = "windows")]
fn install_system_ca(cert_path: &Path, _name: &str) -> Result<()> {
    println!("-> System Store (Windows Certificate Store):");

    let status = Command::new("certutil")
        .args(["-addstore", "Root"])
        .arg(cert_path)
        .status()
        .context("Failed to execute certutil. Are you running as Administrator?")?;

    if !status.success() {
        anyhow::bail!("certutil -addstore failed. Run as Administrator.");
    }

    println!("   ✓ CA added to Windows Root store");
    Ok(())
}

#[cfg(target_os = "windows")]
fn uninstall_system_ca(_cert_path: &Path, name: &str) -> Result<()> {
    println!("-> System Store (Windows Certificate Store):");

    let status = Command::new("certutil")
        .args(["-delstore", "Root", name])
        .status()
        .context("Failed to execute certutil. Are you running as Administrator?")?;

    if !status.success() {
        eprintln!("   ⚠ certutil -delstore returned error (cert may not exist)");
    } else {
        println!("   ✓ CA removed from Windows Root store");
    }

    Ok(())
}

// =============================================================================
// Browser Configuration
// =============================================================================

fn configure_browsers(cert_path: &Path, name: &str) -> Result<()> {
    println!("-> Configuring Browsers:");

    configure_firefox(cert_path, name)?;

    #[cfg(target_os = "linux")]
    configure_nss_browsers(cert_path, name)?;

    #[cfg(not(target_os = "linux"))]
    {
        let _ = (cert_path, name);
        println!("   Chrome/Edge use system trust store automatically ✓");
    }

    Ok(())
}

// =============================================================================
// NSS via certutil CLI (Linux only)
// =============================================================================

#[cfg(target_os = "linux")]
fn ensure_certutil() -> Result<()> {
    if !which("certutil") {
        anyhow::bail!(
            "'certutil' not found. Install it with:\n\
             \n  Ubuntu/Debian: sudo apt install libnss3-tools\n\
             \n  Fedora/RHEL:   sudo dnf install nss-tools\n\
             \n  Arch:          sudo pacman -S nss"
        );
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn find_nss_databases() -> Result<Vec<PathBuf>> {
    let home_dir = get_real_home()?;
    let pki_db = home_dir.join(".pki/nssdb");

    let mut dbs = vec![];
    if pki_db.exists() {
        dbs.push(pki_db);
    }

    let snap_dir = home_dir.join("snap");
    if snap_dir.exists() {
        for entry in WalkDir::new(&snap_dir)
            .max_depth(3)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_name() == "nssdb"
                && entry
                    .path()
                    .parent()
                    .is_some_and(|p| p.file_name().is_some_and(|name| name == ".pki"))
            {
                dbs.push(entry.path().to_path_buf());
            }
        }
    }

    Ok(dbs)
}

#[cfg(target_os = "linux")]
fn configure_nss_browsers(cert_path: &Path, name: &str) -> Result<()> {
    ensure_certutil()?;

    let dbs = find_nss_databases()?;
    if dbs.is_empty() {
        return Ok(());
    }

    for db_path in dbs {
        let db_arg = format!("sql:{}", db_path.display());
        // Delete existing cert first (ignore errors if not found)
        let _ = Command::new("certutil")
            .args(["-D", "-d", &db_arg, "-n", name])
            .output();

        let status = Command::new("certutil")
            .args(["-A", "-d", &db_arg, "-t", "C,,", "-n", name, "-i"])
            .arg(cert_path)
            .status();

        match status {
            Ok(s) if s.success() => {
                println!("   ✓ NSS DB {:?}", db_path);
            }
            _ => {
                eprintln!("   x Failed to configure NSS DB at {:?}", db_path);
            }
        }
    }

    Ok(())
}

#[cfg(target_os = "linux")]
fn uninstall_nss_browsers(_cert_path: &Path, name: &str) -> Result<()> {
    println!("-> Browser NSS DBs:");

    ensure_certutil()?;

    let dbs = find_nss_databases()?;
    if dbs.is_empty() {
        return Ok(());
    }

    let nicknames = [name, "Local Development CA"];

    for db_path in dbs {
        let db_arg = format!("sql:{}", db_path.display());
        for nick in &nicknames {
            let _ = Command::new("certutil")
                .args(["-D", "-d", &db_arg, "-n", nick])
                .output();
        }
        println!("   ✓ NSS DB {:?}", db_path);
    }

    Ok(())
}

// =============================================================================
// Firefox Configuration (all platforms)
// =============================================================================

fn configure_firefox(cert_path: &Path, name: &str) -> Result<()> {
    let home_dir = get_real_home()?;

    #[cfg(target_os = "linux")]
    let firefox_dirs = vec![
        home_dir.join(".mozilla/firefox"),
        home_dir.join("snap/firefox/common/.mozilla/firefox"),
    ];

    #[cfg(target_os = "macos")]
    let firefox_dirs = vec![home_dir.join("Library/Application Support/Firefox/Profiles")];

    #[cfg(target_os = "windows")]
    let firefox_dirs = vec![home_dir.join("AppData/Roaming/Mozilla/Firefox/Profiles")];

    for firefox_dir in &firefox_dirs {
        if !firefox_dir.exists() {
            continue;
        }
        configure_firefox_dir(firefox_dir, cert_path, name)?;
    }

    Ok(())
}

fn configure_firefox_dir(firefox_dir: &Path, _cert_path: &Path, _name: &str) -> Result<()> {
    println!("   Scanning Firefox profiles in {:?}", firefox_dir);

    for entry in std::fs::read_dir(firefox_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let prefs_path = path.join("prefs.js");
            if prefs_path.exists() {
                println!("   Configuring profile: {:?}", path.file_name().unwrap());
                update_firefox_prefs(&prefs_path)?;

                // Import cert directly into Firefox profile NSS DB
                #[cfg(target_os = "linux")]
                if which("certutil") {
                    let db_arg = format!("sql:{}", path.display());
                    // Remove old cert first
                    let _ = Command::new("certutil")
                        .args(["-D", "-d", &db_arg, "-n", name])
                        .output();
                    let status = Command::new("certutil")
                        .args(["-A", "-d", &db_arg, "-t", "C,,", "-n", name, "-i"])
                        .arg(cert_path)
                        .status();
                    match status {
                        Ok(s) if s.success() => {
                            println!("     ✓ Cert imported into Firefox profile");
                        }
                        _ => {
                            eprintln!("     x Failed to import cert into Firefox profile");
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

fn update_firefox_prefs(path: &Path) -> Result<()> {
    let content = fs::read_to_string(path).context("Failed to read prefs.js")?;

    let re =
        Regex::new(r#"(?m)^user_pref\("security\.enterprise_roots\.enabled",\s*.*\);"#).unwrap();
    let new_line = r#"user_pref("security.enterprise_roots.enabled", true);"#;

    let new_content = if re.is_match(&content) {
        re.replace(&content, new_line).to_string()
    } else {
        format!("{}\n{}", content, new_line)
    };

    if content != new_content {
        fs::write(path, new_content).context("Failed to write prefs.js")?;
        println!("     ✓ Enabled security.enterprise_roots.enabled");
    } else {
        println!("     - Already configured");
    }

    Ok(())
}

// =============================================================================
// Utilities
// =============================================================================

fn get_real_home() -> Result<PathBuf> {
    #[cfg(unix)]
    {
        if let Ok(sudo_user) = std::env::var("SUDO_USER") {
            if !sudo_user.is_empty() {
                #[cfg(target_os = "linux")]
                let path = PathBuf::from("/home").join(&sudo_user);

                #[cfg(target_os = "macos")]
                let path = PathBuf::from("/Users").join(&sudo_user);

                if path.exists() {
                    return Ok(path);
                }
            }
        }
    }

    dirs::home_dir().context("Could not determine home directory")
}
