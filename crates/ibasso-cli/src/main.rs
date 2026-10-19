use clap::{Parser, Subcommand, ValueEnum};
use ibasso_hid::{Client, Result};
use ibasso_protocol::{Filter, Gain, Output, Setting};
use std::{process::ExitCode, time::Duration};

#[derive(Debug, Parser)]
#[command(version, about = "Control an iBasso DC04 Pro over HID")]
struct Args {
    /// Exact HID path from `list`; required when multiple devices are connected.
    #[arg(long, global = true)]
    device: Option<String>,
    /// Maximum response wait per transaction, in milliseconds.
    #[arg(long, default_value_t = 1000, global = true, value_parser = clap::value_parser!(u64).range(1..=30000))]
    timeout_ms: u64,
    /// Print raw TX/RX reports to stderr.
    #[arg(long, global = true)]
    trace: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// List connected DC04 Pro HID interfaces without changing settings.
    List,
    /// Read firmware, volume mirrors, filter, gain, output mode and ECO.
    Status,
    /// Change one setting, then verify every written register.
    Set {
        /// Print the write/readback plan without opening a device.
        #[arg(long, global = true)]
        dry_run: bool,
        #[command(subcommand)]
        setting: SetCommand,
    },
}

#[derive(Debug, Subcommand)]
enum SetCommand {
    /// Set hardware volume (0=mute, 100=maximum); balance defaults to centered.
    Volume {
        #[arg(value_parser = clap::value_parser!(u8).range(0..=100))]
        level: u8,
        /// APK balance -10..10; negative lowers left raw attenuation, positive lowers right.
        #[arg(long, default_value_t = 0, allow_hyphen_values = true, value_parser = clap::value_parser!(i8).range(-10..=10))]
        balance: i8,
    },
    /// Select D1..D5 according to the official App's click mapping.
    Filter {
        #[arg(value_enum)]
        value: FilterArg,
    },
    Gain {
        #[arg(value_enum)]
        value: GainArg,
    },
    Output {
        #[arg(value_enum)]
        value: OutputArg,
    },
    Eco {
        #[arg(value_enum)]
        value: Toggle,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum FilterArg {
    D1,
    D2,
    D3,
    D4,
    D5,
}
#[derive(Debug, Clone, Copy, ValueEnum)]
enum GainArg {
    Low,
    Medium,
    High,
}
#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputArg {
    Normal,
    Turbo,
}
#[derive(Debug, Clone, Copy, ValueEnum)]
enum Toggle {
    On,
    Off,
}

impl SetCommand {
    fn setting(&self) -> Setting {
        match *self {
            Self::Volume { level, balance } => Setting::Volume { level, balance },
            Self::Filter { value } => Setting::Filter(match value {
                FilterArg::D1 => Filter::D1,
                FilterArg::D2 => Filter::D2,
                FilterArg::D3 => Filter::D3,
                FilterArg::D4 => Filter::D4,
                FilterArg::D5 => Filter::D5,
            }),
            Self::Gain { value } => Setting::Gain(match value {
                GainArg::Low => Gain::Low,
                GainArg::Medium => Gain::Medium,
                GainArg::High => Gain::High,
            }),
            Self::Output { value } => Setting::Output(match value {
                OutputArg::Normal => Output::Normal,
                OutputArg::Turbo => Output::Turbo,
            }),
            Self::Eco { value } => Setting::Eco(matches!(value, Toggle::On)),
        }
    }
}

fn run(args: Args) -> Result<()> {
    match args.command {
        Command::List => {
            let devices = ibasso_hid::devices()?;
            if devices.is_empty() {
                println!("No DC04 Pro HID devices found.");
            }
            for device in devices {
                println!(
                    "{:04x}:{:04x} {}\n  interface={} usage={:04x}:{:04x}\n  path={}",
                    device.vendor_id(),
                    device.product_id(),
                    device.product_string().unwrap_or("unknown"),
                    device.interface_number(),
                    device.usage_page(),
                    device.usage(),
                    device.path().to_string_lossy()
                );
            }
        }
        Command::Status => {
            let mut client = Client::open(
                args.device.as_deref(),
                Duration::from_millis(args.timeout_ms),
                args.trace,
            )?;
            println!("{}", client.status()?);
        }
        Command::Set { setting, dry_run } => {
            let setting = setting.setting();
            let plan = setting.plan()?;
            if dry_run {
                println!(
                    "{setting:?}; no device opened, {} writes planned",
                    plan.len()
                );
                for (index, step) in plan.iter().enumerate() {
                    println!("{:02}. write {:02x?}", index + 1, step.write.0);
                    if let Some(preamble) = step.readback.read_preamble() {
                        println!("    readback MAP preamble {:02x?}", preamble.0);
                    }
                    println!(
                        "    read  {:02x?} -> 0x{:02x}",
                        step.readback.0, step.expected
                    );
                }
            } else {
                let mut client = Client::open(
                    args.device.as_deref(),
                    Duration::from_millis(args.timeout_ms),
                    args.trace,
                )?;
                client.apply(setting)?;
                println!(
                    "Applied {setting:?}; all {} register readbacks verified.",
                    plan.len()
                );
            }
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    match run(Args::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cli_definition_is_consistent() {
        use clap::CommandFactory;
        Args::command().debug_assert();
    }
    #[test]
    fn rejects_bad_values_before_device_io() {
        for arguments in [
            vec!["cli", "set", "volume", "101"],
            vec!["cli", "set", "volume", "-1"],
            vec!["cli", "set", "volume", "20", "--balance", "-11"],
            vec!["cli", "set", "gain", "maximum"],
            vec!["cli", "set", "filter", "d6"],
            vec!["cli", "status", "--timeout-ms", "0"],
            vec!["cli", "status", "--timeout-ms", "30001"],
        ] {
            assert!(Args::try_parse_from(arguments).is_err());
        }
    }
    #[test]
    fn accepts_negative_balance_and_offline_plan() {
        let args = Args::try_parse_from([
            "cli",
            "set",
            "volume",
            "20",
            "--balance",
            "-10",
            "--dry-run",
        ])
        .unwrap();
        assert!(matches!(
            args.command,
            Command::Set {
                dry_run: true,
                setting: SetCommand::Volume {
                    level: 20,
                    balance: -10
                }
            }
        ));
    }
}
