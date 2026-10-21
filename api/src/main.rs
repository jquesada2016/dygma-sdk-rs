#[macro_use]
extern crate derive_more;

mod diagnostics;

use clap::{Parser, Subcommand, ValueEnum};
use dygma_cli::devices::conversions;
use dygma_cli::devices::defaults;
use dygma_cli::devices::defy::{
    CreateBackupError, DefyColormap, DefyKeyboard, DefyKeymap, MacroMap, SuperkeyMap,
};
use dygma_cli::focus_api::parsing::backup::DefyBackup;
use dygma_cli::focus_api::parsing::palette::Palette;
use dygma_cli::focus_api::{FocusApiConnection, parsing};
use dygma_cli::keycode_tables::{Blank, KeyKind};
use error_stack::{IntoReport, ResultExt};
use itertools::Itertools;
use std::io::ErrorKind;
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
    /// Commands for working with config files.
    #[command(subcommand)]
    Config(ConfigCommands),
    /// Commands for working with device settings and identity.
    #[command(subcommand)]
    Settings(SettingsCommands),
    /// Commands for working with LEDs: the palette, the colormap, and
    /// the brightness/idle/fade settings.
    #[command(subcommand)]
    Leds(LedsCommands),
    /// Commands for working with the wireless side: the side batteries,
    /// the RF link, and the Bluetooth identity.
    ///
    /// The battery level and status are live state, read here; the
    /// persisted wireless settings (battery saving mode, RF power,
    /// channel hop, the Bluetooth device name) ride
    /// `settings get`/`settings set` like every other setting.
    #[command(subcommand)]
    Wireless(WirelessCommands),
    /// Commands for working with device backups.
    #[command(subcommand)]
    Backup(BackupCommands),
    /// Commands for working with the neuron names store.
    Names {
        /// The path of the names store JSON file.
        ///
        /// Defaults to `$HOME/.dygma-sdk/names.json`.
        #[clap(long, global = true)]
        store: Option<PathBuf>,
        #[command(subcommand)]
        command: NamesCommands,
    },
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
            Self::Config(cmd) => cmd.perform().await,
            Self::Settings(cmd) => cmd.perform().await,
            Self::Leds(cmd) => cmd.perform().await,
            Self::Wireless(cmd) => cmd.perform().await,
            Self::Backup(cmd) => cmd.perform().await,
            Self::Names { store, command } => command.perform(store.as_deref()).await,
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
                    match keymap.parse::<DefyKeymap>() {
                        Ok(keymap) => keymap,
                        Err(err) => {
                            return Err(diagnostics::wire_parse_report(
                                diagnostics::WireFormat::Keymap,
                                &keymap,
                                diagnostics::WireFailure::keymap(&err),
                            ));
                        }
                    }
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

                // Infallible: a `DefyKeymap` holds exactly the ten
                // layers the command data requires.
                let data = keymap.to_keymap_custom_data();

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
                    match superkeys.parse::<SuperkeyMap>() {
                        Ok(map) => map,
                        Err(err) => {
                            return Err(diagnostics::wire_parse_report(
                                diagnostics::WireFormat::Superkeys,
                                &superkeys,
                                diagnostics::WireFailure::superkeys(&err),
                            ));
                        }
                    }
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
                    match macros.parse::<MacroMap>() {
                        Ok(map) => map,
                        Err(err) => {
                            return Err(diagnostics::wire_parse_report(
                                diagnostics::WireFormat::Macros,
                                &macros,
                                diagnostics::WireFailure::macros(&err),
                            ));
                        }
                    }
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
    /// The keyboard LED brightness (0-255).
    LedBrightness,
    /// The underglow LED brightness (0-255).
    LedBrightnessUG,
    /// The wireless keyboard LED brightness (0-255).
    LedBrightnessWireless,
    /// The wireless underglow LED brightness (0-255).
    LedBrightnessUGWireless,
    /// Whether the LEDs fade in on layer changes.
    LedFade,
    /// The wired LED idle timeout, in seconds.
    IdleLedsTimeLimit,
    /// The wireless LED idle timeout, in seconds.
    IdleLedsWireless,
    /// Whether true sleep is enabled.
    TrueSleep,
    /// The true-sleep timeout, in seconds.
    TrueSleepTime,
    /// Whether the battery-saving mode is active.
    BatterySavingMode,
    /// The RF signal strength (`0` low / `1` medium / `2` high).
    RfPower,
    /// Whether the RF link hops channels.
    RfChannelHop,
    /// The Bluetooth device name (possibly empty).
    BluetoothDeviceName,
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
    /// The keyboard LED brightness (0-255).
    LedBrightness,
    /// The underglow LED brightness (0-255).
    LedBrightnessUG,
    /// The wireless keyboard LED brightness (0-255).
    LedBrightnessWireless,
    /// The wireless underglow LED brightness (0-255).
    LedBrightnessUGWireless,
    /// Whether the LEDs fade in on layer changes (`1`/`0`/`true`/`false`).
    LedFade,
    /// The wired LED idle timeout, in seconds (`0` disables it).
    IdleLedsTimeLimit,
    /// The wireless LED idle timeout, in seconds (`0` disables it).
    IdleLedsWireless,
    /// Whether true sleep is enabled (`1`/`0`/`true`/`false`).
    TrueSleep,
    /// The true-sleep timeout, in seconds.
    TrueSleepTime,
    /// Whether the battery-saving mode is active (`1`/`0`/`true`/`false`).
    BatterySavingMode,
    /// The RF signal strength (`0` low / `1` medium / `2` high).
    RfPower,
    /// Whether the RF link hops channels (`1`/`0`/`true`/`false`).
    RfChannelHop,
    /// The Bluetooth device name (surrounding whitespace is trimmed,
    /// so an all-whitespace value — like an empty one — writes
    /// nothing: the firmware answers a data-less write as a read).
    BluetoothDeviceName,
}

/// The products `config init` knows, as CLI arguments.
#[derive(Clone, Copy, Debug, ValueEnum)]
enum ConfigProduct {
    /// The Dygma Defy.
    Defy,
    /// The Dygma Sonsei.
    Sonsei,
    /// The Dygma Raise (first generation).
    Raise,
    /// The Dygma Raise 2.
    Raise2,
}

impl From<ConfigProduct> for conversions::Product {
    fn from(product: ConfigProduct) -> Self {
        match product {
            ConfigProduct::Defy => Self::Defy,
            ConfigProduct::Sonsei => Self::Sonsei,
            ConfigProduct::Raise => Self::Raise,
            ConfigProduct::Raise2 => Self::Raise2,
        }
    }
}

#[derive(Subcommand)]
enum ConfigCommands {
    /// Writes a product's default configuration as five human-readable
    /// config files: `keymap.json`, `superkeys.json`, `macros.json`,
    /// `palette.json`, and `colormap.json` — the factory default,
    /// ready to edit and to apply (`keymap apply`, `superkeys apply`,
    /// `macros apply`, `leds palette apply`, `leds colormap apply`).
    ///
    /// The defaults are evidence-backed, not invented: they match
    /// Bazecor's shipped Defy defaults, whose two independent sources —
    /// the app's virtual keyboards and its shipped default backups —
    /// agree, and are differentially validated against a recording of
    /// them (tools/bazecor-differential). Refuses to overwrite existing
    /// files unless `--force` is passed.
    Init {
        /// The product to write the default config of.
        #[clap(long, default_value = "defy")]
        product: ConfigProduct,
        /// The directory the config files are written to.
        ///
        /// Created when missing.
        #[clap(long)]
        out_dir: Option<PathBuf>,
        /// Overwrite existing config files.
        #[clap(long, default_value_t = false)]
        force: bool,
        /// Print what would be written without writing any file.
        #[clap(long, default_value_t = false)]
        dry_run: bool,
    },
}

/// One config file `config init` writes: its path and the default
/// value it holds.
struct ConfigFile {
    path: PathBuf,
    /// What the file holds, for the preview and the summary.
    description: &'static str,
}

impl ConfigCommands {
    async fn perform(self) -> Result<(), error_stack::Report<Error>> {
        let Self::Init {
            product,
            out_dir,
            force,
            dry_run,
        } = self;

        // Offline by construction: the defaults are pure data, and
        // every check below runs before any file is written. No
        // device connection is ever opened.
        let config = defaults::default_config(product.into()).map_err(|err| {
            err.into_report().change_context(Error).attach(format!(
                "initializing a default config for the {product:?}; \
                     the defaults that exist for it are recorded in \
                     tools/bazecor-differential/expectations/default-configs.json"
            ))
        })?;

        let out_dir = out_dir.unwrap_or_else(|| PathBuf::from("."));

        let files = [
            ConfigFile {
                path: out_dir.join("keymap.json"),
                description: "the factory keymap: 10 layers (Base / Navigation / FKeys, then \
                              blank), 70 keys each",
            },
            ConfigFile {
                path: out_dir.join("superkeys.json"),
                description: "the factory superkey: superkey 1, the clipboard keys (tap \
                              Ctrl+C, hold Ctrl+V, tap-hold Ctrl+Shift+V, double-tap Ctrl+X)",
            },
            ConfigFile {
                path: out_dir.join("macros.json"),
                description: "the factory macro: macro 1, typing \"Hey, dygmate\"",
            },
            ConfigFile {
                path: out_dir.join("palette.json"),
                description: "the factory palette: 16 RGBW colors (amber accents, silver keys, \
                              pure white)",
            },
            ConfigFile {
                path: out_dir.join("colormap.json"),
                description: "the factory colormap: 10 layers of palette indices, 178 LEDs each \
                              (70 keyboard + 108 underglow)",
            },
        ];

        // A real run refuses to clobber; the dry run reports the same
        // refusal a real run would produce, so it previews reality.
        let mut existing = Vec::new();

        for file in &files {
            match tokio::fs::try_exists(&file.path).await {
                Ok(true) => existing.push(file.path.display().to_string()),
                Ok(false) => {}
                Err(err) => {
                    return Err(err
                        .into_report()
                        .change_context(Error)
                        .attach("checking for an existing config file")
                        .attach(file.path.to_string_lossy().into_owned()));
                }
            }
        }

        if !existing.is_empty() && !force {
            return Err(Error
                .into_report()
                .attach(format!(
                    "initializing a default config in {}",
                    out_dir.display()
                ))
                .attach(format!(
                    "refusing to overwrite {} existing file(s): {}. \
                     Move them aside, or pass --force to overwrite",
                    existing.len(),
                    existing.join(", ")
                )));
        }

        if dry_run {
            println!(
                "Would write the {product:?} default config (matching \
                 Bazecor's shipped defaults) to {}:",
                out_dir.display()
            );

            for file in &files {
                println!("  {} — {}", file.path.display(), file.description);
            }

            println!("Dry run: nothing was written.");
            print_evidence_note();

            return Ok(());
        }

        tokio::fs::create_dir_all(&out_dir)
            .await
            .change_context(Error)
            .attach("creating the output directory")
            .attach_with(|| out_dir.to_string_lossy().into_owned())?;

        // Serialize-then-write, the #22 atomic helper: each file lands
        // whole or not at all. A failure part-way leaves the earlier
        // files written (each is independently valid); the error names
        // the file that failed.
        safe_pretty_json_file(&config.keymap, &files[0].path)
            .await
            .change_context(Error)
            .attach_with(|| format!("writing {}", files[0].path.display()))?;
        safe_pretty_json_file(&config.superkeys, &files[1].path)
            .await
            .change_context(Error)
            .attach_with(|| format!("writing {}", files[1].path.display()))?;
        safe_pretty_json_file(&config.macros, &files[2].path)
            .await
            .change_context(Error)
            .attach_with(|| format!("writing {}", files[2].path.display()))?;
        safe_pretty_json_file(&config.palette, &files[3].path)
            .await
            .change_context(Error)
            .attach_with(|| format!("writing {}", files[3].path.display()))?;
        safe_pretty_json_file(&config.colormap, &files[4].path)
            .await
            .change_context(Error)
            .attach_with(|| format!("writing {}", files[4].path.display()))?;

        println!(
            "Wrote the {product:?} default config (matching Bazecor's \
             shipped defaults) to {}:",
            out_dir.display()
        );

        for file in &files {
            println!("  {} — {}", file.path.display(), file.description);
        }

        print_evidence_note();

        Ok(())
    }
}

