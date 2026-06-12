use anyhow::{Context, Result};
use hidapi::HidApi;
use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

const HOOK_COMMAND: &str = "voyager-disco omarchy match-theme";

#[cfg(target_os = "linux")]
const SERVICE_NAME: &str = "voyager-disco-watch.service";

fn hook_path() -> Result<PathBuf> {
    let home = std::env::var("HOME").context("HOME environment variable not set")?;
    Ok(PathBuf::from(home).join(".config/omarchy/hooks/theme-set"))
}

pub fn theme_colors_path() -> Result<PathBuf> {
    let home = std::env::var("HOME").context("HOME environment variable not set")?;
    Ok(PathBuf::from(home).join(".config/omarchy/current/theme/colors.toml"))
}

pub fn read_theme_accent() -> Result<(u8, u8, u8)> {
    let path = theme_colors_path()?;
    let content = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read {}", path.display()))?;

    let colors: HashMap<String, String> =
        toml::from_str(&content).context("Failed to parse colors.toml")?;

    let accent = colors
        .get("accent")
        .context("No 'accent' key found in colors.toml")?;

    crate::color::hex_to_rgb(accent).context("Invalid accent color in colors.toml")
}

pub fn match_theme(device_filter: &Option<String>) -> Result<()> {
    let (r, g, b) = read_theme_accent()?;
    let api = HidApi::new().context("Failed to initialize HID API")?;
    let devices = crate::device::open_devices(&api, device_filter)?;
    crate::device::set_color(&devices, r, g, b)?;
    eprintln!("(from {})", theme_colors_path()?.display());
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn watch(device_filter: &Option<String>) -> Result<()> {
    use std::{thread, time::Duration};

    let socket = udev::MonitorBuilder::new()
        .context("Failed to create udev monitor")?
        .match_subsystem("hidraw")
        .context("Failed to filter udev monitor to hidraw devices")?
        .listen()
        .context("Failed to listen for udev events")?;

    eprintln!("Watching for ZSA keyboard connections...");

    // Sync keyboards that are already connected when the watcher starts
    sync_theme(device_filter, 1);

    loop {
        let Some(event) = socket.iter().next() else {
            thread::sleep(Duration::from_millis(500));
            continue;
        };

        if event.event_type() != udev::EventType::Add || !is_zsa_device(&event.device()) {
            continue;
        }

        // A keyboard exposes several HID interfaces, each producing its own
        // add event; wait for enumeration to settle, then drain the rest.
        thread::sleep(Duration::from_secs(1));
        while socket.iter().next().is_some() {}

        sync_theme(device_filter, 5);
    }
}

#[cfg(target_os = "linux")]
fn is_zsa_device(device: &udev::Device) -> bool {
    device
        .parent_with_subsystem_devtype("usb", "usb_device")
        .ok()
        .flatten()
        .and_then(|usb| usb.attribute_value("idVendor").map(|v| v.to_os_string()))
        .is_some_and(|vid| vid.eq_ignore_ascii_case("3297"))
}

#[cfg(target_os = "linux")]
fn sync_theme(device_filter: &Option<String>, attempts: u32) {
    use std::{thread, time::Duration};

    for attempt in 1..=attempts {
        if attempt > 1 {
            thread::sleep(Duration::from_secs(1));
        }
        match match_theme(device_filter) {
            Ok(()) => return,
            Err(e) => eprintln!(
                "voyager-disco: theme sync attempt {attempt}/{attempts} failed: {e:#}"
            ),
        }
    }
}

pub fn install() -> Result<()> {
    install_hook()?;

    #[cfg(target_os = "linux")]
    install_watch_service()?;

    Ok(())
}

fn install_hook() -> Result<()> {
    let hook_path = hook_path()?;

    if hook_path.exists() {
        let content = fs::read_to_string(&hook_path)
            .with_context(|| format!("Failed to read {}", hook_path.display()))?;

        if content.lines().any(|line| line.trim() == HOOK_COMMAND) {
            eprintln!("Hook already installed in {}", hook_path.display());
            return Ok(());
        }

        let new_content = if content.ends_with('\n') {
            format!("{content}{HOOK_COMMAND}\n")
        } else {
            format!("{content}\n{HOOK_COMMAND}\n")
        };

        fs::write(&hook_path, new_content)
            .with_context(|| format!("Failed to write {}", hook_path.display()))?;
    } else {
        if let Some(parent) = hook_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create {}", parent.display()))?;
        }
        fs::write(&hook_path, format!("{HOOK_COMMAND}\n"))
            .with_context(|| format!("Failed to write {}", hook_path.display()))?;
    }

    let mut perms = fs::metadata(&hook_path)?.permissions();
    perms.set_mode(perms.mode() | 0o111);
    fs::set_permissions(&hook_path, perms)?;

    eprintln!("Installed hook in {}", hook_path.display());
    Ok(())
}

#[cfg(target_os = "linux")]
fn service_path() -> Result<PathBuf> {
    let home = std::env::var("HOME").context("HOME environment variable not set")?;
    Ok(PathBuf::from(home)
        .join(".config/systemd/user")
        .join(SERVICE_NAME))
}

#[cfg(target_os = "linux")]
fn install_watch_service() -> Result<()> {
    use std::process::Command;

    let exe = std::env::current_exe().context("Failed to determine voyager-disco path")?;
    let unit = format!(
        "[Unit]\n\
         Description=Sync ZSA keyboard LEDs with the Omarchy theme on connect\n\
         \n\
         [Service]\n\
         ExecStart={} omarchy watch\n\
         Restart=on-failure\n\
         RestartSec=2\n\
         \n\
         [Install]\n\
         WantedBy=default.target\n",
        exe.display()
    );

    let service_path = service_path()?;
    if let Some(parent) = service_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }
    fs::write(&service_path, unit)
        .with_context(|| format!("Failed to write {}", service_path.display()))?;

    eprintln!("Installed service in {}", service_path.display());

    let systemctl = |args: &[&str]| {
        Command::new("systemctl")
            .args(args)
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    };

    let enabled = systemctl(&["--user", "daemon-reload"])
        && systemctl(&["--user", "enable", SERVICE_NAME])
        && systemctl(&["--user", "restart", SERVICE_NAME]);

    if enabled {
        eprintln!("Enabled and started {SERVICE_NAME}");
    } else {
        eprintln!(
            "Could not enable {SERVICE_NAME} automatically. \
             Run: systemctl --user enable --now {SERVICE_NAME}"
        );
    }

    Ok(())
}
