#[macro_use]
extern crate derive_more;

use std::path::{Path, PathBuf};

use clap::{Args, Parser, Subcommand};
use dygma_cli::devices::defy::{DefyKeyboard, DefyKeymap};
use dygma_cli::focus_api::FocusApi;
use dygma_cli::parsing::keymap::KeyKind;
use error_stack::{ResultExt, report};
use itertools::Itertools;
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter},
};

const PRODUCT_NAME: &str = "DEFY";
const BAUD_RATE: u32 = 115_200;

#[derive(Parser)]
enum Cli {
    /// Allows executing low level commands on Dygma hardware.
    RunCommand(RunCommandArgs),
    /// Useful commands for working with keymaps.
    #[command(subcommand)]
    Keymap(KeymapCommands),
    /// Useful commands for working with keymap key codes.
    #[command(subcommand)]
    KeyCodes(KeyCodeCommands),
}

impl Cli {
    async fn perform(self) -> Result<(), Box<dyn std::error::Error>> {
        match self {
            Self::RunCommand(args) => run_command(args).await.map_err(Into::into),
            Self::Keymap(cmd) => cmd.perform().await,
            Cli::KeyCodes(cmd) => cmd.perform(),
        }
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
    /// The product name used to identify compatible serial ports.
    #[arg(short, long, default_value = PRODUCT_NAME)]
    product_name: String,
    /// The baud rate to use for communication with the device.
    #[arg(short, long, default_value_t = BAUD_RATE)]
    baud_rate: u32,
}

#[derive(Subcommand)]
enum KeymapCommands {
    /// Create a new keymap config file.
    New {
        /// The raw keymap string found in the bazecore config file.
        #[clap(short, long)]
        keymap: String,
        /// The path the keymap will be saved to.
        #[clap(default_value = "keymap.json")]
        path: PathBuf,
    },
    /// Reads a keymap file and outputs it as a raw keymap data string that can
    /// be used to send to the keyboard.
    ToCommandData {
        /// The path of the keymap file.
        path: PathBuf,
    },
    /// Apply the keymap to the keyboard.
    Apply {
        /// The path of the keymap file.
        path: PathBuf,
    },
}

impl KeymapCommands {
    async fn perform(self) -> Result<(), Box<dyn std::error::Error>> {
        match self {
            Self::New { keymap, path } => {
                let keymap = keymap.parse::<DefyKeymap>()?;

                save_keymap_file(keymap, &path).await?;

                Ok(())
            }
            Self::ToCommandData { path } => {
                let keymap = read_keymap_file(&path).await?;

                let res = keymap
                    .to_keymap_custom_data()?
                    .into_iter()
                    .map(|key| key.unwrap_or_default())
                    .join(" ");

                println!("{res}");

                Ok(())
            }
            Self::Apply { path } => {
                let keymap = read_keymap_file(&path).await?;

                let data = keymap
                    .to_keymap_custom_data()?
                    .into_iter()
                    .map(|key| key.unwrap_or_default())
                    .join(" ");

                let mut defy = DefyKeyboard::new().await?;

                defy.run_command("keymap.custom", Some(&data)).await?;

                // TODO: make this configurable
                // Overwrite the keymap file to ensure file remains prettified
                save_keymap_file(keymap, &path).await?;

                Ok(())
            }
        }
    }
}

#[derive(Subcommand)]
enum KeyCodeCommands {
    /// Get a human-readable name for the key code.
    DescribeKeyCode {
        /// The key code you want to get a human-readable name for.
        code: u16,
    },
    /// Get a human-readable description of a string of key codes.
    DescribeKeyCodeSequence {
        /// The string of space-seperated key codes.
        keys: String,
    },
    /// Parse a string into a key code.
    Parse {
        /// The string representing the key.
        data: String,
        /// If true, the raw u16 key code will be returned, otherwise, a parsable
        /// key ID will be returned.
        #[clap(short, long)]
        raw: bool,
    },
}

impl KeyCodeCommands {
    fn perform(self) -> Result<(), Box<dyn std::error::Error>> {
        match self {
            Self::DescribeKeyCode { code } => {
                let key = KeyKind::from(code);

                println!("{key}");

                Ok(())
            }
            Self::DescribeKeyCodeSequence { keys } => {
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
            Self::Parse { data, raw } => {
                let Ok(key) = data.parse::<KeyKind>() else {
                    println!("Could not recognize the key.");

                    return Ok(());
                };

                if raw {
                    let code: u16 = key.into();

                    println!("{code}");
                } else {
                    println!("{key:?}");
                }

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

async fn read_keymap_file(path: &Path) -> Result<DefyKeymap, Box<dyn std::error::Error>> {
    let file = File::open(path).await?;

    let mut data = vec![];

    BufReader::new(file).read_to_end(&mut data).await?;

    serde_json::from_reader::<_, DefyKeymap>(data.as_slice()).map_err(Into::into)
}

async fn save_keymap_file(
    keymap: DefyKeymap,
    path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let file = File::create(path).await?;

    let mut writer = BufWriter::new(file);

    let data = serde_json::to_vec_pretty(&keymap)?;

    writer.write_all(&data).await?;

    writer.flush().await?;

    Ok(())
}