/// Prints the recorded-but-unwritten defaults disclosure, shared by
/// the dry-run preview and the real write so both paths are equally
/// informative about what a default config does NOT carry: the
/// palette/colormap and settings defaults exist in the recorded
/// evidence but have no file format here.
fn print_evidence_note() {
    println!(
        "Note: the default LED settings (brightness, fade, idle and \
         true-sleep timers) are recorded in the project source (the \
         bazecor-differential expectations) but are not written: this \
         CLI has no settings file format — settings live on the device \
         (`settings get`/`set`, `leds status`)."
    );
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
        /// number or `none`. `battery-saving-mode` and
        /// `rf-channel-hop` accept `1`/`0`/`true`/`false`, `rf-power`
        /// accepts `0`/`1`/`2`, and `bluetooth-device-name` is sent
        /// unquoted; surrounding whitespace is trimmed.
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
                    ReadableSetting::LedBrightness => defy
                        .get_led_brightness()
                        .await
                        .change_context(Error)
                        .attach("reading `led.brightness` from the Defy")?
                        .to_string(),
                    ReadableSetting::LedBrightnessUG => defy
                        .get_led_underglow_brightness()
                        .await
                        .change_context(Error)
                        .attach("reading `led.brightnessUG` from the Defy")?
                        .to_string(),
                    ReadableSetting::LedBrightnessWireless => defy
                        .get_wireless_led_brightness()
                        .await
                        .change_context(Error)
                        .attach("reading `led.brightness.wireless` from the Defy")?
                        .to_string(),
                    ReadableSetting::LedBrightnessUGWireless => defy
                        .get_wireless_led_underglow_brightness()
                        .await
                        .change_context(Error)
                        .attach("reading `led.brightnessUG.wireless` from the Defy")?
                        .to_string(),
                    ReadableSetting::LedFade => defy
                        .get_led_fade()
                        .await
                        .change_context(Error)
                        .attach("reading `led.fade` from the Defy")?
                        .to_string(),
                    ReadableSetting::IdleLedsTimeLimit => defy
                        .get_idle_leds_time_limit()
                        .await
                        .change_context(Error)
                        .attach("reading `idleleds.time_limit` from the Defy")?
                        .to_string(),
                    ReadableSetting::IdleLedsWireless => defy
                        .get_wireless_idle_leds_time_limit()
                        .await
                        .change_context(Error)
                        .attach("reading `idleleds.wireless` from the Defy")?
                        .to_string(),
                    ReadableSetting::TrueSleep => defy
                        .get_true_sleep()
                        .await
                        .change_context(Error)
                        .attach("reading `idleleds.true_sleep` from the Defy")?
                        .to_string(),
                    ReadableSetting::TrueSleepTime => defy
                        .get_true_sleep_time()
                        .await
                        .change_context(Error)
                        .attach("reading `idleleds.true_sleep_time` from the Defy")?
                        .to_string(),
                    ReadableSetting::BatterySavingMode => defy
                        .get_battery_saving_mode()
                        .await
                        .change_context(Error)
                        .attach("reading `wireless.battery.savingMode` from the Defy")?
                        .to_string(),
                    ReadableSetting::RfPower => defy
                        .get_rf_power()
                        .await
                        .change_context(Error)
                        .attach("reading `wireless.rf.power` from the Defy")?
                        .to_string(),
                    ReadableSetting::RfChannelHop => defy
                        .get_rf_channel_hop()
                        .await
                        .change_context(Error)
                        .attach("reading `wireless.rf.channelHop` from the Defy")?
                        .to_string(),
                    ReadableSetting::BluetoothDeviceName => defy
                        .get_bluetooth_device_name()
                        .await
                        .change_context(Error)
                        .attach("reading `wireless.bluetooth.deviceName` from the Defy")?
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

                let command_name = write.command_name();

                // An empty device name writes an empty payload, which
                // the firmware answers as a read — a silent no-op, not
                // a blank name, so "Wrote" would overstate it. The
                // check runs before `apply` because the write is
                // consumed by it.
                let answered_as_read = matches!(
                    &write,
                    SettingWrite::BluetoothDeviceName(name) if name.as_str().is_empty()
                );

                write
                    .apply(&mut defy)
                    .await
                    .change_context(Error)
                    .attach("writing the setting to the Defy")?;

                if answered_as_read {
                    println!(
                        "Sent `{command_name}` with an empty payload; the firmware \
                         answers an empty name as a read, not a blank name."
                    );
                } else {
                    println!("Wrote `{command_name}` to the Defy.");
                }

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

/// The LED commands: the palette and colormap (each with the same
/// new/format/to-command-data/apply file workflow the keymap group
/// has), plus `leds status` for the scalar LED settings.
#[derive(Subcommand)]
enum LedsCommands {
    /// Reads every LED setting from the device: wired and wireless
    /// brightness, fade, and the idle/true-sleep timers.
    Status,
    /// Commands for working with the device's 16-color palette.
    #[command(subcommand)]
    Palette(PaletteCommands),
    /// Commands for working with the per-layer colormap.
    #[command(subcommand)]
    Colormap(ColormapCommands),
}

#[derive(Subcommand)]
enum PaletteCommands {
    /// Create a new palette config file.
    ///
    /// If omitted, the palette is read from the keyboard.
    New {
        /// The raw palette data string found in a backup file.
        ///
        /// 64 space-separated decimals: 4 RGBW channels per color, 16
        /// colors.
        #[clap(short, long)]
        palette: Option<String>,
        /// The path the palette will be saved to.
        #[clap(default_value = "palette.json")]
        path: PathBuf,
    },
    /// Formats the palette file.
    Format {
        /// The path of the palette JSON file.
        #[clap(default_value = "palette.json")]
        path: PathBuf,
    },
    /// Reads a palette file and outputs it as a raw palette data string
    /// that can be used to send to the keyboard.
    ToCommandData {
        /// The path of the palette JSON file.
        path: PathBuf,
    },
    /// Apply the palette to the keyboard.
    Apply {
        /// The path of the palette file.
        path: PathBuf,
        /// Do not rewrite the palette file after applying.
        ///
        /// By default, the file is rewritten canonically after a
        /// successful apply, keeping it prettified. This flag leaves
        /// your file untouched.
        #[clap(long, default_value_t = false)]
        no_write: bool,
    },
}

#[derive(Subcommand)]
enum ColormapCommands {
    /// Create a new colormap config file.
    ///
    /// If omitted, the colormap is read from the keyboard.
    New {
        /// The raw colormap data string found in a backup file.
        ///
        /// 1780 space-separated palette indices: 178 per layer, 10
        /// layers.
        #[clap(short, long)]
        colormap: Option<String>,
        /// The path the colormap will be saved to.
        #[clap(default_value = "colormap.json")]
        path: PathBuf,
    },
    /// Formats the colormap file.
    Format {
        /// The path of the colormap JSON file.
        #[clap(default_value = "colormap.json")]
        path: PathBuf,
    },
    /// Reads a colormap file and outputs it as a raw colormap data
    /// string that can be used to send to the keyboard.
    ToCommandData {
        /// The path of the colormap JSON file.
        path: PathBuf,
    },
    /// Apply the colormap to the keyboard.
    Apply {
        /// The path of the colormap file.
        path: PathBuf,
        /// Do not rewrite the colormap file after applying.
        #[clap(long, default_value_t = false)]
        no_write: bool,
    },
}

/// The typed value behind a `leds palette`/`leds colormap` file
/// command: parsing, canonical writing, and the wire form are uniform,
/// only the value type differs.
enum LedFileValue {
    /// A 16-color RGBW palette.
    Palette(Palette),
    /// A ten-layer colormap.
    Colormap(Box<DefyColormap>),
}

impl LedFileValue {
    /// Writes the value's JSON file, canonically.
    async fn write(&self, path: &Path) -> Result<(), error_stack::Report<Error>> {
        match self {
            Self::Palette(palette) => safe_pretty_json_file(palette, path).await,
            Self::Colormap(colormap) => safe_pretty_json_file(colormap, path).await,
        }
    }

    /// The value's raw wire form (the `palette`/`colormap.map` command
    /// data).
    fn to_command_data(&self) -> String {
        match self {
            Self::Palette(palette) => palette.to_string(),
            Self::Colormap(colormap) => colormap.to_colormap_map_data(),
        }
    }

    /// Sends the value to the device.
    async fn apply(&self, defy: &mut DefyKeyboard) -> Result<(), error_stack::Report<Error>> {
        match self {
            Self::Palette(palette) => defy
                .apply_palette(palette)
                .await
                .change_context(Error)
                .attach("applying the palette to the Defy"),
            Self::Colormap(colormap) => defy
                .apply_colormap(colormap)
                .await
                .change_context(Error)
                .attach("applying the colormap to the Defy"),
        }
    }
}

/// The two LED file formats the `leds` group edits. Everything that
/// needs to know WHICH format an operation works on — and nothing else
/// — lives here.
#[derive(Clone, Copy)]
enum LedFileFormat {
    /// The 16-color RGBW palette.
    Palette,
    /// The ten-layer colormap.
    Colormap,
}

impl LedFileFormat {
    /// The #16 diagnostics vocabulary for this format's wire failures.
    fn wire_format(self) -> diagnostics::WireFormat {
        match self {
            Self::Palette => diagnostics::WireFormat::Palette,
            Self::Colormap => diagnostics::WireFormat::Colormap,
        }
    }

    /// Parses raw wire data, rendering a failure through the #16
    /// diagnostics bridge for this format.
    fn parse_wire(self, raw: &str) -> Result<LedFileValue, error_stack::Report<Error>> {
        match self {
            Self::Palette => match raw.parse::<Palette>() {
                Ok(palette) => Ok(LedFileValue::Palette(palette)),
                Err(err) => Err(diagnostics::wire_parse_report(
                    self.wire_format(),
                    raw,
                    diagnostics::WireFailure::palette(&err),
                )),
            },
            Self::Colormap => match raw.parse::<DefyColormap>() {
                Ok(colormap) => Ok(LedFileValue::Colormap(Box::new(colormap))),
                Err(err) => Err(diagnostics::wire_parse_report(
                    self.wire_format(),
                    raw,
                    diagnostics::WireFailure::colormap(&err),
                )),
            },
        }
    }

    /// Reads the format's JSON file.
    async fn read(self, path: &Path) -> Result<LedFileValue, error_stack::Report<Error>> {
        match self {
            Self::Palette => Ok(LedFileValue::Palette(
                read_json_file::<Palette>(path).await?,
            )),
            Self::Colormap => Ok(LedFileValue::Colormap(Box::new(
                read_json_file::<DefyColormap>(path).await?,
            ))),
        }
    }
}

impl LedsCommands {
    async fn perform(self) -> Result<(), error_stack::Report<Error>> {
        match self {
            Self::Status => {
                let mut defy = DefyKeyboard::new()
                    .await
                    .change_context(Error)
                    .attach("connecting to the Defy keyboard")?;

                let brightness = defy
                    .get_led_brightness()
                    .await
                    .change_context(Error)
                    .attach("reading `led.brightness` from the Defy")?;
                let underglow = defy
                    .get_led_underglow_brightness()
                    .await
                    .change_context(Error)
                    .attach("reading `led.brightnessUG` from the Defy")?;
                let wireless_brightness = defy
                    .get_wireless_led_brightness()
                    .await
                    .change_context(Error)
                    .attach("reading `led.brightness.wireless` from the Defy")?;
                let wireless_underglow = defy
                    .get_wireless_led_underglow_brightness()
                    .await
                    .change_context(Error)
                    .attach("reading `led.brightnessUG.wireless` from the Defy")?;
                let fade = defy
                    .get_led_fade()
                    .await
                    .change_context(Error)
                    .attach("reading `led.fade` from the Defy")?;
                let idle = defy
                    .get_idle_leds_time_limit()
                    .await
                    .change_context(Error)
                    .attach("reading `idleleds.time_limit` from the Defy")?;
                let wireless_idle = defy
                    .get_wireless_idle_leds_time_limit()
                    .await
                    .change_context(Error)
                    .attach("reading `idleleds.wireless` from the Defy")?;
                let true_sleep = defy
                    .get_true_sleep()
                    .await
                    .change_context(Error)
                    .attach("reading `idleleds.true_sleep` from the Defy")?;
                let true_sleep_time = defy
                    .get_true_sleep_time()
                    .await
                    .change_context(Error)
                    .attach("reading `idleleds.true_sleep_time` from the Defy")?;

                // The percentage is Bazecor's UI mapping, round-half-up
                // (`Math.round((value * 100) / 255)`); the math runs in
                // u16 — a u8 multiply overflows at the top brightness.
                println!(
                    "brightness: {brightness} ({}%)",
                    brightness_percent(brightness)
                );
                println!(
                    "underglow brightness: {underglow} ({}%)",
                    brightness_percent(underglow)
                );
                println!(
                    "wireless brightness: {wireless_brightness} ({}%)",
                    brightness_percent(wireless_brightness)
                );
                println!(
                    "wireless underglow brightness: {wireless_underglow} ({}%)",
                    brightness_percent(wireless_underglow)
                );
                println!("fade on layer change: {fade}");
                println!("idle LEDs timeout: {idle} s{}", describe_minutes(idle));
                println!(
                    "wireless idle LEDs timeout: {wireless_idle} s{}",
                    describe_minutes(wireless_idle)
                );
                println!("true sleep: {true_sleep}");
                println!(
                    "true sleep timeout: {true_sleep_time} s{}",
                    describe_minutes(true_sleep_time)
                );

                Ok(())
            }
            Self::Palette(command) => command.perform(LedFileFormat::Palette).await,
            Self::Colormap(command) => command.perform(LedFileFormat::Colormap).await,
        }
    }
}

/// A brightness byte as Bazecor's UI shows it: the percentage of 255,
/// rounded half-up (`Math.round((value * 100) / 255)` in
/// `LEDSettings.tsx`). The multiply runs in u16 — a `u8` overflows at
/// 255 long before 255 × 100.
fn brightness_percent(brightness: u8) -> u16 {
    (u16::from(brightness) * 100 + 127) / 255
}

/// The wireless commands: the live side-battery state, and the two
/// wireless actions. The persisted wireless settings ride
/// `settings get`/`settings set`, exactly like the LED settings do.
#[derive(Subcommand)]
enum WirelessCommands {
    /// Reads both sides' battery level and status, the battery-saving
    /// mode, the RF power and channel hop, the Bluetooth device name,
    /// and the five Bluetooth info channels.
    ///
    /// Like `leds status`, the reads run in sequence and the first
    /// failure fails the command — there is no partial output. A wired
    /// Defy serves none of these commands (`FOCUS_API.md`: wireless
    /// commands enumerate on wireless hardware only), so on wired
    /// hardware this fails at the first read.
    Status,
    /// Forces the Neuron to re-read both sides' battery state
    /// (`wireless.battery.forceRead`). The sides' answers update a
    /// couple of seconds later; run `wireless status` again after
    /// that wait. (Bazecor's UI re-polls immediately and retries —
    /// up to five reads, 500 ms apart, while a side still answers
    /// disconnected.)
    ForceBatteryRead,
    /// Re-syncs the Neuron with both sides over the RF link
    /// (`wireless.rf.syncPairing`), rebuilding the shared pairing
    /// identity. `FOCUS_API.md` documents the operation as usable only
    /// in wired mode with all cables connected.
    SyncPairing,
}

impl WirelessCommands {
    async fn perform(self) -> Result<(), error_stack::Report<Error>> {
        match self {
            Self::Status => {
                let mut defy = DefyKeyboard::new()
                    .await
                    .change_context(Error)
                    .attach("connecting to the Defy keyboard")?;

                // The `leds status` shape: every read runs to
                // completion before the first print, so a mid-sequence
                // failure leaves no partial output on stdout.
                let mut batteries = Vec::new();
                for side in [
                    parsing::wireless::BatterySide::Left,
                    parsing::wireless::BatterySide::Right,
                ] {
                    let level = defy
                        .get_battery_level(side)
                        .await
                        .change_context(Error)
                        .attach_with(|| {
                            format!("reading `wireless.battery.{side}.level` from the Defy")
                        })?;
                    let status = defy
                        .get_battery_status(side)
                        .await
                        .change_context(Error)
                        .attach_with(|| {
                            format!("reading `wireless.battery.{side}.status` from the Defy")
                        })?;

                    batteries.push((side, level, status));
                }

                let saving_mode = defy
                    .get_battery_saving_mode()
                    .await
                    .change_context(Error)
                    .attach("reading `wireless.battery.savingMode` from the Defy")?;
                let rf_power = defy
                    .get_rf_power()
                    .await
                    .change_context(Error)
                    .attach("reading `wireless.rf.power` from the Defy")?;
                let channel_hop = defy
                    .get_rf_channel_hop()
                    .await
                    .change_context(Error)
                    .attach("reading `wireless.rf.channelHop` from the Defy")?;
                let device_name = defy
                    .get_bluetooth_device_name()
                    .await
                    .change_context(Error)
                    .attach("reading `wireless.bluetooth.deviceName` from the Defy")?;

                let mut info_channels = Vec::new();
                for channel in parsing::wireless::BluetoothChannelId::ALL {
                    let info = defy
                        .get_bluetooth_info_channel(channel)
                        .await
                        .change_context(Error)
                        .attach_with(|| {
                            format!(
                                "reading `wireless.bluetooth.infoChannel {}` from the Defy",
                                channel.get()
                            )
                        })?;

                    info_channels.push((channel, info));
                }

                for (side, level, status) in batteries {
                    println!("battery {side}: {}% ({})", level, status.description());
                }
                println!("battery saving mode: {saving_mode}");
                println!("rf power: {rf_power} ({})", rf_power.description());
                println!("rf channel hop: {channel_hop}");
                println!("bluetooth device name: {device_name}");
                for (channel, info) in info_channels {
                    println!("bluetooth info channel {}: {info}", channel.get());
                }

                Ok(())
            }
            Self::ForceBatteryRead => {
                let mut defy = DefyKeyboard::new()
                    .await
                    .change_context(Error)
                    .attach("connecting to the Defy keyboard")?;

                defy.force_battery_read()
                    .await
                    .change_context(Error)
                    .attach("sending `wireless.battery.forceRead` to the Defy")?;

                println!(
                    "Sent `wireless.battery.forceRead`; the sides' battery \
                     answers update in a couple of seconds — run \
                     `wireless status` again then."
                );

                Ok(())
            }
            Self::SyncPairing => {
                let mut defy = DefyKeyboard::new()
                    .await
                    .change_context(Error)
                    .attach("connecting to the Defy keyboard")?;

                defy.sync_rf_pairing()
                    .await
                    .change_context(Error)
                    .attach("sending `wireless.rf.syncPairing` to the Defy")?;

                println!(
                    "Sent `wireless.rf.syncPairing` to the Defy; \
                     `FOCUS_API.md` documents the operation as \
                     rebuilding the shared RF pairing identity."
                );

                Ok(())
            }
        }
    }
}

/// Describes a seconds value in whole minutes, for `leds status`.
fn describe_minutes(seconds: u16) -> String {
    if seconds.is_multiple_of(60) {
        format!(" ({} min)", seconds / 60)
    } else {
        String::new()
    }
}

impl PaletteCommands {
    async fn perform(self, format: LedFileFormat) -> Result<(), error_stack::Report<Error>> {
        // The palette and colormap subcommands are the same workflow;
        // only the format vocabulary differs. `PaletteCommands::perform`
        // and `ColormapCommands::perform` each dispatch
        // `shared_led_file_command` directly with their own format.
        shared_led_file_command(
            match self {
                PaletteCommands::New { palette, path } => {
                    LedFileCommand::New { raw: palette, path }
                }
                PaletteCommands::Format { path } => LedFileCommand::Format { path },
                PaletteCommands::ToCommandData { path } => LedFileCommand::ToCommandData { path },
                PaletteCommands::Apply { path, no_write } => {
                    LedFileCommand::Apply { path, no_write }
                }
            },
            format,
        )
        .await
    }
}

impl ColormapCommands {
    async fn perform(self, format: LedFileFormat) -> Result<(), error_stack::Report<Error>> {
        shared_led_file_command(
            match self {
                ColormapCommands::New { colormap, path } => LedFileCommand::New {
                    raw: colormap,
                    path,
                },
                ColormapCommands::Format { path } => LedFileCommand::Format { path },
                ColormapCommands::ToCommandData { path } => LedFileCommand::ToCommandData { path },
                ColormapCommands::Apply { path, no_write } => {
                    LedFileCommand::Apply { path, no_write }
                }
            },
            format,
        )
        .await
    }
}

/// One `leds palette`/`leds colormap` file workflow, format-agnostic.
enum LedFileCommand {
    New { raw: Option<String>, path: PathBuf },
    Format { path: PathBuf },
    ToCommandData { path: PathBuf },
    Apply { path: PathBuf, no_write: bool },
}

/// Runs one palette/colormap file workflow against its wire format.
/// Never writes to the device except through `Apply`, and `New` without
/// a raw string reads the device (a read, never a write).
async fn shared_led_file_command(
    command: LedFileCommand,
    format: LedFileFormat,
) -> Result<(), error_stack::Report<Error>> {
    match command {
        LedFileCommand::New { raw, path } => {
            let value = if let Some(raw) = raw {
                format.parse_wire(&raw)?
            } else {
                let mut defy = DefyKeyboard::new()
                    .await
                    .change_context(Error)
                    .attach("connecting to the Defy keyboard")?;

                match format {
                    LedFileFormat::Palette => LedFileValue::Palette(
                        defy.get_palette()
                            .await
                            .change_context(Error)
                            .attach("getting the palette from the Defy")?,
                    ),
                    LedFileFormat::Colormap => LedFileValue::Colormap(Box::new(
                        defy.get_colormap()
                            .await
                            .change_context(Error)
                            .attach("getting the colormap from the Defy")?,
                    )),
                }
            };

            value.write(&path).await?;

            Ok(())
        }
        LedFileCommand::Format { path } => {
            let value = format.read(&path).await?;

            value.write(&path).await?;

            Ok(())
        }
        LedFileCommand::ToCommandData { path } => {
            let value = format.read(&path).await?;

            println!("{}", value.to_command_data());

            Ok(())
        }
        LedFileCommand::Apply { path, no_write } => {
            let value = format.read(&path).await?;

            let mut defy = DefyKeyboard::new()
                .await
                .change_context(Error)
                .attach("connecting to the Defy keyboard")?;

            value.apply(&mut defy).await?;

            if !no_write {
                // Overwrite the file to ensure it remains prettified,
                // exactly like `keymap apply` does.
                value.write(&path).await?;
            }

            Ok(())
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
    ///
    /// The identity probes (`hardware.chip_id`, `version`,
    /// `hardware.wireless`) are best-effort: a probe the device's
    /// `help` output does not list is never sent (the wireless Defy's
    /// firmware does not serve `hardware.wireless`), and a served
    /// probe that fails is recorded rather than fatal. A capture whose
    /// every command was captured saves as a COMPLETE backup even when
    /// identity was unobtainable — with the chip id, versions, and
    /// descriptor fields it did obtain, and a note on stdout naming
    /// what the device did not provide.
    ///
    /// If a capture-list command read fails mid-capture, a PARTIAL
    /// capture is still saved: everything captured so far, marked with
    /// the commands it is missing. `backup restore` refuses partial
    /// files unless `--allow-partial` is passed; complete one with
    /// `--retry-missing`.
    Create {
        /// The path the backup JSON file will be saved to.
        #[clap(default_value = "backup.json")]
        path: PathBuf,
        /// Complete a partial capture instead of capturing afresh: the
        /// file at `path` must be a partial capture, and only the
        /// commands it is missing are captured (they are reads, so
        /// this is safe), merged into the file, which is then
        /// rewritten complete. If the completion is itself
        /// interrupted, the file is rewritten as the updated partial.
        #[clap(long, default_value_t = false)]
        retry_missing: bool,
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
    /// Restore a backup JSON file to the device.
    ///
    /// This **rewrites the captured parts of the keyboard's
    /// configuration** (keymap, superkeys, macros, settings, LED
    /// state). It cannot be undone except by restoring another backup —
    /// capture a fresh one with `backup create` first. The whole backup
    /// is validated offline before anything is sent, so an invalid
    /// backup never touches the device; commands are sent in Bazecor's
    /// restore order (superkeys before the keymap) and `led.mode 0` is
    /// sent last. Note the known Bluetooth limitation: large payloads
    /// fail over BLE, so restore wired or over RF.
    ///
    /// A PARTIAL capture — a file `backup create` saved from an
    /// interrupted run, marked `partial` — is refused: it does not
    /// hold the whole captured state. Pass `--allow-partial` to restore
    /// what the file holds anyway (the refused error and `--dry-run`
    /// both name the missing commands), and prefer completing it first
    /// with `backup create --retry-missing <file>`.
    ///
    /// The backup's neuron object (its names) is applied to the names
    /// store under the connected device's `hardware.chip_id` after the
    /// device restore succeeds — names travel to the Neuron the backup
    /// is restored on, as in Bazecor. When the backup carries no names
    /// but the store already holds names for the connected Neuron, a
    /// warning is printed after connecting and before anything is
    /// sent, and the stored names are overwritten.
    Restore {
        /// The path of the backup JSON file.
        path: PathBuf,
        /// Print the validated restore plan without opening a
        /// connection to the device.
        #[clap(long, default_value_t = false)]
        dry_run: bool,
        /// Allow restoring a partial capture — a backup file carrying
        /// the `partial` marker of an interrupted capture.
        #[clap(long, default_value_t = false)]
        allow_partial: bool,
        /// The path of the names store JSON file.
        ///
        /// Defaults to `$HOME/.dygma-sdk/names.json`.
        #[clap(long, global = true)]
        store: Option<PathBuf>,
    },
    /// Convert a backup JSON file between product wire shapes.
    ///
    /// Rewrites the three product-shaped captured commands
    /// (`keymap.custom`, `colormap.map`, `palette`) from the backup's
    /// product into the target's — exactly the three commands
    /// Bazecor's restore-time conversion rewrites — leaves every other
    /// captured command verbatim, and replaces the backup's device
    /// descriptor with the target product's. A backup converted to the
    /// Defy passes `backup restore` unchanged; a backup converted to
    /// the Sonsei is for Bazecor's restore flow — this SDK restores
    /// Defy backups only. The Sonsei → Defy direction is an SDK
    /// extension (Bazecor blind-restores that pair, writing
    /// 60-position keymap layers to a Defy).
    ///
    /// Raise and Raise 2 targets take `--keyboard-type`: the physical
    /// layout of the TARGET keyboard (the Enter/Shift wire positions
    /// the conversion swaps differ between ANSI and ISO). The source
    /// layout is read from the backup's own descriptor.
    ///
    /// This is fully offline — it never opens a device connection —
    /// and it prints the mapping report: which positions mapped 1:1,
    /// which were defaulted (to transparent, or palette color 0), and
    /// which source positions nothing reads, so nothing is ever
    /// silently guessed. Converting onto the Defy re-enters the keymap
    /// through the exactly-10-layer type, so the output is a valid
    /// `keymap.custom` write by construction.
    Convert {
        /// The path of the backup JSON file, captured from any product
        /// Bazecor backs up.
        path: PathBuf,
        /// The product to convert the backup into.
        #[clap(long)]
        to: ConvertTarget,
        /// The physical key layout of a Raise or Raise 2 target.
        /// Required for `--to raise` / `--to raise2`; not accepted for
        /// other targets.
        #[clap(long)]
        keyboard_type: Option<KeyboardTypeArg>,
        /// The path the converted backup JSON file is written to.
        #[clap(long, required_unless_present = "dry_run")]
        out: Option<PathBuf>,
        /// Print the mapping report without writing any file.
        #[clap(long, default_value_t = false)]
        dry_run: bool,
    },
}

/// The products `backup convert` can target, as CLI arguments.
#[derive(Clone, Copy, Debug, ValueEnum)]
enum ConvertTarget {
    /// The Dygma Defy (the only product this SDK talks to).
    Defy,
    /// The Dygma Sonsei.
    Sonsei,
    /// The Dygma Raise (first generation).
    Raise,
    /// The Dygma Raise 2.
    Raise2,
}

/// The physical key layout of a Raise-generation `backup convert`
/// target, as `--keyboard-type` spells it — the device descriptors'
/// own `ANSI`/`ISO` spelling.
#[derive(Clone, Copy, Debug, ValueEnum)]
enum KeyboardTypeArg {
    #[value(name = "ANSI")]
    Ansi,
    #[value(name = "ISO")]
    Iso,
}

impl From<KeyboardTypeArg> for conversions::KeyboardType {
    fn from(keyboard_type: KeyboardTypeArg) -> Self {
        match keyboard_type {
            KeyboardTypeArg::Ansi => Self::Ansi,
            KeyboardTypeArg::Iso => Self::Iso,
        }
    }
}

/// Error from combining `backup convert`'s target arguments: the
/// keyboard layout is required exactly for the targets that convert by
/// layout, and accepted exactly there.
#[derive(Clone, Debug, Display, Error)]
enum ConvertTargetError {
    /// A Raise/Raise 2 target without a `--keyboard-type`.
    #[display(
        "converting onto a Raise or Raise 2 needs the target keyboard's physical layout: pass `--keyboard-type ANSI` or `--keyboard-type ISO`"
    )]
    KeyboardTypeRequired,
    /// `--keyboard-type` given for a Defy/Sonsei target.
    #[display(
        "`--keyboard-type` only applies to Raise and Raise 2 targets; a {to} keyboard has one layout"
    )]
    KeyboardTypeNotApplicable {
        /// The target whose layout is not a choice.
        to: conversions::Product,
    },
}

