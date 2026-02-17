use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

const HOOK_COMMAND: &str = "voyager-disco omarchy match-theme";

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

pub fn install() -> Result<()> {
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
