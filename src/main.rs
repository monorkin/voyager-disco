use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use hidapi::{DeviceInfo, HidApi};
use std::collections::HashMap;
use std::process;

const VENDOR_ID: u16 = 0x3297;
const PRODUCT_ID: u16 = 0x1977;
const USAGE_PAGE: u16 = 0xFF60;
const USAGE_ID: u16 = 0x61;
const PACKET_SIZE: usize = 32;
const THEME_COLORS_PATH: &str =
    "/home/stanko/.config/omarchy/current/theme/colors.toml";

// Oryx protocol command codes
const CMD_PAIRING_INIT: u8 = 0x01;
const CMD_RGB_CONTROL: u8 = 0x05;
const CMD_SET_RGB_LED_ALL: u8 = 0x09;

// Oryx protocol event codes
const EVT_PAIRING_SUCCESS: u8 = 0x04;

#[derive(Parser)]
#[command(name = "voyager-disco", about = "Control ZSA Voyager RGB LEDs")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List all connected Voyager keyboards
    List,
    /// Set all LEDs to a color (hex value, e.g. ff00aa or #ff00aa)
    SetColor {
        color: String,
        /// Target specific devices (comma-separated serial numbers). Defaults to all.
        #[arg(short, long)]
        device: Option<String>,
    },
    /// Set LEDs to the accent color from the current omarchy theme
    MatchTheme {
        /// Target specific devices (comma-separated serial numbers). Defaults to all.
        #[arg(short, long)]
        device: Option<String>,
    },
    /// Reset LEDs to normal keyboard lighting
    Reset {
        /// Target specific devices (comma-separated serial numbers). Defaults to all.
        #[arg(short, long)]
        device: Option<String>,
    },
}

fn is_voyager_raw_hid(d: &DeviceInfo) -> bool {
    d.vendor_id() == VENDOR_ID
        && d.product_id() == PRODUCT_ID
        && d.usage_page() == USAGE_PAGE
        && d.usage() == USAGE_ID
}

fn device_serial(d: &DeviceInfo) -> String {
    d.serial_number().unwrap_or("unknown").to_string()
}

fn device_label(d: &DeviceInfo) -> String {
    let product = d.product_string().unwrap_or("ZSA Voyager");
    let serial = d.serial_number().unwrap_or("unknown");
    format!("{product} [{serial}]")
}

struct Voyager {
    label: String,
    device: hidapi::HidDevice,
}

impl Voyager {
    fn open(api: &HidApi, info: &DeviceInfo) -> Result<Self> {
        let label = device_label(info);
        let device = info
            .open_device(api)
            .with_context(|| format!("Failed to open {label}. Check udev rules on Linux."))?;
        Ok(Self { label, device })
    }

    fn send(&self, data: &[u8]) -> Result<()> {
        let mut packet = [0u8; PACKET_SIZE + 1]; // +1 for report ID
        let len = data.len().min(PACKET_SIZE);
        packet[1..1 + len].copy_from_slice(&data[..len]);
        self.device
            .write(&packet)
            .context("Failed to write to device")?;
        Ok(())
    }

    fn recv(&self, timeout_ms: i32) -> Result<Option<[u8; PACKET_SIZE]>> {
        let mut buf = [0u8; PACKET_SIZE];
        let n = self
            .device
            .read_timeout(&mut buf, timeout_ms)
            .context("Failed to read from device")?;
        if n == 0 { Ok(None) } else { Ok(Some(buf)) }
    }

    fn pair(&self) -> Result<()> {
        self.send(&[CMD_PAIRING_INIT])?;

        let resp = self
            .recv(2000)?
            .context("No response to pairing request")?;

        if resp[0] != EVT_PAIRING_SUCCESS {
            bail!("Pairing failed: response code {:#04x}", resp[0]);
        }

        // Consume the layer event that follows pairing
        self.recv(1000)?;
        Ok(())
    }

    fn enable_rgb_control(&self) -> Result<()> {
        self.send(&[CMD_RGB_CONTROL, 0x01])?;
        self.recv(1000)?;
        Ok(())
    }

    fn disable_rgb_control(&self) -> Result<()> {
        self.send(&[CMD_RGB_CONTROL, 0x00])?;
        self.recv(1000)?;
        Ok(())
    }

    fn set_color_all(&self, r: u8, g: u8, b: u8) -> Result<()> {
        self.send(&[CMD_SET_RGB_LED_ALL, r, g, b])
    }
}

fn parse_device_filter(device: &Option<String>) -> Option<Vec<String>> {
    device.as_ref().map(|d| {
        d.split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    })
}