/// Resolves `backup convert`'s `--to` and `--keyboard-type` into the
/// conversion target the SDK layer takes — the one place the CLI's
/// "layout required exactly for Raise targets" rule lives.
fn conversion_target(
    to: ConvertTarget,
    keyboard_type: Option<KeyboardTypeArg>,
) -> Result<conversions::ConversionTarget, ConvertTargetError> {
    match to {
        ConvertTarget::Defy => match keyboard_type {
            None => Ok(conversions::ConversionTarget::Defy),
            Some(_) => Err(ConvertTargetError::KeyboardTypeNotApplicable {
                to: conversions::Product::Defy,
            }),
        },
        ConvertTarget::Sonsei => match keyboard_type {
            None => Ok(conversions::ConversionTarget::Sonsei),
            Some(_) => Err(ConvertTargetError::KeyboardTypeNotApplicable {
                to: conversions::Product::Sonsei,
            }),
        },
        ConvertTarget::Raise => match keyboard_type {
            Some(keyboard_type) => Ok(conversions::ConversionTarget::Raise(keyboard_type.into())),
            None => Err(ConvertTargetError::KeyboardTypeRequired),
        },
        ConvertTarget::Raise2 => match keyboard_type {
            Some(keyboard_type) => Ok(conversions::ConversionTarget::Raise2(keyboard_type.into())),
            None => Err(ConvertTargetError::KeyboardTypeRequired),
        },
    }
}

