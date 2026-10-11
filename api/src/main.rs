#[macro_use]
extern crate derive_more;

use clap::{Parser, Subcommand, ValueEnum};
use dygma_cli::devices::defy::{DefyKeyboard, DefyKeymap, MacroMap, SuperkeyMap};
use dygma_cli::focus_api::parsing::backup::DefyBackup;
use dygma_cli::focus_api::{FocusApiConnection, parsing};
use dygma_cli::keycode_tables::{Blank, KeyKind};
use error_stack::{IntoReport, ResultExt};
use itertools::Itertools;
use std::path::{Path, PathBuf};
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter},
};

#[derive(Clone, Copy, Debug, Display, Error)]
#[display("something went wrong running the command")]
struct Error;

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
    /// Commands for working with macros.
    #[command(subcommand)]
    Macros(MacroCommands),
    /// Commands for working with device settings and identity.
    #[command(subcommand)]
    Settings(SettingsCommands),
    /// Commands for working with device backups.
    #[command(subcommand)]
    Backup(BackupCommands),
    /// Commands for working with keymap key codes.
    #[command(subcommand)]
    KeyCode(KeyCodeCommands),
}

impl Cli {
    async fn perform(self) -> Result<(), error_stack::Report<Error>> {
        match self {
            Self::Command(cmd) => cmd.perform().await,
            Self::Keymap(cmd) => cmd.perform().await,
            Self::Superkeys(cmd) => cmd.perform().await,
            Self::Macros(cmd) => cmd.perform().await,
            Self::Settings(cmd) => cmd.perform().await,
            Self::Backup(cmd) => cmd.perform().await,
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
        /// If provided, filters commands that contain this term.
        term: Option<String>,
    },
}

impl CommandCommands {
    async fn perform(self) -> Result<(), error_stack::Report<Error>> {
        match self {
            Self::Run { cmd, data } => {
                let mut defy = DefyKeyboard::new()
                    .await
                    .change_context(Error)
                    .attach("connecting to the Defy keyboard")?;

                let available_cmds = defy
                    .available_commands()
                    .await
                    .change_context(Error)
                    .attach("getting list of available commands")?;

                if !available_cmds.contains(&cmd) {
                    let suggestions = get_command_suggestions(&available_cmds, &cmd)
                        .into_iter()
                        .map(ToOwned::to_owned)
                        .collect();

                    return Err(RunCommandError { cmd, suggestions }
                        .into_report()
                        .change_context(Error));
                }

                let res = defy
                    .run_command(&cmd, data.as_deref())
                    .await
                    .change_context(Error)
                    .attach_with(|| format!("running `{cmd}` command"))?;

                println!("{res}");

                Ok(())
            }
            Self::List { term } => {
                let mut defy = DefyKeyboard::new()
                    .await
                    .change_context(Error)
                    .attach("connecting to the Defy keyboard")?;

                defy.available_commands()
                    .await
                    .change_context(Error)
                    .attach("getting list of available commands")?
                    .into_iter()
                    .filter(|cmd| {
                        if let Some(term) = term.as_ref() {
                            cmd.contains(term)
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
        /// Do not rewrite the keymap file after applying.
        ///
        /// By default, the file is rewritten canonically after a
        /// successful apply, keeping it prettified and its layer numbers
        /// up to date. This flag leaves your file untouched.
        #[clap(long, default_value_t = false)]
        no_write: bool,
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
    async fn perform(self) -> Result<(), error_stack::Report<Error>> {
        match self {
            Self::New { keymap, path } => {
                let keymap = if let Some(keymap) = keymap {
                    keymap
                        .parse::<DefyKeymap>()
                        .change_context(Error)
                        .attach("parsing keymap JSON file")?
                } else {
                    let mut defy = DefyKeyboard::new()
                        .await
                        .change_context(Error)
                        .attach("connecting to the Defy keyboard")?;

                    defy.get_custom_keymap()
                        .await
                        .change_context(Error)
                        .attach("getting the custom keymap from the Defy")?
                };

                safe_pretty_json_file(&keymap, &path).await?;

                Ok(())
            }
            Self::ToCommandData { path } => {
                let keymap = read_json_file::<DefyKeymap>(&path).await?;

                let data = keymap
                    .to_keymap_custom_data()
                    .change_context(Error)
                    .attach("serializing keymap into command data")?;

                println!("{data}");

                Ok(())
            }
            Self::Apply { path, no_write } => {
                let keymap = read_json_file::<DefyKeymap>(&path).await?;

                let mut defy = DefyKeyboard::new()
                    .await
                    .change_context(Error)
                    .attach("connecting to the Defy keyboard")?;

                defy.apply_custom_keymap(&keymap)
                    .await
                    .change_context(Error)
                    .attach("applying the keymap to the Defy")?;

                if !no_write {
                    // Overwrite the keymap file to ensure file remains prettified
                    safe_pretty_json_file(&keymap, &path).await?;
                }

                Ok(())
            }
            Self::Format { path } => {
                let keymap = read_json_file::<DefyKeymap>(&path).await?;

                safe_pretty_json_file(&keymap, &path).await?;

                Ok(())
            }
            Self::ClearLayer { path, layer, key } => {
                let mut keymap = read_json_file::<DefyKeymap>(&path).await?;

                keymap
                    .clear_layer_to(layer as usize, key)
                    .change_context(Error)
                    .attach_with(|| format!("clearing the `{layer}` layer to key `{key}`"))?;

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
    /// Apply the superkeys to the keyboard.
    Apply {
        /// The path of the superkeys file.
        path: PathBuf,
        /// Do not rewrite the superkeys file after applying.
        ///
        /// By default, the file is rewritten canonically after a
        /// successful apply, keeping it prettified and its superkey
        /// numbers up to date. This flag leaves your file untouched.
        #[clap(long, default_value_t = false)]
        no_write: bool,
    },
}

impl SuperkeyCommands {
    async fn perform(self) -> Result<(), error_stack::Report<Error>> {
        match self {
            Self::New { superkeys, path } => {
                let map = if let Some(superkeys) = superkeys {
                    superkeys
                        .parse::<SuperkeyMap>()
                        .change_context(Error)
                        .attach("parsing superkeys JSON file")?
                } else {
                    let mut defy = DefyKeyboard::new()
                        .await
                        .change_context(Error)
                        .attach("connecting to the Defy keyboard")?;

                    defy.get_superkeys()
                        .await
                        .change_context(Error)
                        .attach("getting superkeys from the Defy")?
                };

                safe_pretty_json_file(&map, &path).await?;

                Ok(())
            }
            Self::ToCommandData { path } => {
                let map = read_json_file::<SuperkeyMap>(&path).await?;

                let str_data = parsing::superkeys::SuperkeyMap::from(&map)
                    .to_command_data::<{ DefyKeyboard::SUPERKEY_MEMORY_SIZE }>()
                    .change_context(Error)
                    .attach("serializing superkeys to command data")?;

                println!("{str_data}");

                Ok(())
            }
            Self::Apply { path, no_write } => {
                let map = read_json_file::<SuperkeyMap>(&path).await?;

                let mut defy = DefyKeyboard::new()
                    .await
                    .change_context(Error)
                    .attach("connecting to the Defy keyboard")?;

                defy.apply_superkeys(&map)
                    .await
                    .change_context(Error)
                    .attach("applying superkeys to the Defy")?;

                if !no_write {
                    // We override the original config file to make sure everything
                    // stays nice and prettified
                    safe_pretty_json_file(&map, &path).await?;
                }

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
enum MacroCommands {
    /// Create a new macros config file.
    New {
        /// The raw macros string found in the bazecor config file.
        ///
        /// If omitted, will attempt to read it from the keyboard.
        #[clap(short, long)]
        macros: Option<String>,
        /// The path the macros will be saved to.
        #[clap(default_value = "macros.json")]
        path: PathBuf,
    },
    /// Formats the macros JSON file.
    Format {
        /// The path of the macros JSON file.
        #[clap(default_value = "macros.json")]
        path: PathBuf,
    },
    /// Reads a macros file and outputs it as a raw macros data string that
    /// can be used to send to the keyboard.
    ToCommandData {
        /// The path of the macros JSON file.
        #[clap(default_value = "macros.json")]
        path: PathBuf,
        /// The device's macro map memory size.
        ///
        /// The real value is probed from the device with the
        /// `macros.memory` command when applying; when serializing
        /// offline, this defaults to the stock 2048.
        #[clap(long, default_value_t = DefyKeyboard::MACRO_MEMORY_SIZE as u16)]
        memory: u16,
    },
    /// Apply the macros to the keyboard.
    Apply {
        /// The path of the macros file.
        #[clap(default_value = "macros.json")]
        path: PathBuf,
        /// Do not rewrite the macros file after applying.
        ///
        /// By default, the file is rewritten canonically after a
        /// successful apply, keeping it prettified and its macro numbers
        /// up to date. This flag leaves your file untouched.
        #[clap(long, default_value_t = false)]
        no_write: bool,
    },
}

impl MacroCommands {
    async fn perform(self) -> Result<(), error_stack::Report<Error>> {
        match self {
            Self::New { macros, path } => {
                let map = if let Some(macros) = macros {
                    macros
                        .parse::<MacroMap>()
                        .change_context(Error)
                        .attach("parsing macros string")?
                } else {
                    let mut defy = DefyKeyboard::new()
                        .await
                        .change_context(Error)
                        .attach("connecting to the Defy keyboard")?;

                    defy.get_macros()
                        .await
                        .change_context(Error)
                        .attach("getting macros from the Defy")?
                };

                safe_pretty_json_file(&map, &path).await?;

                Ok(())
            }
            Self::ToCommandData { path, memory } => {
                let map = read_json_file::<MacroMap>(&path).await?;

                let str_data = parsing::macros::MacroMap::from(&map)
                    .to_command_data(usize::from(memory))
                    .change_context(Error)
                    .attach("serializing macros to command data")?;

                println!("{str_data}");

                Ok(())
            }
            Self::Apply { path, no_write } => {
                let map = read_json_file::<MacroMap>(&path).await?;

                let mut defy = DefyKeyboard::new()
                    .await
                    .change_context(Error)
                    .attach("connecting to the Defy keyboard")?;

                defy.apply_macros(&map)
                    .await
                    .change_context(Error)
                    .attach("applying macros to the Defy")?;

                if !no_write {
                    // We override the original config file to make sure everything
                    // stays nice and prettified
                    safe_pretty_json_file(&map, &path).await?;
                }

                Ok(())
            }
            Self::Format { path } => {
                let map = read_json_file::<MacroMap>(&path).await?;

                safe_pretty_json_file(&map, &path).await?;

                Ok(())
            }
        }
    }
}

/// A single setting that can be read from the device.
#[derive(Clone, Copy, Debug, ValueEnum)]
enum ReadableSetting {
    /// The firmware version triple (Bazecor, Kaleidoscope, firmware).
    Version,
    /// The Neuron identity string.
    ChipId,
    /// Whether this keyboard is the wireless variant.
    Wireless,
    /// The physical key layout.
    Layout,
    /// Whether only the layers in `keymap.custom` are used.
    OnlyCustom,
    /// The layer the keyboard boots into.
    DefaultLayer,
    /// The qukeys hold timeout, in ms.
    QukeysHoldTimeout,
    /// The qukeys overlap threshold, as a percentage.
    QukeysOverlapThreshold,
    /// The minimum time a qukey must be held to count as a hold, in ms.
    QukeysMinimumHoldTime,
    /// The minimum interval since the previous press for a qukey hold, in ms.
    QukeysMinimumPriorInterval,
    /// The superkey tap-hold timeout, in ms.
    SuperkeysTimeout,
    /// The superkey auto-repeat interval, in ms.
    SuperkeysRepeat,
    /// The superkey double-tap wait window, in ms.
    SuperkeysWaitFor,
    /// The superkey hold start time, in ms.
    SuperkeysHoldStart,
    /// The superkey overlap threshold, as a percentage.
    SuperkeysOverlap,
    /// The base mouse cursor speed.
    MouseSpeed,
    /// The mouse cursor start-up delay, in ms.
    MouseSpeedDelay,
    /// The mouse acceleration speed.
    MouseAccelSpeed,
    /// The mouse acceleration delay, in ms.
    MouseAccelDelay,
    /// The mouse wheel speed.
    MouseWheelSpeed,
    /// The mouse wheel delay, in ms.
    MouseWheelDelay,
    /// The mouse cursor speed limit.
    MouseSpeedLimit,
    /// The macro map memory size (probed and sanitized).
    MacrosMemory,
}

/// A single setting that can be written to the device.
#[derive(Clone, Copy, Debug, ValueEnum)]
enum WritableSetting {
    /// Whether only the layers in `keymap.custom` are used.
    OnlyCustom,
    /// The layer the keyboard boots into.
    DefaultLayer,
    /// The qukeys hold timeout, in ms.
    QukeysHoldTimeout,
    /// The qukeys overlap threshold, as a percentage.
    QukeysOverlapThreshold,
    /// The minimum time a qukey must be held to count as a hold, in ms.
    QukeysMinimumHoldTime,
    /// The minimum interval since the previous press for a qukey hold, in ms.
    QukeysMinimumPriorInterval,
    /// The superkey tap-hold timeout, in ms.
    SuperkeysTimeout,
    /// The superkey auto-repeat interval, in ms.
    SuperkeysRepeat,
    /// The superkey double-tap wait window, in ms.
    SuperkeysWaitFor,
    /// The superkey hold start time, in ms.
    SuperkeysHoldStart,
    /// The superkey overlap threshold, as a percentage.
    SuperkeysOverlap,
    /// The base mouse cursor speed.
    MouseSpeed,
    /// The mouse cursor start-up delay, in ms.
    MouseSpeedDelay,
    /// The mouse acceleration speed.
    MouseAccelSpeed,
    /// The mouse acceleration delay, in ms.
    MouseAccelDelay,
    /// The mouse wheel speed.
    MouseWheelSpeed,
    /// The mouse wheel delay, in ms.
    MouseWheelDelay,
    /// The mouse cursor speed limit.
    MouseSpeedLimit,
}

#[derive(Subcommand)]
enum SettingsCommands {
    /// Reads the device's identity: version, chip id, wireless, layout.
    Identity,
    /// Reads a single setting from the device.
    Get {
        /// The setting to read.
        setting: ReadableSetting,
    },
    /// Writes a single setting to the device.
    Set {
        /// The setting to write.
        setting: WritableSetting,
        /// The value to write.
        ///
        /// Numbers are plain decimals. `only-custom` accepts
        /// `1`/`0`/`true`/`false`. `default-layer` accepts a layer
        /// number or `none`.
        value: String,
    },
    /// Resets every plugin setting stored in the device's EEPROM.
    ///
    /// This wipes qukeys, superkeys timings, mouse settings, LED
    /// settings, and everything else plugins keep in the EEPROM. It
    /// cannot be undone.
    ResetEeprom {
        /// Actually perform the reset.
        ///
        /// Without this flag, the command prints a warning and exits
        /// without opening a connection to the device.
        #[clap(long, default_value_t = false)]
        yes: bool,
    },
}

/// Parses a boolean from a `1`/`0`/`true`/`false` value string.
fn parse_bool_value(value: &str) -> Result<bool, Error> {
    match value {
        "1" | "true" => Ok(true),
        "0" | "false" => Ok(false),
        _ => Err(Error),
    }
}

/// Parses a default layer from a `none`/layer-number value string.
fn parse_default_layer_value(value: &str) -> Result<parsing::settings::DefaultLayer, Error> {
    if value == "none" {
        return Ok(parsing::settings::DefaultLayer::none());
    }

    value
        .parse::<u8>()
        .ok()
        .and_then(parsing::settings::DefaultLayer::layer)
        .ok_or(Error)
}

impl SettingsCommands {
    async fn perform(self) -> Result<(), error_stack::Report<Error>> {
        match self {
            Self::Identity => {
                let mut defy = DefyKeyboard::new()
                    .await
                    .change_context(Error)
                    .attach("connecting to the Defy keyboard")?;

                let version = defy
                    .get_version()
                    .await
                    .change_context(Error)
                    .attach("reading `version` from the Defy")?;
                let chip_id = defy
                    .get_chip_id()
                    .await
                    .change_context(Error)
                    .attach("reading `hardware.chip_id` from the Defy")?;
                let wireless = defy
                    .is_wireless()
                    .await
                    .change_context(Error)
                    .attach("reading `hardware.wireless` from the Defy")?;
                let layout = defy
                    .get_layout()
                    .await
                    .change_context(Error)
                    .attach("reading `hardware.layout` from the Defy")?;

                println!("version: {version}");
                println!("chip id: {chip_id}");
                println!("wireless: {wireless}");
                println!("layout: {layout}");

                Ok(())
            }
            Self::Get { setting } => {
                let mut defy = DefyKeyboard::new()
                    .await
                    .change_context(Error)
                    .attach("connecting to the Defy keyboard")?;

                let value = match setting {
                    ReadableSetting::Version => defy
                        .get_version()
                        .await
                        .change_context(Error)
                        .attach("reading `version` from the Defy")?
                        .to_string(),
                    ReadableSetting::ChipId => defy
                        .get_chip_id()
                        .await
                        .change_context(Error)
                        .attach("reading `hardware.chip_id` from the Defy")?
                        .to_string(),
                    ReadableSetting::Wireless => defy
                        .is_wireless()
                        .await
                        .change_context(Error)
                        .attach("reading `hardware.wireless` from the Defy")?
                        .to_string(),
                    ReadableSetting::Layout => defy
                        .get_layout()
                        .await
                        .change_context(Error)
                        .attach("reading `hardware.layout` from the Defy")?
                        .to_string(),
                    ReadableSetting::OnlyCustom => defy
                        .get_only_custom()
                        .await
                        .change_context(Error)
                        .attach("reading `keymap.onlyCustom` from the Defy")?
                        .to_string(),
                    ReadableSetting::DefaultLayer => defy
                        .get_default_layer()
                        .await
                        .change_context(Error)
                        .attach("reading `settings.defaultLayer` from the Defy")?
                        .to_string(),
                    ReadableSetting::QukeysHoldTimeout => defy
                        .get_qukeys_hold_timeout()
                        .await
                        .change_context(Error)
                        .attach("reading `qukeys.holdTimeout` from the Defy")?
                        .to_string(),
                    ReadableSetting::QukeysOverlapThreshold => defy
                        .get_qukeys_overlap_threshold()
                        .await
                        .change_context(Error)
                        .attach("reading `qukeys.overlapThreshold` from the Defy")?
                        .to_string(),
                    ReadableSetting::QukeysMinimumHoldTime => defy
                        .get_qukeys_minimum_hold_time()
                        .await
                        .change_context(Error)
                        .attach("reading `qukeys.minimumHoldTime` from the Defy")?
                        .to_string(),
                    ReadableSetting::QukeysMinimumPriorInterval => defy
                        .get_qukeys_minimum_prior_interval()
                        .await
                        .change_context(Error)
                        .attach("reading `qukeys.minimumPriorInterval` from the Defy")?
                        .to_string(),
                    ReadableSetting::SuperkeysTimeout => defy
                        .get_superkeys_timeout()
                        .await
                        .change_context(Error)
                        .attach("reading `superkeys.timeout` from the Defy")?
                        .to_string(),
                    ReadableSetting::SuperkeysRepeat => defy
                        .get_superkeys_repeat()
                        .await
                        .change_context(Error)
                        .attach("reading `superkeys.repeat` from the Defy")?
                        .to_string(),
                    ReadableSetting::SuperkeysWaitFor => defy
                        .get_superkeys_wait_for()
                        .await
                        .change_context(Error)
                        .attach("reading `superkeys.waitfor` from the Defy")?
                        .to_string(),
                    ReadableSetting::SuperkeysHoldStart => defy
                        .get_superkeys_hold_start()
                        .await
                        .change_context(Error)
                        .attach("reading `superkeys.holdstart` from the Defy")?
                        .to_string(),
                    ReadableSetting::SuperkeysOverlap => defy
                        .get_superkeys_overlap()
                        .await
                        .change_context(Error)
                        .attach("reading `superkeys.overlap` from the Defy")?
                        .to_string(),
                    ReadableSetting::MouseSpeed => defy
                        .get_mouse_speed()
                        .await
                        .change_context(Error)
                        .attach("reading `mouse.speed` from the Defy")?
                        .to_string(),
                    ReadableSetting::MouseSpeedDelay => defy
                        .get_mouse_speed_delay()
                        .await
                        .change_context(Error)
                        .attach("reading `mouse.speedDelay` from the Defy")?
                        .to_string(),
                    ReadableSetting::MouseAccelSpeed => defy
                        .get_mouse_accel_speed()
                        .await
                        .change_context(Error)
                        .attach("reading `mouse.accelSpeed` from the Defy")?
                        .to_string(),
                    ReadableSetting::MouseAccelDelay => defy
                        .get_mouse_accel_delay()
                        .await
                        .change_context(Error)
                        .attach("reading `mouse.accelDelay` from the Defy")?
                        .to_string(),
                    ReadableSetting::MouseWheelSpeed => defy
                        .get_mouse_wheel_speed()
                        .await
                        .change_context(Error)
                        .attach("reading `mouse.wheelSpeed` from the Defy")?
                        .to_string(),
                    ReadableSetting::MouseWheelDelay => defy
                        .get_mouse_wheel_delay()
                        .await
                        .change_context(Error)
                        .attach("reading `mouse.wheelDelay` from the Defy")?
                        .to_string(),
                    ReadableSetting::MouseSpeedLimit => defy
                        .get_mouse_speed_limit()
                        .await
                        .change_context(Error)
                        .attach("reading `mouse.speedLimit` from the Defy")?
                        .to_string(),
                    ReadableSetting::MacrosMemory => defy
                        .macro_memory_size()
                        .await
                        .change_context(Error)
                        .attach("reading `macros.memory` from the Defy")?
                        .to_string(),
                };

                println!("{value}");

                Ok(())
            }
            Self::Set { setting, value } => {
                // Parse the value into its typed write BEFORE touching
                // the device, so a bad value fails without ever
                // opening a connection.
                let write = parse_setting_write(setting, &value)?;

                let mut defy = DefyKeyboard::new()
                    .await
                    .change_context(Error)
                    .attach("connecting to the Defy keyboard")?;

                write
                    .apply(&mut defy)
                    .await
                    .change_context(Error)
                    .attach("writing the setting to the Defy")?;

                println!("Wrote `{}` to the Defy.", write.command_name());

                Ok(())
            }
            Self::ResetEeprom { yes } => {
                if !yes {
                    eprintln!(
                        "This wipes every plugin setting stored in the keyboard's EEPROM \
                        (qukeys, superkeys timings, mouse and LED settings, and more). \
                        It cannot be undone. Re-run with `--yes` to actually reset it."
                    );

                    // No device connection is opened without `--yes`.
                    return Ok(());
                }

                let mut defy = DefyKeyboard::new()
                    .await
                    .change_context(Error)
                    .attach("connecting to the Defy keyboard")?;

                defy.reset_eeprom()
                    .await
                    .change_context(Error)
                    .attach("resetting the Defy's EEPROM")?;

                println!("The Defy's EEPROM has been reset.");

                Ok(())
            }
        }
    }
}

#[derive(Subcommand)]
enum BackupCommands {
    /// Capture a full configuration backup from the device and save it
    /// as a Bazecor-compatible backup JSON file.
    ///
    /// The capture list is the device's `help` output minus the same
    /// deny-list Bazecor applies, with `led.brightnessUG` force-added.
    /// Every listed command is sent as a read and its response stored
    /// verbatim — capturing never writes to the keyboard, so it is
    /// safe to run at any time (e.g. before an apply). The file this
    /// writes loads into Bazecor's backup flow.
    Create {
        /// The path the backup JSON file will be saved to.
        #[clap(default_value = "backup.json")]
        path: PathBuf,
    },
    /// Summarize a backup JSON file.
    ///
    /// This never opens a connection to the device: it only parses the
    /// file and prints what it holds. Works with files written by
    /// `backup create` and with Bazecor's own backup files.
    Show {
        /// The path of the backup JSON file.
        path: PathBuf,
    },
}

impl BackupCommands {
    async fn perform(self) -> Result<(), error_stack::Report<Error>> {
        match self {
            Self::Create { path } => {
                let mut defy = DefyKeyboard::new()
                    .await
                    .change_context(Error)
                    .attach("connecting to the Defy keyboard")?;

                let backup = defy
                    .create_backup()
                    .await
                    .change_context(Error)
                    .attach("capturing the backup from the Defy")?;

                safe_pretty_json_file(&backup, &path).await?;

                println!(
                    "Captured {} commands from Neuron {} into {}.",
                    backup.backup.len(),
                    backup.neuron_id,
                    path.display()
                );

                Ok(())
            }
            Self::Show { path } => {
                let backup = read_json_file::<DefyBackup>(&path).await?;

                println!("neuron id: {}", backup.neuron_id);
                println!("neuron name: {}", backup.neuron.name);

                for (name, version) in [
                    ("bazecor", &backup.versions.bazecor),
                    ("kaleidoscope", &backup.versions.kaleidoscope),
                    ("firmware", &backup.versions.firmware),
                ] {
                    if let Some(version) = version {
                        println!("version {name}: {version}");
                    }
                }

                println!("commands captured: {}", backup.backup.len());

                for entry in &backup.backup {
                    println!("  {} ({} bytes of data)", entry.command, entry.data.len());
                }

                Ok(())
            }
        }
    }
}

/// A parsed setting write: the value has been validated from its
/// command-line string, and only the device round-trip remains.
#[derive(Clone, Copy, Debug)]
enum SettingWrite {
    OnlyCustom(bool),
    DefaultLayer(parsing::settings::DefaultLayer),
    QukeysHoldTimeout(u16),
    QukeysOverlapThreshold(u8),
    QukeysMinimumHoldTime(u8),
    QukeysMinimumPriorInterval(u8),
    SuperkeysTimeout(u16),
    SuperkeysRepeat(u8),
    SuperkeysWaitFor(u16),
    SuperkeysHoldStart(u16),
    SuperkeysOverlap(u8),
    MouseSpeed(u8),
    MouseSpeedDelay(u16),
    MouseAccelSpeed(u8),
    MouseAccelDelay(u16),
    MouseWheelSpeed(u8),
    MouseWheelDelay(u16),
    MouseSpeedLimit(u8),
}

/// Parses a setting write from its command-line value string. This
/// never talks to the device.
fn parse_setting_write(
    setting: WritableSetting,
    value: &str,
) -> Result<SettingWrite, error_stack::Report<Error>> {
    let write = match setting {
        WritableSetting::OnlyCustom => SettingWrite::OnlyCustom(
            parse_bool_value(value)
                .change_context(Error)
                .attach("parsing the value; expected 1/0/true/false")?,
        ),
        WritableSetting::DefaultLayer => SettingWrite::DefaultLayer(
            parse_default_layer_value(value)
                .change_context(Error)
                .attach("parsing the value; expected a layer number or `none`")?,
        ),
        WritableSetting::QukeysHoldTimeout => SettingWrite::QukeysHoldTimeout(
            parse_u16_value(value)
                .change_context(Error)
                .attach("parsing the value; expected a number")?,
        ),
        WritableSetting::QukeysOverlapThreshold => SettingWrite::QukeysOverlapThreshold(
            parse_u8_value(value)
                .change_context(Error)
                .attach("parsing the value; expected a number")?,
        ),
        WritableSetting::QukeysMinimumHoldTime => SettingWrite::QukeysMinimumHoldTime(
            parse_u8_value(value)
                .change_context(Error)
                .attach("parsing the value; expected a number")?,
        ),
        WritableSetting::QukeysMinimumPriorInterval => SettingWrite::QukeysMinimumPriorInterval(
            parse_u8_value(value)
                .change_context(Error)
                .attach("parsing the value; expected a number")?,
        ),
        WritableSetting::SuperkeysTimeout => SettingWrite::SuperkeysTimeout(
            parse_u16_value(value)
                .change_context(Error)
                .attach("parsing the value; expected a number")?,
        ),
        WritableSetting::SuperkeysRepeat => SettingWrite::SuperkeysRepeat(
            parse_u8_value(value)
                .change_context(Error)
                .attach("parsing the value; expected a number")?,
        ),
        WritableSetting::SuperkeysWaitFor => SettingWrite::SuperkeysWaitFor(
            parse_u16_value(value)
                .change_context(Error)
                .attach("parsing the value; expected a number")?,
        ),
        WritableSetting::SuperkeysHoldStart => SettingWrite::SuperkeysHoldStart(
            parse_u16_value(value)
                .change_context(Error)
                .attach("parsing the value; expected a number")?,
        ),
        WritableSetting::SuperkeysOverlap => SettingWrite::SuperkeysOverlap(
            parse_u8_value(value)
                .change_context(Error)
                .attach("parsing the value; expected a number")?,
        ),
        WritableSetting::MouseSpeed => SettingWrite::MouseSpeed(
            parse_u8_value(value)
                .change_context(Error)
                .attach("parsing the value; expected a number")?,
        ),
        WritableSetting::MouseSpeedDelay => SettingWrite::MouseSpeedDelay(
            parse_u16_value(value)
                .change_context(Error)
                .attach("parsing the value; expected a number")?,
        ),
        WritableSetting::MouseAccelSpeed => SettingWrite::MouseAccelSpeed(
            parse_u8_value(value)
                .change_context(Error)
                .attach("parsing the value; expected a number")?,
        ),
        WritableSetting::MouseAccelDelay => SettingWrite::MouseAccelDelay(
            parse_u16_value(value)
                .change_context(Error)
                .attach("parsing the value; expected a number")?,
        ),
        WritableSetting::MouseWheelSpeed => SettingWrite::MouseWheelSpeed(
            parse_u8_value(value)
                .change_context(Error)
                .attach("parsing the value; expected a number")?,
        ),
        WritableSetting::MouseWheelDelay => SettingWrite::MouseWheelDelay(
            parse_u16_value(value)
                .change_context(Error)
                .attach("parsing the value; expected a number")?,
        ),
        WritableSetting::MouseSpeedLimit => SettingWrite::MouseSpeedLimit(
            parse_u8_value(value)
                .change_context(Error)
                .attach("parsing the value; expected a number")?,
        ),
    };

    Ok(write)
}

impl SettingWrite {
    /// The Focus command this write maps to, for user-facing messages.
    fn command_name(&self) -> &'static str {
        match self {
            Self::OnlyCustom(_) => "keymap.onlyCustom",
            Self::DefaultLayer(_) => "settings.defaultLayer",
            Self::QukeysHoldTimeout(_) => "qukeys.holdTimeout",
            Self::QukeysOverlapThreshold(_) => "qukeys.overlapThreshold",
            Self::QukeysMinimumHoldTime(_) => "qukeys.minimumHoldTime",
            Self::QukeysMinimumPriorInterval(_) => "qukeys.minimumPriorInterval",
            Self::SuperkeysTimeout(_) => "superkeys.timeout",
            Self::SuperkeysRepeat(_) => "superkeys.repeat",
            Self::SuperkeysWaitFor(_) => "superkeys.waitfor",
            Self::SuperkeysHoldStart(_) => "superkeys.holdstart",
            Self::SuperkeysOverlap(_) => "superkeys.overlap",
            Self::MouseSpeed(_) => "mouse.speed",
            Self::MouseSpeedDelay(_) => "mouse.speedDelay",
            Self::MouseAccelSpeed(_) => "mouse.accelSpeed",
            Self::MouseAccelDelay(_) => "mouse.accelDelay",
            Self::MouseWheelSpeed(_) => "mouse.wheelSpeed",
            Self::MouseWheelDelay(_) => "mouse.wheelDelay",
            Self::MouseSpeedLimit(_) => "mouse.speedLimit",
        }
    }

    /// Sends the write to the device.
    async fn apply(self, defy: &mut DefyKeyboard) -> Result<(), error_stack::Report<Error>> {
        match self {
            Self::OnlyCustom(value) => defy
                .set_only_custom(value)
                .await
                .change_context(Error)
                .attach("writing `keymap.onlyCustom` to the Defy"),
            Self::DefaultLayer(value) => defy
                .set_default_layer(value)
                .await
                .change_context(Error)
                .attach("writing `settings.defaultLayer` to the Defy"),
            Self::QukeysHoldTimeout(value) => defy
                .set_qukeys_hold_timeout(value)
                .await
                .change_context(Error)
                .attach("writing `qukeys.holdTimeout` to the Defy"),
            Self::QukeysOverlapThreshold(value) => defy
                .set_qukeys_overlap_threshold(value)
                .await
                .change_context(Error)
                .attach("writing `qukeys.overlapThreshold` to the Defy"),
            Self::QukeysMinimumHoldTime(value) => defy
                .set_qukeys_minimum_hold_time(value)
                .await
                .change_context(Error)
                .attach("writing `qukeys.minimumHoldTime` to the Defy"),
            Self::QukeysMinimumPriorInterval(value) => defy
                .set_qukeys_minimum_prior_interval(value)
                .await
                .change_context(Error)
                .attach("writing `qukeys.minimumPriorInterval` to the Defy"),
            Self::SuperkeysTimeout(value) => defy
                .set_superkeys_timeout(value)
                .await
                .change_context(Error)
                .attach("writing `superkeys.timeout` to the Defy"),
            Self::SuperkeysRepeat(value) => defy
                .set_superkeys_repeat(value)
                .await
                .change_context(Error)
                .attach("writing `superkeys.repeat` to the Defy"),
            Self::SuperkeysWaitFor(value) => defy
                .set_superkeys_wait_for(value)
                .await
                .change_context(Error)
                .attach("writing `superkeys.waitfor` to the Defy"),
            Self::SuperkeysHoldStart(value) => defy
                .set_superkeys_hold_start(value)
                .await
                .change_context(Error)
                .attach("writing `superkeys.holdstart` to the Defy"),
            Self::SuperkeysOverlap(value) => defy
                .set_superkeys_overlap(value)
                .await
                .change_context(Error)
                .attach("writing `superkeys.overlap` to the Defy"),
            Self::MouseSpeed(value) => defy
                .set_mouse_speed(value)
                .await
                .change_context(Error)
                .attach("writing `mouse.speed` to the Defy"),
            Self::MouseSpeedDelay(value) => defy
                .set_mouse_speed_delay(value)
                .await
                .change_context(Error)
                .attach("writing `mouse.speedDelay` to the Defy"),
            Self::MouseAccelSpeed(value) => defy
                .set_mouse_accel_speed(value)
                .await
                .change_context(Error)
                .attach("writing `mouse.accelSpeed` to the Defy"),
            Self::MouseAccelDelay(value) => defy
                .set_mouse_accel_delay(value)
                .await
                .change_context(Error)
                .attach("writing `mouse.accelDelay` to the Defy"),
            Self::MouseWheelSpeed(value) => defy
                .set_mouse_wheel_speed(value)
                .await
                .change_context(Error)
                .attach("writing `mouse.wheelSpeed` to the Defy"),
            Self::MouseWheelDelay(value) => defy
                .set_mouse_wheel_delay(value)
                .await
                .change_context(Error)
                .attach("writing `mouse.wheelDelay` to the Defy"),
            Self::MouseSpeedLimit(value) => defy
                .set_mouse_speed_limit(value)
                .await
                .change_context(Error)
                .attach("writing `mouse.speedLimit` to the Defy"),
        }
    }
}

/// Parses a u8 from a value string.
fn parse_u8_value(value: &str) -> Result<u8, Error> {
    value.parse().map_err(|_| Error)
}

/// Parses a u16 from a value string.
fn parse_u16_value(value: &str) -> Result<u16, Error> {
    value.parse().map_err(|_| Error)
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
    fn perform(self) -> Result<(), error_stack::Report<Error>> {
        match self {
            Self::Describe { code } => {
                let key = KeyKind::from(code);

                println!("{key}");

                Ok(())
            }
            Self::DescribeSequence { keys } => {
                let parsed = keys
                    .split(' ')
                    .filter(|seq| !seq.is_empty())
                    .map(|seq| {
                        seq.parse::<u16>()
                            .map_err(|_| InvalidKeyCodeError(seq.to_owned()))
                    })
                    .collect::<Result<Vec<_>, _>>();

                let keys = match parsed {
                    Ok(keys) => keys,
                    Err(err) => {
                        return Err(err
                            .into_report()
                            .change_context(Error)
                            .attach("describing the key code sequence; each key must be a u16"));
                    }
                };

                let keys = keys
                    .into_iter()
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
async fn main() -> Result<(), error_stack::Report<Error>> {
    Cli::parse().perform().await?;

    Ok(())
}

#[derive(Clone, Debug, Display, Error)]
#[display(
    "failed to run command: `{cmd}` is not a valid command, did you mean one of these? {}",
    suggestions.join(", ")
)]
struct RunCommandError {
    cmd: String,
    suggestions: Vec<String>,
}

#[derive(Clone, Debug, Display, Error)]
#[display("`{_0}` is not a valid u16 key code")]
struct InvalidKeyCodeError(#[error(not(source))] String);

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

async fn read_json_file<T>(path: &Path) -> Result<T, error_stack::Report<Error>>
where
    T: for<'de> serde::Deserialize<'de>,
{
    let file = File::open(path)
        .await
        .change_context(Error)
        .attach("opening the file")
        .attach_with(|| path.to_string_lossy().into_owned())?;

    let mut data = vec![];

    BufReader::new(file)
        .read_to_end(&mut data)
        .await
        .change_context(Error)
        .attach("reading file contents")
        .attach_with(|| path.to_string_lossy().into_owned())?;

    serde_json::from_reader::<_, T>(data.as_slice())
        .change_context(Error)
        .attach("parsing file contents")
        .attach_with(|| path.to_string_lossy().into_owned())
}

async fn safe_pretty_json_file<T>(data: &T, path: &Path) -> Result<(), error_stack::Report<Error>>
where
    T: serde::Serialize,
{
    let file = File::create(path)
        .await
        .change_context(Error)
        .attach("creating file")
        .attach_with(|| path.to_string_lossy().into_owned())?;

    let mut writer = BufWriter::new(file);

    let data = serde_json::to_vec_pretty(data).unwrap();

    writer
        .write_all(&data)
        .await
        .change_context(Error)
        .attach("writing data to the file")
        .attach_with(|| path.to_string_lossy().into_owned())?;

    writer
        .flush()
        .await
        .change_context(Error)
        .attach("flushing data to the file")
        .attach_with(|| path.to_string_lossy().into_owned())?;

    Ok(())
}
