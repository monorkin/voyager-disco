mod color;
mod device;
mod omarchy;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
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
    /// Set all LEDs to a color (hex value, e.g. ff00aa or #ff00aa)
    SetColor {
        color: String,
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
    /// Omarchy integration commands
    Omarchy {
        #[command(subcommand)]
        command: OmarchyCommand,
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
        Command::SetColor { ref color, ref device } => {
            let (r, g, b) = color::hex_to_rgb(color)?;
            let api = HidApi::new().context("Failed to initialize HID API")?;
            let devices = device::open_devices(&api, device)?;
            device::set_color(&devices, r, g, b)?;
        }
        Command::Reset { ref device } => {
            let api = HidApi::new().context("Failed to initialize HID API")?;
            let devices = device::open_devices(&api, device)?;
            device::reset(&devices)?;
        }
        Command::Omarchy { command } => match command {
            OmarchyCommand::Install => {
                omarchy::install()?;
            }
            OmarchyCommand::MatchTheme { ref device } => {
                let (r, g, b) = omarchy::read_theme_accent()?;
                let api = HidApi::new().context("Failed to initialize HID API")?;
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
