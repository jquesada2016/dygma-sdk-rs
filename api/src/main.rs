#[macro_use]
extern crate derive_more;

use clap::{Args, Parser, Subcommand};
use dygma_cli::focus_api::FocusApi;
use error_stack::{ResultExt, report};
use itertools::Itertools;
use std::path::PathBuf;

const PRODUCT_NAME: &str = "DEFY";
const BAUD_RATE: u32 = 115_200;

#[derive(Parser)]
enum Cli {
    /// Allows executing low level commands on Dygma hardware.
    RunCommand(RunCommandArgs),
    #[command(subcommand)]
    /// Useful commands for working with Bazecore JSON config files.
    Config(ConfigCommands),
    /// Useful commands for working with raw string config values.
    #[clap(subcommand)]
    Raw(RawCommands),
}

impl Cli {
    async fn perform(self) -> Result<(), Box<dyn std::error::Error>> {
        match self {
            Self::RunCommand(args) => run_command(args).await?,
            Self::Config(cmd) => cmd.perform().await?,
            Cli::Raw(cmd) => cmd.perform().await?,
        }

        Ok(())
    }
}

#[derive(Args)]
struct RunCommandArgs {
    /// The command to be executed.
    #[arg(short, long = "command")]
    cmd: String,
    /// The data to be submitted along with this command.
    #[arg(short, long)]
    data: Option<String>,
    /// The manufacturer name used to identify compatible serial ports.

    /// The product name used to identify compatible serial ports.
    #[arg(short, long, default_value = PRODUCT_NAME)]
    product_name: String,
    /// The baud rate to use for communication with the device.
    #[arg(short, long, default_value_t = BAUD_RATE)]
    baud_rate: u32,
}

#[derive(Subcommand)]
enum ConfigCommands {
    /// Reads the Bazecore config file and outputs a human-readable
    /// keymap file.
    Keymap {
        /// The path to the JSON config file.
        path: PathBuf,
    },
}

impl ConfigCommands {
    async fn perform(self) -> Result<(), Box<dyn std::error::Error>> {
        match self {
            ConfigCommands::Keymap { path } => todo!(),
        }
    }
}

#[derive(Subcommand)]
enum RawCommands {
    /// Takes a raw keymap string and returns an optionally human-readable
    /// JSON version of the keymap.
    Keymap {
        /// Raw keymap data.
        data: String,
        /// Whether or not to render the output in human-readable format.
        #[clap(short = 'H', long, default_value_t = true)]
        human_readable: bool,
    },
    /// Get a human-readable name for the key code.
    KeyCode {
        /// The key code you want to get a human-readable name for.
        code: u16,
    },
    /// Get a human-readable description of a string of key codes.
    KeyCodeString {
        /// The string of space-seperated key codes.
        keys: String,
    },
}

impl RawCommands {
    async fn perform(self) -> Result<(), Box<dyn std::error::Error>> {
        match self {
            RawCommands::Keymap {
                data,
                human_readable,
            } => {
                use dygma_cli::devices::defy::DefyKeymap;

                let keymap = data.parse::<DefyKeymap>()?;

                println!("{keymap:#?}");

                Ok(())
            }
            RawCommands::KeyCode { code } => {
                use dygma_cli::parsing::keymap::KeyKind;

                let key = KeyKind::from(code);

                println!("{key}");

                Ok(())
            }

            RawCommands::KeyCodeString { keys } => {
                use dygma_cli::parsing::keymap::KeyKind;

                let keys = keys
                    .split(' ')
                    .filter(|seq| !seq.is_empty())
                    .map(|seq| seq.parse::<u16>().unwrap())
                    .map(KeyKind::from)
                    .map(|key| key.to_string())
                    .join(" ");

                println!("{keys}");

                Ok(())
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    Cli::parse().perform().await?;

    Ok(())
}

#[derive(Clone, Copy, Debug, Display, Error)]
#[display("failed to run command")]
struct RunCommandError;

async fn run_command(args: RunCommandArgs) -> error_stack::Result<(), RunCommandError> {
    let RunCommandArgs {
        cmd,
        data,
        product_name,
        baud_rate,
    } = args;

    let mut focus_api = FocusApi::new(&product_name, baud_rate)
        .await
        .change_context(RunCommandError)?;

    let available_cmds = focus_api
        .available_commands()
        .await
        .change_context(RunCommandError)?;

    if !available_cmds.contains(&cmd) {
        let suggestions = get_command_suggestions(&available_cmds, &cmd);

        let report = report!(RunCommandError)
            .attach_printable(format!("`{cmd}` is not a valid command"))
            .attach_printable(format!(
                "did you mean one of these? {}",
                suggestions.join(", ")
            ));

        return Err(report);
    }

    let res = focus_api
        .run_command(&cmd, data.as_deref())
        .await
        .change_context(RunCommandError)?;

    println!("{res}");

    Ok(())
}

/// Utility function for getting possible commands the user might
/// have intended to write, but did not.
pub fn get_command_suggestions<'a>(available_cmds: &'a [String], user_input: &str) -> Vec<&'a str> {
    use strsim::jaro_winkler;

    let mut scored_suggestions = available_cmds
        .iter()
        .map(|cmd| {
            (
                (jaro_winkler(cmd, user_input) * 10_000.0) as u64,
                cmd.as_str(),
            )
        })
        .collect::<Vec<_>>();

    scored_suggestions.sort_unstable_by_key(|(score, _)| *score);

    scored_suggestions
        .into_iter()
        .rev()
        .take(5)
        .map(|(_, cmd)| cmd)
        .collect::<Vec<_>>()
}