fn filtered_device_paths<'a>(
    api: &'a HidApi,
    filter: &Option<Vec<String>>,
) -> Result<Vec<&'a DeviceInfo>> {
    let all: Vec<&DeviceInfo> = api.device_list().filter(|d| is_voyager_raw_hid(d)).collect();

    if all.is_empty() {
        bail!(
            "No Voyager keyboards found. \
             Is one connected? Is Keymapp disconnected?"
        );
    }

    let Some(serials) = filter else {
        return Ok(all);
    };

    let mut matched = Vec::new();
    for serial in serials {
        let found = all
            .iter()
            .find(|d| device_serial(d) == *serial)
            .with_context(|| {
                let available: Vec<String> = all.iter().map(|d| device_serial(d)).collect();
                format!(
                    "Device with serial \"{serial}\" not found. Available: {}",
                    available.join(", ")
                )
            })?;
        matched.push(*found);
    }

    Ok(matched)
}

fn open_devices(api: &HidApi, device_flag: &Option<String>) -> Result<Vec<Voyager>> {
    let filter = parse_device_filter(device_flag);
    let infos = filtered_device_paths(api, &filter)?;
    let mut devices = Vec::new();
    for info in infos {
        devices.push(Voyager::open(api, info)?);
    }
    Ok(devices)
}

fn parse_hex_color(s: &str) -> Result<(u8, u8, u8)> {
    let hex = s.strip_prefix('#').unwrap_or(s);

    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        bail!("Invalid hex color: \"{s}\". Expected format: ff00aa or #ff00aa");
    }

    let r = u8::from_str_radix(&hex[0..2], 16)?;
    let g = u8::from_str_radix(&hex[2..4], 16)?;
    let b = u8::from_str_radix(&hex[4..6], 16)?;
    Ok((r, g, b))
}

fn read_theme_accent() -> Result<(u8, u8, u8)> {
    let content = std::fs::read_to_string(THEME_COLORS_PATH)
        .with_context(|| format!("Failed to read {THEME_COLORS_PATH}"))?;

    let colors: HashMap<String, String> =
        toml::from_str(&content).context("Failed to parse colors.toml")?;

    let accent = colors
        .get("accent")
        .context("No 'accent' key found in colors.toml")?;

    parse_hex_color(accent).context("Invalid accent color in colors.toml")
}

fn set_color(devices: &[Voyager], r: u8, g: u8, b: u8) -> Result<()> {
    for voyager in devices {
        voyager
            .pair()
            .with_context(|| format!("Pairing with {}", voyager.label))?;
        voyager
            .enable_rgb_control()
            .with_context(|| format!("Enabling RGB on {}", voyager.label))?;
        voyager
            .set_color_all(r, g, b)
            .with_context(|| format!("Setting color on {}", voyager.label))?;
        eprintln!("{}: set color to #{:02x}{:02x}{:02x}", voyager.label, r, g, b);
    }
    Ok(())
}

fn reset(devices: &[Voyager]) -> Result<()> {
    for voyager in devices {
        voyager
            .pair()
            .with_context(|| format!("Pairing with {}", voyager.label))?;
        voyager
            .disable_rgb_control()
            .with_context(|| format!("Resetting {}", voyager.label))?;
        eprintln!("{}: restored normal lighting", voyager.label);
    }
    Ok(())
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let api = HidApi::new().context("Failed to initialize HID API")?;

    match cli.command {
        Command::List => {
            let devices: Vec<&DeviceInfo> =
                api.device_list().filter(|d| is_voyager_raw_hid(d)).collect();

            if devices.is_empty() {
                eprintln!("No Voyager keyboards found.");
            } else {
                for d in &devices {
                    let serial = d.serial_number().unwrap_or("unknown");
                    let product = d.product_string().unwrap_or("ZSA Voyager");
                    let path = d.path().to_str().unwrap_or("?");
                    println!("{serial}  {product}  {path}");
                }
            }
        }
        Command::SetColor { ref color, ref device } => {
            let (r, g, b) = parse_hex_color(color)?;
            let devices = open_devices(&api, device)?;
            set_color(&devices, r, g, b)?;
        }
        Command::MatchTheme { ref device } => {
            let (r, g, b) = read_theme_accent()?;
            let devices = open_devices(&api, device)?;
            set_color(&devices, r, g, b)?;
            eprintln!("(from {THEME_COLORS_PATH})");
        }
        Command::Reset { ref device } => {
            let devices = open_devices(&api, device)?;
            reset(&devices)?;
        }
    }

    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {e:#}");
        process::exit(1);
    }
}
