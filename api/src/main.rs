#[macro_use]
extern crate derive_more;

use clap::{Parser, Subcommand};
use dygma_cli::devices::defy::{DefyKeyboard, DefyKeymap, SuperkeyMap};
use dygma_cli::focus_api::{FocusApiConnection, parsing};
use dygma_cli::keycode_tables::{Blank, KeyKind};
use itertools::Itertools;
use std::path::{Path, PathBuf};
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter},
};

/// CLI for programatically configuring and talking with your Dygma keyboard.
///
/// Made with Rust and <3.
#[derive(Parser)]
#[clap(about, author)]
enum Cli {
    /// Commands for talking with your device.
    #[command(subcommand)]
    Command(CommandCommands),
    /// Commands for working with keymaps.
    #[command(subcommand)]
    Keymap(KeymapCommands),
    /// Commands for working with superkeys.
    #[command(subcommand)]
    Superkeys(SuperkeyCommands),
    /// Commands for working with keymap key codes.
    #[command(subcommand)]
    KeyCode(KeyCodeCommands),
}

impl Cli {
    async fn perform(self) -> Result<(), Box<dyn std::error::Error>> {
        match self {
            Self::Command(cmd) => cmd.perform().await,
            Self::Keymap(cmd) => cmd.perform().await,
            Self::Superkeys(cmd) => cmd.perform().await,
            Self::KeyCode(cmd) => cmd.perform(),
        }
    }
}

#[derive(Subcommand)]
enum CommandCommands {
    /// Runs a low-level command on the device.
    ///
    /// You can use the `command list` command to get a list of available
    /// commands.
    ///
    /// # Examples:
    ///
    /// The following command will switch to layer 6:
    ///
    /// ```sh
    /// cargo r -- command run layer.activate --data 5
    /// ```
    Run {
        /// The command to be executed.
        cmd: String,
        /// The data to be submitted along with this command.
        #[arg(short, long)]
        data: Option<String>,
    },
    /// Lists available commands.
    List {
        /// If provided, filters commands using this prefix.
        prefix: Option<String>,
    },
}

impl CommandCommands {
    async fn perform(self) -> Result<(), Box<dyn std::error::Error>> {
        match self {
            Self::Run { cmd, data } => {
                let mut defy = DefyKeyboard::new().await?;

                let available_cmds = defy.available_commands().await?;

                if !available_cmds.contains(&cmd) {
                    let suggestions = get_command_suggestions(&available_cmds, &cmd);

                    eprintln!("`{cmd}` is not a valid command");
                    eprintln!("did you mean one of these? {}", suggestions.join(", "));

                    return Err(RunCommandError.into());
                }

                let res = defy.run_command(&cmd, data.as_deref()).await?;

                println!("{res}");

                Ok(())
            }
            Self::List { prefix } => {
                let mut defy = DefyKeyboard::new().await?;

                defy.available_commands()
                    .await?
                    .into_iter()
                    .filter(|cmd| {
                        if let Some(prefix) = prefix.as_ref() {
                            cmd.starts_with(prefix)
                        } else {
                            true
                        }
                    })
                    .for_each(|cmd| println!("{cmd}"));

                Ok(())
            }
        }
    }
}

#[derive(Subcommand)]
enum KeymapCommands {
    /// Create a new keymap config file.
    New {
        /// The raw keymap string found in the bazecore config file.
        ///
        /// If omitted, will attempt to read it from the keyboard.
        #[clap(short, long)]
        keymap: Option<String>,
        /// The path the keymap will be saved to.
        #[clap(default_value = "keymap.json")]
        path: PathBuf,
    },
    /// Formats the keymap file.
    Format {
        /// The path of the keymap JSON file.
        #[clap(default_value = "keymap.json")]
        path: PathBuf,
    },
    /// Reads a keymap file and outputs it as a raw keymap data string that can
    /// be used to send to the keyboard.
    ToCommandData {
        /// The path of the keymap JSON file.
        path: PathBuf,
    },
    /// Apply the keymap to the keyboard.
    Apply {
        /// The path of the keymap file.
        path: PathBuf,
    },
    /// Clears an entire layer, optionally with the specified key.
    ///
    /// This command does not automatically apply the change to the keyboard,
    /// only to the keymap file.
    ClearLayer {
        /// The path of the keymap file.
        path: PathBuf,
        /// The layer number to clear. Index starts at 1.
        ///
        /// The layer number should match the `layer_number` shown in your keymap JSON
        /// file after running the `keymap format` command.
        #[clap(short, long)]
        layer: u8,
        /// The key to use to clear the layer.
        #[arg(short, long, default_value_t = KeyKind::Blank(Blank::NoKey))]
        key: KeyKind,
    },
}

