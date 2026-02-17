use anyhow::{Result, bail};

pub fn hex_to_rgb(s: &str) -> Result<(u8, u8, u8)> {
    let hex = s.strip_prefix('#').unwrap_or(s);

    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        bail!("Invalid hex color: \"{s}\". Expected format: ff00aa or #ff00aa");
    }

    let r = u8::from_str_radix(&hex[0..2], 16)?;
    let g = u8::from_str_radix(&hex[2..4], 16)?;
    let b = u8::from_str_radix(&hex[4..6], 16)?;
    Ok((r, g, b))
}