/// Prints the conversion's mapping report: which positions mapped 1:1,
/// which were defaulted, and which source positions nothing reads.
fn describe_conversion_report(report: &conversions::ConversionReport) {
    println!("Conversion: {} → {}.", report.from, report.to);

    describe_position_map(
        "keymap.custom",
        "positions",
        &report.keymap,
        &report.normalized_padding,
    );
    describe_position_map("colormap.map", "LEDs", &report.colormap, &[]);

    println!(
        "palette: {} colors, {}.",
        conversions::PALETTE_COLORS,
        report.palette
    );

    println!(
        "{} other captured commands pass through verbatim.",
        report.passthrough_commands
    );

    println!(
        "The backup's device descriptor is replaced with the {} descriptor.",
        report.to
    );
}

/// Prints one position-mapping summary of a conversion report.
/// `normalized_padding` names defaulted positions the output rewrote —
/// Defy wire-padding slots the keymap type does not model, which
/// serialize as `0` instead of the table's default value — so the
/// printed report states exactly what the output file holds.
fn describe_position_map(
    name: &str,
    noun: &str,
    map: &conversions::PositionMapReport,
    normalized_padding: &[u16],
) {
    println!(
        "{name}: {} layers × {} {noun} (from {} per layer)",
        map.layers, map.target_positions, map.source_positions
    );

    println!("  {} mapped 1:1 per layer", map.mapped_per_layer());

    let defaulted = map.defaulted_positions();
    let kind = match map.default_value {
        conversions::DefaultValue::Transparent => "transparent",
        conversions::DefaultValue::FirstPaletteColor => "palette color 0",
    };

    println!(
        "  {} defaulted to {} ({kind}) per layer{}",
        defaulted.len(),
        map.default_value.value(),
        if defaulted.is_empty() {
            String::new()
        } else {
            format!(
                ": {}",
                defaulted
                    .iter()
                    .map(u16::to_string)
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        }
    );

    if !normalized_padding.is_empty() {
        println!(
            "  note: {} of the defaulted positions per layer are Defy \
             wire-padding slots the keymap type does not model; the \
             output writes them as 0, not {}: {}",
            normalized_padding.len(),
            map.default_value.value(),
            normalized_padding
                .iter()
                .map(u16::to_string)
                .collect::<Vec<_>>()
                .join(" ")
        );
    }

    println!(
        "  {} source positions dropped per layer (no target position reads them)",
        map.dropped_sources.len()
    );
}

impl BackupCommands {
    async fn perform(self) -> Result<(), error_stack::Report<Error>> {
        match self {
            Self::Create {
                path,
                retry_missing,
            } => {
                // The --retry-missing file check is fully offline and
                // runs BEFORE the connection: a nonexistent, unparsable,
                // or complete file fails without ever touching the
                // device.
                let partial = if retry_missing {
                    let partial = read_json_file::<DefyBackup>(&path).await?;

                    if !partial.is_partial() {
                        return Err(Error
                            .into_report()
                            .attach(format!("completing the backup in {}", path.display()))
                            .attach(
                                "the file is not a partial capture: it carries no \
                                 `partial` marker, so there is nothing to complete — \
                                 run `backup create` without --retry-missing",
                            ));
                    }

                    Some(partial)
                } else {
                    None
                };

                let mut defy = DefyKeyboard::new()
                    .await
                    .change_context(Error)
                    .attach("connecting to the Defy keyboard")?;

                // Typed first, report later: the interrupted-capture
                // case carries the partial backup to save, and that
                // payload only exists on the typed error.
                let capture = if let Some(partial) = &partial {
                    defy.complete_backup(partial).await
                } else {
                    defy.create_backup().await
                };

                let capture_context = if retry_missing {
                    format!("completing the partial capture in {}", path.display())
                } else {
                    "capturing the backup from the Defy".to_owned()
                };

                let (backup, identity) = match capture {
                    // The capture was interrupted: save the partial
                    // capture it preserved — recoverable, never
                    // mistaken for complete — and fail with the full
                    // recovery story.
                    Err(CreateBackupError::IncompleteCapture {
                        partial,
                        command,
                        source,
                    }) => {
                        // The partial save itself can fail (a full
                        // disk, a bad path): the write error keeps the
                        // capture story attached instead of discarding
                        // it, so a double failure still narrates the
                        // interrupted capture.
                        if let Err(report) = safe_pretty_json_file(&*partial, &path).await {
                            return Err(report
                                .attach(format!(
                                    "while saving the partial capture: the `{command}` read \
                                     failed: {source}"
                                ))
                                .attach(capture_context));
                        }

                        return Err(diagnostics::interrupted_capture_report(
                            &path,
                            &partial,
                            &command,
                            source,
                            capture_context,
                        ));
                    }
                    Err(err) => {
                        return Err(err
                            .into_report()
                            .change_context(Error)
                            .attach(capture_context));
                    }
                    Ok(complete) => complete,
                };

                safe_pretty_json_file(&backup, &path).await?;

                println!(
                    "Captured {} commands from Neuron {} into {}.",
                    backup.backup.len(),
                    backup.neuron_id,
                    path.display()
                );

                // Evidence honesty: the file just saved is complete
                // (the capture only completes when every command was
                // captured), but identity the device did not provide
                // is said out loud, not left as silent empty fields.
                let unavailable: Vec<String> = identity
                    .unavailable()
                    .map(|reason| reason.to_string())
                    .collect();

                if !unavailable.is_empty() {
                    println!(
                        "note: the device did not provide all identity metadata (the \
                         backup is complete): {}",
                        unavailable.join("; ")
                    );
                }

                // A success return never carries the partial marker
                // (the capture core debug-asserts the invariant), so
                // this output is always a complete capture's.
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

                describe_partial_capture(&backup, &path);

                for entry in &backup.backup {
                    println!("  {} ({} bytes of data)", entry.command, entry.data.len());
                }

                Ok(())
            }
            Self::Restore {
                path,
                dry_run,
                allow_partial,
                store,
            } => {
                // Everything offline first: parse the file, validate the
                // whole backup, load the names store — nothing touches
                // the device until all three succeeded.
                let backup = read_json_file::<DefyBackup>(&path).await?;

                // A partial capture is refused by default; `--allow-partial`
                // is the explicit acknowledgment that the file holds only
                // part of a captured state.
                let plan = if allow_partial {
                    DefyKeyboard::backup_restore_plan_allowing_partial(&backup)
                } else {
                    DefyKeyboard::backup_restore_plan(&backup)
                };

                let plan = match plan {
                    Ok(plan) => plan,
                    Err(err) => {
                        return Err(diagnostics::backup_validation_report(&path, err));
                    }
                };

                let store_path = names_store_path(store.as_deref())?;
                let mut names_store = load_names_store(&store_path).await?;

                if dry_run {
                    println!(
                        "Restore plan for Neuron {} from {} ({} commands, \
                        `led.mode 0` last):",
                        backup.neuron_id,
                        path.display(),
                        plan.len()
                    );

                    for entry in &plan {
                        println!(
                            "  {} ({} bytes of data)",
                            entry.command,
                            entry.data.trim().len()
                        );
                    }

                    describe_partial_capture(&backup, &path);

                    println!("Dry run: nothing was sent, no device connection was opened.");
                    println!(
                        "On a real restore the backup's names are applied to the \
                        names store under the connected device's chip id."
                    );

                    return Ok(());
                }

                // A real --allow-partial restore says what it is
                // about to restore — and what it is not — before any
                // device write: a partial capture's missing list
                // prints here (a complete file prints nothing).
                describe_partial_capture(&backup, &path);

                // The device write itself: default restore refuses a
                // partial capture (the plan above already validated
                // the file under the caller's choice).
                let mut defy = DefyKeyboard::new()
                    .await
                    .change_context(Error)
                    .attach("connecting to the Defy keyboard")?;

                // The connected Neuron's chip id keys the names apply —
                // Bazecor restores names onto the Neuron the backup
                // lands on, whatever Neuron captured the file. Probed
                // before anything is sent so the overwrite warning
                // below fires on the same key; a failed probe aborts
                // with nothing sent.
                let chip_id = defy
                    .get_chip_id()
                    .await
                    .change_context(Error)
                    .attach("reading `hardware.chip_id` to key the names apply")?;

                if names_store.replaces_named_entry(&backup, chip_id.as_str())
                    && let Some(entry) = names_store.get(chip_id.as_str())
                {
                    eprintln!(
                        "Warning: this backup carries no names, but the names store \
                        already holds names for Neuron {} ({}). Restoring will \
                        overwrite them with the backup's empty names.",
                        chip_id,
                        describe_neuron_names(entry)
                    );
                }

                // The device write honors the caller's partial choice:
                // the default refuses (already proven above, so this
                // cannot fire for a non-partial failure), the override
                // sends what the file holds.
                let restore = if allow_partial {
                    defy.restore_backup_allowing_partial(&backup).await
                } else {
                    defy.restore_backup(&backup).await
                };

                restore
                    .change_context(Error)
                    .attach_with(|| format!("restoring the backup in {}", path.display()))?;

                println!(
                    "Restored {} commands to the Defy (`led.mode 0` last).",
                    plan.len()
                );

                // Names apply only after the device restore succeeded:
                // a failed restore leaves the names store untouched.
                // They apply under the connected Neuron's chip id, so a
                // backup captured elsewhere carries its names here.
                names_store.apply_backup(&backup, chip_id.as_str());
                save_names_store(&store_path, &names_store).await?;

                println!(
                    "Names for Neuron {} written to {}.",
                    chip_id,
                    store_path.display()
                );

                if chip_id.as_str() != backup.neuron_id {
                    println!(
                        "Note: the backup was captured from Neuron {}; its names \
                        were carried to the connected Neuron {chip_id}.",
                        backup.neuron_id
                    );
                }

                Ok(())
            }
            Self::Convert {
                path,
                to,
                keyboard_type,
                out,
                dry_run,
            } => {
                // The target arguments are validated before the file
                // is touched: a `--keyboard-type` misuse has nothing
                // to do with the file's contents.
                let target = match conversion_target(to, keyboard_type) {
                    Ok(target) => target,
                    Err(err) => {
                        return Err(err
                            .into_report()
                            .change_context(Error)
                            .attach(format!("converting the backup in {}", path.display())));
                    }
                };

                // Fully offline: parse the file and convert it — no
                // device connection is ever opened.
                let backup = read_json_file::<DefyBackup>(&path).await?;

                let (converted, report) = match conversions::convert_backup(&backup, target) {
                    Ok(result) => result,
                    Err(err) => return Err(diagnostics::conversion_report(&path, err)),
                };

                describe_conversion_report(&report);

                // The marker is data, not damage: the converted file
                // keeps it (the conversion never adds the missing
                // commands), so a converted partial stays unmistakable
                // everywhere the file travels.
                if let Some(partial) = &backup.partial {
                    println!(
                        "Warning: this backup is a PARTIAL capture ({} command(s) \
                         missing); the converted file keeps the partial marker.",
                        partial.missing.len()
                    );
                }

                if dry_run {
                    println!("Dry run: nothing was written, no device connection was opened.");

                    return Ok(());
                }

                let out = out
                    .unwrap_or_else(|| unreachable!("clap requires --out unless --dry-run is set"));

                safe_pretty_json_file(&converted, &out).await?;

                println!("Converted backup written to {}.", out.display());

                Ok(())
            }
        }
    }
}

/// The target of a `names set` write.
#[derive(Subcommand)]
enum NameTarget {
    /// The keyboard's user-chosen name.
    KeyboardName {
        /// The name to set.
        name: String,
    },
    /// One layer's name. Layer ids are 0-based, as the device and
    /// Bazecor number them.
    Layer {
        /// The layer's id.
        id: u8,
        /// The name to set.
        name: String,
    },
}

#[derive(Subcommand)]
enum NamesCommands {
    /// Lists every neuron entry in the names store.
    ///
    /// This never opens a connection to the device.
    List,
    /// Shows one neuron's stored names entry.
    ///
    /// This never opens a connection to the device.
    Show {
        /// The Neuron's `hardware.chip_id`.
        chip_id: String,
    },
    /// Imports a backup file's neuron object (its names) into the
    /// names store.
    ///
    /// The store entry keyed by the backup's `neuronID` is replaced
    /// wholesale with the backup's neuron. Unlike `backup restore`,
    /// which applies names under the connected device's chip id, this
    /// offline import keys the entry by the backup file's own
    /// `neuronID`. When the backup carries no names but the store
    /// already holds names for that Neuron, a warning is printed and
    /// the stored names are overwritten.
    ///
    /// This never opens a connection to the device.
    Import {
        /// The path of the backup JSON file.
        path: PathBuf,
    },
    /// Sets a name on a neuron's stored entry, creating the entry if
    /// the store does not hold one for this chip id yet.
    ///
    /// This never opens a connection to the device.
    Set {
        /// The Neuron's `hardware.chip_id`.
        chip_id: String,
        #[command(subcommand)]
        target: NameTarget,
    },
}

/// One human-readable line describing what names a neuron entry
/// carries, for lists and warnings.
fn describe_neuron_names(neuron: &parsing::backup::BackupNeuron) -> String {
    match (
        neuron.name.is_empty(),
        neuron.layers.len(),
        neuron.macros.len(),
        neuron.superkeys.len(),
    ) {
        (true, 0, 0, 0) => "no names".to_owned(),
        _ => format!(
            "\"{}\", {} layers, {} macros, {} superkeys named",
            neuron.name,
            neuron.layers.len(),
            neuron.macros.len(),
            neuron.superkeys.len()
        ),
    }
}

/// Prints the partial-capture description of a backup file: the
/// missing commands (or the why-none line for an empty missing list)
/// and the two recovery paths. Shared by `backup show`, the
/// `backup restore --dry-run` output, and the real `--allow-partial`
/// restore — every path that acts on a partial file says exactly
/// what it is missing first.
fn describe_partial_capture(backup: &DefyBackup, path: &Path) {
    let Some(partial) = &backup.partial else {
        return;
    };

    println!(
        "PARTIAL CAPTURE: this file was saved from an interrupted `backup create` \
         and is missing {} command(s):",
        partial.missing.len()
    );

    if partial.missing.is_empty() {
        println!(
            "  (none — the capture stopped after every command, before the \
             backup could be assembled completely)"
        );
    } else {
        for command in &partial.missing {
            println!("  {command}");
        }
    }

    println!(
        "  Complete it with `backup create --retry-missing {}`, or pass \
         --allow-partial to restore it anyway.",
        path.display()
    );
}

impl NamesCommands {
    async fn perform(self, store: Option<&Path>) -> Result<(), error_stack::Report<Error>> {
        match self {
            Self::List => {
                let store_path = names_store_path(store)?;
                let store = load_names_store(&store_path).await?;

                if store.is_empty() {
                    println!("The names store at {} is empty.", store_path.display());

                    return Ok(());
                }

                for (chip_id, neuron) in store.iter() {
                    println!("{chip_id}  {}", describe_neuron_names(neuron));
                }

                Ok(())
            }
            Self::Show { chip_id } => {
                let store_path = names_store_path(store)?;
                let store = load_names_store(&store_path).await?;

                let neuron = store.get(&chip_id).ok_or_else(|| {
                    Error
                        .into_report()
                        .attach(format!("no names stored for Neuron `{chip_id}`"))
                })?;

                let rendered = serde_json::to_string_pretty(neuron)
                    .change_context(Error)
                    .attach("serializing the stored names entry")
                    .attach_with(|| store_path.to_string_lossy().into_owned())?;

                println!("{rendered}");

                Ok(())
            }
            Self::Import { path } => {
                let backup = read_json_file::<DefyBackup>(&path).await?;

                let store_path = names_store_path(store)?;
                let mut store = load_names_store(&store_path).await?;

                // Offline import: keyed by the backup file's own
                // `neuronID`, unlike the device-backed `backup restore`.
                if store.replaces_named_entry(&backup, &backup.neuron_id)
                    && let Some(entry) = store.get(&backup.neuron_id)
                {
                    eprintln!(
                        "Warning: this backup carries no names, but the names store \
                        already holds names for Neuron {} ({}). Importing will \
                        overwrite them with the backup's empty names.",
                        backup.neuron_id,
                        describe_neuron_names(entry)
                    );
                }

                store.apply_backup(&backup, &backup.neuron_id);
                save_names_store(&store_path, &store).await?;

                println!(
                    "Names for Neuron {} imported from {} into {}.",
                    backup.neuron_id,
                    path.display(),
                    store_path.display()
                );

                Ok(())
            }
            Self::Set { chip_id, target } => {
                let store_path = names_store_path(store)?;
                let mut store = load_names_store(&store_path).await?;

                match target {
                    NameTarget::KeyboardName { name } => {
                        store.set_keyboard_name(&chip_id, &name);

                        println!("Keyboard name \"{name}\" set for Neuron {chip_id}.");
                    }
                    NameTarget::Layer { id, name } => {
                        store.set_layer_name(&chip_id, id, &name);

                        println!("Layer {id} named \"{name}\" for Neuron {chip_id}.");
                    }
                }

                save_names_store(&store_path, &store).await?;

                println!("Names written to {}.", store_path.display());

                Ok(())
            }
        }
    }
}

/// Resolves the names store path: `--store` when given, else
/// `$HOME/.dygma-sdk/names.json`.
fn names_store_path(store: Option<&Path>) -> Result<PathBuf, error_stack::Report<Error>> {
    if let Some(store) = store {
        return Ok(store.to_owned());
    }

    std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join(".dygma-sdk").join("names.json"))
        .ok_or_else(|| {
            Error
                .into_report()
                .attach("resolving the names store path; pass --store or set $HOME")
        })
}