impl KeymapCommands {
    async fn perform(self) -> Result<(), Box<dyn std::error::Error>> {
        match self {
            Self::New { keymap, path } => {
                let keymap = if let Some(keymap) = keymap {
                    keymap.parse::<DefyKeymap>()?
                } else {
                    let mut defy = DefyKeyboard::new().await?;

                    defy.get_custom_keymap().await?
                };

                safe_pretty_json_file(&keymap, &path).await?;

                Ok(())
            }
            Self::ToCommandData { path } => {
                let keymap = read_json_file::<DefyKeymap>(&path).await?;

                let data = keymap.to_keymap_custom_data()?;

                println!("{data}");

                Ok(())
            }
            Self::Apply { path } => {
                let keymap = read_json_file::<DefyKeymap>(&path).await?;

                let mut defy = DefyKeyboard::new().await?;

                defy.apply_custom_keymap(&keymap).await?;

                // TODO: make this configurable
                // Overwrite the keymap file to ensure file remains prettified
                safe_pretty_json_file(&keymap, &path).await?;

                Ok(())
            }
            Self::Format { path } => {
                let keymap = read_json_file::<DefyKeymap>(&path).await?;

                safe_pretty_json_file(&keymap, &path).await?;

                Ok(())
            }
            Self::ClearLayer { path, layer, key } => {
                let mut keymap = read_json_file::<DefyKeymap>(&path).await?;

                keymap.clear_layer_to(layer as usize, key)?;

                safe_pretty_json_file(&keymap, &path).await?;

                Ok(())
            }
        }
    }
}

#[derive(Subcommand)]
enum SuperkeyCommands {
    /// Create a new keymap config file.
    New {
        /// The raw keymap string found in the bazecore config file.
        ///
        /// If omitted, will attempt to read it from the keyboard.
        #[clap(short, long)]
        superkeys: Option<String>,
        /// The path the keymap will be saved to.
        #[clap(default_value = "keymap.json")]
        path: PathBuf,
    },
    /// Formats the super keys JSON file.
    Format {
        /// The path of the keymap JSON file.
        path: PathBuf,
    },
    /// Reads a keymap file and outputs it as a raw keymap data string that can
    /// be used to send to the keyboard.
    ToCommandData {
        /// The path of the keymap JSON file.
        path: PathBuf,
    },
    /// Apply the keymap to the keyboard.
    Apply {
        /// The path of the keymap file.
        path: PathBuf,
    },
}

impl SuperkeyCommands {
    async fn perform(self) -> Result<(), Box<dyn std::error::Error>> {
        match self {
            Self::New { superkeys, path } => {
                let map = if let Some(superkeys) = superkeys {
                    superkeys.parse::<SuperkeyMap>()?
                } else {
                    let mut defy = DefyKeyboard::new().await?;

                    defy.get_superkeys().await?
                };

                safe_pretty_json_file(&map, &path).await?;

                Ok(())
            }
            Self::ToCommandData { path } => {
                let map = read_json_file::<SuperkeyMap>(&path).await?;

                let str_data = parsing::superkeys::SuperkeyMap::from(&map)
                    .to_command_data::<{ DefyKeyboard::SUPERKEY_MEMORY_SIZE }>()?;

                println!("{str_data}");

                Ok(())
            }
            Self::Apply { path } => {
                let map = read_json_file::<SuperkeyMap>(&path).await?;

                let mut defy = DefyKeyboard::new().await?;

                defy.apply_superkeys(&map).await?;

                // TODO: Make this configurable
                // We override the original config file to make sure everything stays
                // nice and prettified
                safe_pretty_json_file(&map, &path).await?;

                Ok(())
            }
            Self::Format { path } => {
                let map = read_json_file::<SuperkeyMap>(&path).await?;

                safe_pretty_json_file(&map, &path).await?;

                Ok(())
            }
        }
    }
}

#[derive(Subcommand)]
enum KeyCodeCommands {
    /// Get a human-readable name for a raw u16 key code.
    Describe {
        /// The key code you want to get a human-readable name for.
        code: u16,
    },
    /// Get a human-readable description of a string of space-seperated
    /// raw u16 key codes.
    DescribeSequence {
        /// The string of space-seperated u16 key codes.
        keys: String,
    },
    /// Parse a string into a key code.
    ///
    /// # Examples:
    ///
    /// ```sh
    /// cargo r -- key-code parse "A / Ctrl"
    /// ```
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
            Self::Describe { code } => {
                let key = KeyKind::from(code);

                println!("{key}");

                Ok(())
            }
            Self::DescribeSequence { keys } => {
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

/// Utility function for getting possible commands the user might
/// have intended to write, but did not.
fn get_command_suggestions<'a>(available_cmds: &'a [String], user_input: &str) -> Vec<&'a str> {
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

async fn read_json_file<T>(path: &Path) -> Result<T, Box<dyn std::error::Error>>
where
    T: for<'de> serde::Deserialize<'de>,
{
    let file = File::open(path).await?;

    let mut data = vec![];

    BufReader::new(file).read_to_end(&mut data).await?;

    serde_json::from_reader::<_, T>(data.as_slice()).map_err(Into::into)
}

async fn safe_pretty_json_file<T>(data: &T, path: &Path) -> Result<(), Box<dyn std::error::Error>>
where
    T: serde::Serialize,
{
    let file = File::create(path).await?;

    let mut writer = BufWriter::new(file);

    let data = serde_json::to_vec_pretty(data)?;

    writer.write_all(&data).await?;

    writer.flush().await?;

    Ok(())
}
