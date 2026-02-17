use anyhow::{Context, Result, bail};
use hidapi::{DeviceInfo, HidApi};

const VENDOR_ID: u16 = 0x3297;
const USAGE_PAGE: u16 = 0xFF60;
const USAGE_ID: u16 = 0x61;
const PACKET_SIZE: usize = 32;

// Oryx protocol command codes
const CMD_PAIRING_INIT: u8 = 0x01;
const CMD_RGB_CONTROL: u8 = 0x05;
const CMD_SET_RGB_LED_ALL: u8 = 0x09;
const CMD_GET_PROTOCOL_VERSION: u8 = 0xFE;

// Oryx protocol event codes
const EVT_PAIRING_SUCCESS: u8 = 0x04;
const EVT_GET_PROTOCOL_VERSION: u8 = 0xFE;

const EXPECTED_PROTOCOL_VERSION: u8 = 0x04;

fn is_oryx_raw_hid(d: &DeviceInfo) -> bool {
    d.vendor_id() == VENDOR_ID
        && d.usage_page() == USAGE_PAGE
        && d.usage() == USAGE_ID
}

fn device_serial(d: &DeviceInfo) -> String {
    d.serial_number().unwrap_or("unknown").to_string()
}

fn device_label(d: &DeviceInfo) -> String {
    let product = d.product_string().unwrap_or("ZSA Keyboard");
    let serial = d.serial_number().unwrap_or("unknown");
    format!("{product} [{serial}]")
}

pub struct Device {
    label: String,
    device: hidapi::HidDevice,
}

impl Device {
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

        self.check_protocol_version()?;

        Ok(())
    }

    fn check_protocol_version(&self) -> Result<()> {
        self.send(&[CMD_GET_PROTOCOL_VERSION])?;

        let resp = self
            .recv(2000)?
            .context("No response to protocol version request")?;

        if resp[0] != EVT_GET_PROTOCOL_VERSION {
            bail!(
                "Unexpected response to protocol version request: {:#04x}",
                resp[0]
            );
        }

        let version = resp[1];
        if version != EXPECTED_PROTOCOL_VERSION {
            bail!(
                "Unsupported Oryx protocol version {:#04x} (expected {:#04x})",
                version,
                EXPECTED_PROTOCOL_VERSION
            );
        }

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
    let all: Vec<&DeviceInfo> = api.device_list().filter(|d| is_oryx_raw_hid(d)).collect();

    if all.is_empty() {
        bail!(
            "No ZSA keyboards found. \
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

pub fn open_devices(api: &HidApi, device_flag: &Option<String>) -> Result<Vec<Device>> {
    let filter = parse_device_filter(device_flag);
    let infos = filtered_device_paths(api, &filter)?;
    let mut devices = Vec::new();
    for info in infos {
        devices.push(Device::open(api, info)?);
    }
    Ok(devices)
}

pub fn list(api: &HidApi) {
    let devices: Vec<&DeviceInfo> =
        api.device_list().filter(|d| is_oryx_raw_hid(d)).collect();

    if devices.is_empty() {
        eprintln!("No ZSA keyboards found.");
    } else {
        for d in &devices {
            let serial = d.serial_number().unwrap_or("unknown");
            let product = d.product_string().unwrap_or("ZSA Keyboard");
            let path = d.path().to_str().unwrap_or("?");
            println!("{serial}  {product}  {path}");
        }
    }
}

pub fn set_color(devices: &[Device], r: u8, g: u8, b: u8) -> Result<()> {
    for device in devices {
        device
            .pair()
            .with_context(|| format!("Pairing with {}", device.label))?;
        device
            .enable_rgb_control()
            .with_context(|| format!("Enabling RGB on {}", device.label))?;
        device
            .set_color_all(r, g, b)
            .with_context(|| format!("Setting color on {}", device.label))?;
        eprintln!("{}: set color to #{:02x}{:02x}{:02x}", device.label, r, g, b);
    }
    Ok(())
}

pub fn reset(devices: &[Device]) -> Result<()> {
    for device in devices {
        device
            .pair()
            .with_context(|| format!("Pairing with {}", device.label))?;
        device
            .disable_rgb_control()
            .with_context(|| format!("Resetting {}", device.label))?;
        eprintln!("{}: restored normal lighting", device.label);
    }
    Ok(())
}