/// Loads the names store, treating a missing file as an empty store.
/// Any other failure to stat the file (permissions, I/O) surfaces as
/// an error instead of silently reading as an empty store.
async fn load_names_store(
    path: &Path,
) -> Result<dygma_cli::names::NeuronNameStore, error_stack::Report<Error>> {
    match tokio::fs::metadata(path).await {
        Err(err) if err.kind() == ErrorKind::NotFound => {
            Ok(dygma_cli::names::NeuronNameStore::new())
        }
        Err(err) => Err(err
            .into_report()
            .change_context(Error)
            .attach("statting the names store")
            .attach(path.to_string_lossy().into_owned())),
        Ok(_) => read_json_file(path).await,
    }
}

/// Saves the names store, creating its parent directory if needed.
async fn save_names_store(
    path: &Path,
    store: &dygma_cli::names::NeuronNameStore,
) -> Result<(), error_stack::Report<Error>> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .change_context(Error)
            .attach("creating the names store directory")
            .attach_with(|| parent.to_string_lossy().into_owned())?;
    }

    safe_pretty_json_file(store, path).await
}

/// A parsed setting write: the value has been validated from its
/// command-line string, and only the device round-trip remains.
/// (`Copy` was dropped when the wireless writes joined: the device
/// name carries a `String`, and each write is consumed exactly once
/// anyway.)
#[derive(Clone, Debug)]
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
    LedBrightness(u8),
    LedBrightnessUG(u8),
    LedBrightnessWireless(u8),
    LedBrightnessUGWireless(u8),
    LedFade(bool),
    IdleLedsTimeLimit(u16),
    IdleLedsWireless(u16),
    TrueSleep(bool),
    TrueSleepTime(u16),
    BatterySavingMode(bool),
    RfPower(parsing::wireless::RfPower),
    RfChannelHop(bool),
    BluetoothDeviceName(parsing::wireless::DeviceName),
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
        WritableSetting::LedBrightness => SettingWrite::LedBrightness(
            parse_u8_value(value)
                .change_context(Error)
                .attach("parsing the value; expected a number 0-255")?,
        ),
        WritableSetting::LedBrightnessUG => SettingWrite::LedBrightnessUG(
            parse_u8_value(value)
                .change_context(Error)
                .attach("parsing the value; expected a number 0-255")?,
        ),
        WritableSetting::LedBrightnessWireless => SettingWrite::LedBrightnessWireless(
            parse_u8_value(value)
                .change_context(Error)
                .attach("parsing the value; expected a number 0-255")?,
        ),
        WritableSetting::LedBrightnessUGWireless => SettingWrite::LedBrightnessUGWireless(
            parse_u8_value(value)
                .change_context(Error)
                .attach("parsing the value; expected a number 0-255")?,
        ),
        WritableSetting::LedFade => SettingWrite::LedFade(
            parse_bool_value(value)
                .change_context(Error)
                .attach("parsing the value; expected 1/0/true/false")?,
        ),
        WritableSetting::IdleLedsTimeLimit => SettingWrite::IdleLedsTimeLimit(
            parse_u16_value(value)
                .change_context(Error)
                .attach("parsing the value; expected seconds (`0` disables the timeout)")?,
        ),
        WritableSetting::IdleLedsWireless => SettingWrite::IdleLedsWireless(
            parse_u16_value(value)
                .change_context(Error)
                .attach("parsing the value; expected seconds (`0` disables the timeout)")?,
        ),
        WritableSetting::TrueSleep => SettingWrite::TrueSleep(
            parse_bool_value(value)
                .change_context(Error)
                .attach("parsing the value; expected 1/0/true/false")?,
        ),
        WritableSetting::TrueSleepTime => SettingWrite::TrueSleepTime(
            parse_u16_value(value)
                .change_context(Error)
                .attach("parsing the value; expected seconds")?,
        ),
        WritableSetting::BatterySavingMode => SettingWrite::BatterySavingMode(
            parse_bool_value(value)
                .change_context(Error)
                .attach("parsing the value; expected 1/0/true/false")?,
        ),
        WritableSetting::RfPower => SettingWrite::RfPower(
            parse_rf_power_value(value)
                .change_context(Error)
                .attach("parsing the value; expected 0 (low), 1 (medium), or 2 (high)")?,
        ),
        WritableSetting::RfChannelHop => SettingWrite::RfChannelHop(
            parse_bool_value(value)
                .change_context(Error)
                .attach("parsing the value; expected 1/0/true/false")?,
        ),
        WritableSetting::BluetoothDeviceName => {
            SettingWrite::BluetoothDeviceName(parsing::wireless::DeviceName::new(value))
        }
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
            Self::LedBrightness(_) => "led.brightness",
            Self::LedBrightnessUG(_) => "led.brightnessUG",
            Self::LedBrightnessWireless(_) => "led.brightness.wireless",
            Self::LedBrightnessUGWireless(_) => "led.brightnessUG.wireless",
            Self::LedFade(_) => "led.fade",
            Self::IdleLedsTimeLimit(_) => "idleleds.time_limit",
            Self::IdleLedsWireless(_) => "idleleds.wireless",
            Self::TrueSleep(_) => "idleleds.true_sleep",
            Self::TrueSleepTime(_) => "idleleds.true_sleep_time",
            Self::BatterySavingMode(_) => "wireless.battery.savingMode",
            Self::RfPower(_) => "wireless.rf.power",
            Self::RfChannelHop(_) => "wireless.rf.channelHop",
            Self::BluetoothDeviceName(_) => "wireless.bluetooth.deviceName",
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
            Self::LedBrightness(value) => defy
                .set_led_brightness(value)
                .await
                .change_context(Error)
                .attach("writing `led.brightness` to the Defy"),
            Self::LedBrightnessUG(value) => defy
                .set_led_underglow_brightness(value)
                .await
                .change_context(Error)
                .attach("writing `led.brightnessUG` to the Defy"),
            Self::LedBrightnessWireless(value) => defy
                .set_wireless_led_brightness(value)
                .await
                .change_context(Error)
                .attach("writing `led.brightness.wireless` to the Defy"),
            Self::LedBrightnessUGWireless(value) => defy
                .set_wireless_led_underglow_brightness(value)
                .await
                .change_context(Error)
                .attach("writing `led.brightnessUG.wireless` to the Defy"),
            Self::LedFade(value) => defy
                .set_led_fade(value)
                .await
                .change_context(Error)
                .attach("writing `led.fade` to the Defy"),
            Self::IdleLedsTimeLimit(value) => defy
                .set_idle_leds_time_limit(value)
                .await
                .change_context(Error)
                .attach("writing `idleleds.time_limit` to the Defy"),
            Self::IdleLedsWireless(value) => defy
                .set_wireless_idle_leds_time_limit(value)
                .await
                .change_context(Error)
                .attach("writing `idleleds.wireless` to the Defy"),
            Self::TrueSleep(value) => defy
                .set_true_sleep(value)
                .await
                .change_context(Error)
                .attach("writing `idleleds.true_sleep` to the Defy"),
            Self::TrueSleepTime(value) => defy
                .set_true_sleep_time(value)
                .await
                .change_context(Error)
                .attach("writing `idleleds.true_sleep_time` to the Defy"),
            Self::BatterySavingMode(value) => defy
                .set_battery_saving_mode(value)
                .await
                .change_context(Error)
                .attach("writing `wireless.battery.savingMode` to the Defy"),
            Self::RfPower(value) => defy
                .set_rf_power(value)
                .await
                .change_context(Error)
                .attach("writing `wireless.rf.power` to the Defy"),
            Self::RfChannelHop(value) => defy
                .set_rf_channel_hop(value)
                .await
                .change_context(Error)
                .attach("writing `wireless.rf.channelHop` to the Defy"),
            Self::BluetoothDeviceName(value) => defy
                .set_bluetooth_device_name(&value)
                .await
                .change_context(Error)
                .attach("writing `wireless.bluetooth.deviceName` to the Defy"),
        }
    }
}

