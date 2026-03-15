mod color;
mod device;
mod omarchy;

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{Shell, generate};
use hidapi::HidApi;
use std::process;

#[derive(Parser)]
#[command(name = "voyager-disco", version, about = "Control ZSA keyboard RGB LEDs")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List all connected ZSA keyboards
    List,
    /// Set LEDs to a color (hex value, e.g. ff00aa or #ff00aa)
    SetColor {
        color: String,
        /// LED key index (0-51). Omit to set all LEDs.
        #[arg(short, long)]
        key: Option<u8>,
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
    /// Generate shell completions
    Completions {
        /// Shell to generate completions for
        shell: Shell,
    },
    /// Control keyboard brightness
    Brightness {
        #[command(subcommand)]
        command: BrightnessCommand,
    },
    /// Omarchy integration commands
    Omarchy {
        #[command(subcommand)]
        command: OmarchyCommand,
    },
}

#[derive(Subcommand)]
enum BrightnessCommand {
    /// Increase brightness by N steps (default 1)
    Up {
        /// Number of steps to increase
        #[arg(default_value = "1")]
        steps: u8,
        /// Target specific devices (comma-separated serial numbers). Defaults to all.
        #[arg(short, long)]
        device: Option<String>,
    },
    /// Decrease brightness by N steps (default 1)
    Down {
        /// Number of steps to decrease
        #[arg(default_value = "1")]
        steps: u8,
        /// Target specific devices (comma-separated serial numbers). Defaults to all.
        #[arg(short, long)]
        device: Option<String>,
    },
    /// Set brightness to an absolute percentage (0-100)
    Set {
        /// Brightness percentage (0-100)
        #[arg(value_parser = clap::value_parser!(u8).range(0..=100))]
        percent: u8,
        /// Target specific devices (comma-separated serial numbers). Defaults to all.
        #[arg(short, long)]
        device: Option<String>,
    },
}

#[derive(Subcommand)]
enum OmarchyCommand {
    /// Install the theme-set hook for automatic LED syncing
    Install,
    /// Set LEDs to the accent color from the current omarchy theme
    MatchTheme {
        /// Target specific devices (comma-separated serial numbers). Defaults to all.
        #[arg(short, long)]
        device: Option<String>,
    },
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::List => {
            let api = HidApi::new().context("Failed to initialize HID API")?;
            device::list(&api);
        }
        Command::SetColor { ref color, key, ref device } => {
            let (r, g, b) = color::hex_to_rgb(color)?;
            let api = HidApi::new().context("Failed to initialize HID API")?;
            let devices = device::open_devices(&api, device)?;
            match key {
                Some(index) => {
                    anyhow::ensure!(index <= 51, "Key index must be 0-51, got {index}");
                    device::set_color_for_key(&devices, index, r, g, b)?;
                }
                None => device::set_color(&devices, r, g, b)?,
            }
        }
        Command::Reset { ref device } => {
            let api = HidApi::new().context("Failed to initialize HID API")?;
            let devices = device::open_devices(&api, device)?;
            device::reset(&devices)?;
        }
        Command::Completions { shell } => {
            generate(shell, &mut Cli::command(), "voyager-disco", &mut std::io::stdout());
        }
        Command::Brightness { command } => {
            let device = match &command {
                BrightnessCommand::Up { device, .. } => device,
                BrightnessCommand::Down { device, .. } => device,
                BrightnessCommand::Set { device, .. } => device,
            };
            let api = HidApi::new().context("Failed to initialize HID API")?;
            let devices = device::open_devices(&api, device)?;
            match command {
                BrightnessCommand::Up { steps, .. } => device::brightness_up(&devices, steps)?,
                BrightnessCommand::Down { steps, .. } => device::brightness_down(&devices, steps)?,
                BrightnessCommand::Set { percent, .. } => device::brightness_set(&devices, percent)?,
            }
        }
        Command::Omarchy { command } => match command {
            OmarchyCommand::Install => {
                omarchy::install()?;
            }
            OmarchyCommand::MatchTheme { ref device } => {
                let (r, g, b) = omarchy::read_theme_accent()?;
                let api = HidApi::new().context("Failed to initialize HID API")?;
                if !device::has_devices(&api) {
                    eprintln!("No ZSA keyboards found, skipping theme sync");
                    return Ok(());
                }
                let devices = device::open_devices(&api, device)?;
                device::set_color(&devices, r, g, b)?;
                eprintln!("(from {})", omarchy::theme_colors_path()?.display());
            }
        },
    }

    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {e:#}");
        process::exit(1);
    }
}