/// Parses an RF power level from a `0`/`1`/`2` value string.
fn parse_rf_power_value(value: &str) -> Result<parsing::wireless::RfPower, Error> {
    value.parse().map_err(|_| Error)
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
async fn main() {
    if let Err(report) = Cli::parse().perform().await {
        eprint!("{}", diagnostics::render(&report));

        std::process::exit(1);
    }
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
    T: for<'de> serde::Deserialize<'de> + diagnostics::JsonDomainHint,
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

    match serde_json::from_reader::<_, T>(data.as_slice()) {
        Ok(value) => Ok(value),
        Err(err) => Err(diagnostics::json_parse_report::<T>(path, &data, err)),
    }
}

/// Computes the sibling temp-file path used to stage an atomic write
/// of `path`: a dot-prefixed sibling of the destination whose name
/// embeds the process id, so concurrent writers from different
/// processes cannot collide. Staging in the same directory keeps the
/// final `rename` within one filesystem, which is what makes it
/// atomic.
fn temp_sibling_path(path: &Path) -> Result<PathBuf, error_stack::Report<Error>> {
    let file_name = path.file_name().ok_or_else(|| {
        Error.into_report().attach(format!(
            "computing the temp-file path for `{}`; the path has no file name",
            path.to_string_lossy()
        ))
    })?;

    Ok(path.with_file_name(format!(
        ".{}.tmp-{}",
        file_name.to_string_lossy(),
        std::process::id()
    )))
}

/// Serializes `data` to pretty JSON and writes it to `path` atomically:
/// the bytes are staged in a sibling temp file (see
/// [`temp_sibling_path`]) and only then renamed over the destination, so
/// a mid-write failure (a full disk, a crash) can never leave the
/// destination truncated — it holds either the old content or the new
/// content, never a mix. On any failure after serialization the temp
/// file is removed (best-effort), and the original error is returned
/// unmasked.
async fn safe_pretty_json_file<T>(data: &T, path: &Path) -> Result<(), error_stack::Report<Error>>
where
    T: serde::Serialize,
{
    // Serialize BEFORE touching the filesystem (#21): a serialization
    // failure must leave the destination untouched.
    let data = serde_json::to_vec_pretty(data)
        .change_context(Error)
        .attach("serializing data")
        .attach_with(|| path.to_string_lossy().into_owned())?;

    let temp_path = temp_sibling_path(path)?;

    let result = async {
        let file = File::create(&temp_path)
            .await
            .change_context(Error)
            .attach("creating temp file")
            .attach_with(|| temp_path.to_string_lossy().into_owned())?;

        let mut writer = BufWriter::new(file);

        writer
            .write_all(&data)
            .await
            .change_context(Error)
            .attach("writing data to the temp file")
            .attach_with(|| temp_path.to_string_lossy().into_owned())?;

        writer
            .flush()
            .await
            .change_context(Error)
            .attach("flushing data to the temp file")
            .attach_with(|| temp_path.to_string_lossy().into_owned())?;

        // `rename` atomically replaces the destination within one
        // filesystem on macOS/Linux, so the switch from old to new
        // content cannot be observed half-done. Windows replaces an
        // existing destination FILE too (`std::fs::rename` — and
        // tokio's async wrapper of it — corresponds to MoveFileExW
        // with MOVEFILE_REPLACE_EXISTING, falling back to
        // SetFileInformationByHandle; on Windows 10 1607+ whose
        // filesystem supports FileRenameInfoEx the behavior matches
        // Unix) — only replacing a directory is forbidden there, so
        // the atomic replace works on all three platforms.
        //
        // Atomic replace also trades in-place-write semantics: the
        // renamed-in file carries fresh umask permissions/ownership
        // rather than the previous destination's, and a symlinked
        // destination is replaced rather than written through.
        tokio::fs::rename(&temp_path, path)
            .await
            .change_context(Error)
            .attach("renaming temp file over the destination")
            .attach_with(|| path.to_string_lossy().into_owned())
    }
    .await;

    if result.is_err() {
        // Best-effort cleanup of the staged file; the original error is
        // returned unmasked.
        let _ = tokio::fs::remove_file(&temp_path).await;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The `--keyboard-type` rule: required exactly for Raise and
    /// Raise 2 targets, rejected exactly elsewhere, and resolved into
    /// the SDK's conversion target with the descriptor's layout
    /// spelling.
    #[test]
    fn conversion_target_resolves_the_keyboard_type_rule() {
        // Defy and Sonsei take no layout argument.
        assert_eq!(
            conversion_target(ConvertTarget::Defy, None).unwrap(),
            conversions::ConversionTarget::Defy
        );
        assert_eq!(
            conversion_target(ConvertTarget::Sonsei, None).unwrap(),
            conversions::ConversionTarget::Sonsei
        );
        assert!(matches!(
            conversion_target(ConvertTarget::Defy, Some(KeyboardTypeArg::Ansi)),
            Err(ConvertTargetError::KeyboardTypeNotApplicable {
                to: conversions::Product::Defy
            })
        ));
        assert!(matches!(
            conversion_target(ConvertTarget::Sonsei, Some(KeyboardTypeArg::Iso)),
            Err(ConvertTargetError::KeyboardTypeNotApplicable {
                to: conversions::Product::Sonsei
            })
        ));

        // Raise and Raise 2 require it, in the descriptors' spelling.
        assert_eq!(
            conversion_target(ConvertTarget::Raise, Some(KeyboardTypeArg::Ansi)).unwrap(),
            conversions::ConversionTarget::Raise(conversions::KeyboardType::Ansi)
        );
        assert_eq!(
            conversion_target(ConvertTarget::Raise2, Some(KeyboardTypeArg::Iso)).unwrap(),
            conversions::ConversionTarget::Raise2(conversions::KeyboardType::Iso)
        );
        assert!(matches!(
            conversion_target(ConvertTarget::Raise, None),
            Err(ConvertTargetError::KeyboardTypeRequired)
        ));
        assert!(matches!(
            conversion_target(ConvertTarget::Raise2, None),
            Err(ConvertTargetError::KeyboardTypeRequired)
        ));

        // The misuse errors render through the same #16 diagnostics
        // exit the conversion failures do (the exact construction the
        // `Convert` arm runs): the message and the command context are
        // the story, and there is no file snippet to invent. The
        // rendered chain wraps at the pinned width, so the content
        // assertions normalize the wrapping away.
        for err in [
            conversion_target(ConvertTarget::Raise, None).unwrap_err(),
            conversion_target(ConvertTarget::Defy, Some(KeyboardTypeArg::Iso)).unwrap_err(),
        ] {
            let report = err
                .clone()
                .into_report()
                .change_context(Error)
                .attach("converting the backup in backup.json");
            let flat = diagnostics::render(&report)
                .split('│')
                .flat_map(|part| part.split_whitespace())
                .collect::<Vec<_>>()
                .join(" ");

            assert!(flat.contains(&err.to_string()));
            assert!(flat.contains("converting the backup in backup.json"));
        }
    }

    /// A type whose [`serde::Serialize`] implementation always fails
    /// deterministically.
    #[derive(Debug)]
    struct Unserializable;

    impl serde::Serialize for Unserializable {
        fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::ser::Serializer,
        {
            Err(serde::ser::Error::custom(
                "unserializable by design (issue #21 test fixture)",
            ))
        }
    }

    #[tokio::test]
    async fn serialize_failure_leaves_the_destination_untouched() {
        let directory =
            std::env::temp_dir().join(format!("dygma-cli-issue-21-test-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();

        let path = directory.join("destination.json");
        let original = b"{\"sentinel\": true}";
        std::fs::write(&path, original).unwrap();

        let result = safe_pretty_json_file(&Unserializable, &path).await;

        assert!(result.is_err());

        let contents = std::fs::read(&path).unwrap();
        assert_eq!(contents, original);

        std::fs::remove_file(&path).unwrap();
        std::fs::remove_dir(&directory).unwrap();
    }

    #[tokio::test]
    async fn serializes_pretty_json_to_the_destination() {
        let directory = std::env::temp_dir().join(format!(
            "dygma-cli-issue-21-happy-path-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();

        let path = directory.join("destination.json");
        let data = serde_json::json!({ "sentinel": true });

        safe_pretty_json_file(&data, &path).await.unwrap();

        let contents = std::fs::read_to_string(&path).unwrap();
        assert_eq!(contents, serde_json::to_string_pretty(&data).unwrap());

        std::fs::remove_file(&path).unwrap();
        std::fs::remove_dir(&directory).unwrap();
    }

    #[tokio::test]
    async fn serialize_failure_never_creates_the_destination() {
        let directory = std::env::temp_dir().join(format!(
            "dygma-cli-issue-21-no-destination-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();

        let path = directory.join("destination.json");

        let result = safe_pretty_json_file(&Unserializable, &path).await;

        assert!(result.is_err());
        assert!(!path.exists());

        std::fs::remove_dir(&directory).unwrap();
    }

    #[tokio::test]
    async fn atomic_write_replaces_the_destination_and_leaves_no_temp_files() {
        let directory = std::env::temp_dir().join(format!(
            "dygma-cli-issue-22-happy-path-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();

        let path = directory.join("destination.json");
        std::fs::write(&path, b"{\"sentinel\": \"old\"}").unwrap();

        let data = serde_json::json!({ "sentinel": "new" });

        safe_pretty_json_file(&data, &path).await.unwrap();

        let contents = std::fs::read_to_string(&path).unwrap();
        assert_eq!(contents, serde_json::to_string_pretty(&data).unwrap());

        let staged = temp_sibling_path(&path).unwrap();
        assert!(
            !staged.exists(),
            "temp file left behind: {}",
            staged.display()
        );

        std::fs::remove_file(&path).unwrap();
        std::fs::remove_dir(&directory).unwrap();
    }

    #[tokio::test]
    async fn temp_file_creation_failure_leaves_the_destination_untouched() {
        let directory = std::env::temp_dir().join(format!(
            "dygma-cli-issue-22-temp-create-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();

        let path = directory.join("destination.json");
        let original = b"{\"sentinel\": true}";
        std::fs::write(&path, original).unwrap();

        // A directory squatting on the computed temp path forces the
        // temp-file creation failure deterministically — no timing, no
        // permissions tricks.
        let temp_path = temp_sibling_path(&path).unwrap();
        std::fs::create_dir(&temp_path).unwrap();

        let result = safe_pretty_json_file(&serde_json::json!({ "sentinel": 1 }), &path).await;

        assert!(result.is_err());

        let contents = std::fs::read(&path).unwrap();
        assert_eq!(contents, original);

        // The write neither replaced the destination nor disturbed the
        // fixture directory sitting on the temp path.
        assert!(temp_path.is_dir());

        std::fs::remove_dir(&temp_path).unwrap();
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_dir(&directory).unwrap();
    }

    #[tokio::test]
    async fn rename_failure_leaves_the_destination_intact_and_removes_the_temp_file() {
        let directory = std::env::temp_dir().join(format!(
            "dygma-cli-issue-22-rename-failure-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();

        // A directory at the destination makes `rename` fail (EISDIR)
        // after the temp file has been fully written — exercising the
        // failure cleanup deterministically.
        let path = directory.join("destination.json");
        std::fs::create_dir(&path).unwrap();

        let result = safe_pretty_json_file(&serde_json::json!({ "sentinel": true }), &path).await;

        assert!(result.is_err());
        assert!(path.is_dir());

        let staged = temp_sibling_path(&path).unwrap();
        assert!(
            !staged.exists(),
            "temp file left behind: {}",
            staged.display()
        );

        std::fs::remove_dir(&path).unwrap();
        std::fs::remove_dir(&directory).unwrap();
    }

    #[test]
    fn brightness_percent_is_bazecors_round_half_up_for_every_byte() {
        // Bazecor shows `Math.round((value * 100) / 255)` (round half
        // up), which in exact arithmetic is `floor((v·200 + 255) /
        // 510)`. The integer form here equals `floor((v·200 + 254) /
        // 510)`; the two can only diverge when some `510n` lands in
        // `(v·200 + 254, v·200 + 255]`, i.e. `510n = v·200 + 255` — an
        // even number equalling an odd one, impossible by parity — so
        // every byte must agree.
        let round_half_up = |v: u8| (u16::from(v) * 200 + 255) / 510;

        for v in u8::MIN..=u8::MAX {
            assert_eq!(
                brightness_percent(v),
                round_half_up(v),
                "brightness {v} rounds differently than Bazecor"
            );
        }

        // Pin the endpooints and a mid value against the UI mapping.
        assert_eq!(brightness_percent(0), 0);
        assert_eq!(brightness_percent(128), 50);
        assert_eq!(brightness_percent(255), 100);
    }

    #[test]
    fn describe_minutes_names_only_whole_minutes() {
        assert_eq!(describe_minutes(0), " (0 min)");
        assert_eq!(describe_minutes(60), " (1 min)");
        assert_eq!(describe_minutes(300), " (5 min)");
        assert_eq!(describe_minutes(7200), " (120 min)");

        // Anything not a whole minute gets no suffix — `leds status`
        // already printed the seconds.
        assert_eq!(describe_minutes(1), "");
        assert_eq!(describe_minutes(90), "");
        assert_eq!(describe_minutes(u16::MAX), "");
    }
}
