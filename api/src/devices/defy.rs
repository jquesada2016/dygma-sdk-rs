//! Provides the [`DefyKeyboard`] struct for programatically interacting with
//! the keyboard.

use super::colormap_serde::colormap_layer_serde;
use super::probes::probe_identity;
use crate::{
    focus_api::{
        CreateHidFocusApiError, FocusApiConnection, GetCommandsError, HidFocusApi, RunCommandError,
        SerialPortFocusApi, identities, parsing,
    },
    keycode_tables::KeyKind,
};
pub use error::*;
use parsing::colormap::{KEYBOARD_LEDS, UNDERGLOW_LEDS};
use std::{array, str::FromStr};

mod error {
    use super::*;

    /// Error returned when creating a handle to the keyboard.
    #[derive(Debug, Display, From, Error)]
    #[display("failed to create handle to the Dygma Defy keyboard: {_0}")]
    pub struct CreateDefyKeyboardError(CreateHidFocusApiError);

    /// Error when parsing a keymap from a string slice.
    #[derive(Clone, Debug, Display, From, Error)]
    pub enum ParseKeymapError {
        /// The keymap data is not valid wire data.
        #[display("failed to parse keymap: {_0}")]
        MalformedData(parsing::keymap::ParseKeymapError),
        /// The keymap data is valid wire data but does not hold exactly
        /// the ten layers every Defy keymap holds.
        #[display("{_0}")]
        IncorrectLayerCount(KeymapDoesNotHave10LayersError),
    }

    /// Error when parsing a superkey map from a string slice.
    #[derive(Clone, Debug, Display, From, Error)]
    pub enum ParseSuperkeyMapError {
        /// The superkey map data is not valid wire data.
        #[display("failed to parse superkeys map: {_0}")]
        MalformedData(parsing::superkeys::ParseSuperkeyMapError),
        /// The map holds more superkeys than the firmware can address.
        #[display("{_0}")]
        TooManySuperkeys(SuperkeyMapTooLargeError),
    }

    /// Error when parsing a macro map from a string slice.
    #[derive(Clone, Debug, Display, From, Error)]
    pub enum ParseMacroMapError {
        /// The macro map data is not valid wire data.
        #[display("failed to parse macros map: {_0}")]
        MalformedData(parsing::macros::ParseMacroMapError),
        /// The map holds more macros than the firmware can address.
        #[display("{_0}")]
        TooManyMacros(MacroMapTooLargeError),
    }

    /// Error when parsing a colormap from a string slice.
    #[derive(Clone, Debug, Display, From, Error)]
    pub enum ParseColormapError {
        /// The colormap data is not valid wire data.
        #[display("failed to parse colormap map: {_0}")]
        MalformedData(parsing::colormap::ParseColormapError),
        /// The colormap data is valid wire data but does not hold
        /// exactly the ten layers every Defy colormap holds.
        #[display("{_0}")]
        IncorrectLayerCount(ColormapDoesNotHave10LayersError),
    }

    /// Error returned when building a [`DefyColormap`] from anything
    /// but exactly 10 layers (the number every Defy colormap holds and
    /// every `colormap.map` write carries).
    #[derive(Clone, Copy, Debug, Display, Error)]
    #[display("colormap does not have exactly 10 layers")]
    pub struct ColormapDoesNotHave10LayersError;

    /// Error returned when a [`MacroMap`] holds more macros than the
    /// firmware can address.
    ///
    /// The firmware triggers macros through a keycode range exactly 128
    /// codes wide (`DYNAMIC_MACRO_FIRST` to `DYNAMIC_MACRO_LAST` in the
    /// firmware's `Kaleidoscope-Ranges.h`), so macros past that count
    /// could never be triggered. Its macro position cache is a 127-entry
    /// array (`map_` in the firmware's `DynamicMacros.h`) indexed by a
    /// `u8`, so an oversized map corrupts it well before the `u8` wraps.
    #[derive(Clone, Copy, Debug, Display, Error)]
    #[display(
        "the macro map holds more than {} macros — beyond what the firmware can address",
        MacroMap::MAX_MACROS
    )]
    pub struct MacroMapTooLargeError;

    /// Error returned when a [`SuperkeyMap`] holds more superkeys than
    /// the firmware can address.
    ///
    /// The firmware triggers superkeys through a keycode range exactly
    /// 128 codes wide (`DYNAMIC_SUPER_FIRST` to `DYNAMIC_SUPER_LAST` in
    /// the firmware's `Kaleidoscope-Ranges.h`), so superkeys past that
    /// count could never be triggered.
    #[derive(Clone, Copy, Debug, Display, Error)]
    #[display(
        "the superkey map holds more than {} superkeys — beyond what the firmware can address",
        SuperkeyMap::MAX_SUPERKEYS
    )]
    pub struct SuperkeyMapTooLargeError;

    /// Error returned when building a [`DefyKeymap`] from anything but
    /// exactly 10 layers (the number every Defy keymap holds and every
    /// `keymap.custom` write carries).
    #[derive(Clone, Copy, Debug, Display, Error)]
    #[display("keymap does not have exactly 10 layers")]
    pub struct KeymapDoesNotHave10LayersError;

    /// Error returned from [`DefyKeyboard::apply_custom_keymap`].
    #[derive(Debug, Display, From, Error)]
    #[display("{_0}")]
    pub struct ApplyCustomKeymapError(RunCommandError);

    /// Error returned from [`DefyKeyboard::get_custom_keymap`].
    #[derive(Debug, Display, From, Error)]
    pub enum GetCustomKeymapError {
        /// Failed to run command.
        #[display("{_0}")]
        CommandFailed(RunCommandError),
        /// Keymap returned by the keyboard failed to parse.
        KeymapParsingFailure(ParseKeymapError),
    }

    /// Error returned from [`DefyKeyboard::get_palette`].
    #[derive(Debug, Display, From, Error)]
    pub enum GetPaletteError {
        /// Failed to run command.
        #[display("{_0}")]
        CommandFailed(RunCommandError),
        /// Palette returned by the keyboard failed to parse.
        #[display("{_0}")]
        PaletteParsingFailure(parsing::palette::ParsePaletteError),
    }

    /// Error returned from [`DefyKeyboard::apply_palette`].
    #[derive(Debug, Display, From, Error)]
    #[display("{_0}")]
    pub struct ApplyPaletteError(RunCommandError);

    /// Error returned from [`DefyKeyboard::get_colormap`].
    #[derive(Debug, Display, From, Error)]
    pub enum GetColormapError {
        /// Failed to run command.
        #[display("{_0}")]
        CommandFailed(RunCommandError),
        /// Colormap returned by the keyboard failed to parse.
        ColormapParsingFailure(ParseColormapError),
    }

    /// Error returned from [`DefyKeyboard::apply_colormap`].
    #[derive(Debug, Display, From, Error)]
    #[display("{_0}")]
    pub struct ApplyColormapError(RunCommandError);

    /// Error returned from [`DefyKeyboard::get_superkeys`].
    #[derive(Debug, Display, From, Error)]
    pub enum GetSuperkeyMapError {
        /// Failed to run command.
        #[display("{_0}")]
        CommandFailed(RunCommandError),
        /// Keymap returned by the keyboard failed to parse.
        KeymapParsingFailure(ParseSuperkeyMapError),
    }

    /// Error returned from [`DefyKeyboard::apply_superkeys`].
    #[derive(Debug, Display, From, Error)]
    pub enum ApplySuperkeyError {
        /// Too many superkeys were used.
        #[display("{_0}")]
        TooManySuperkeys(parsing::superkeys::TooManySuperkeysError),
        /// Command failed to run.
        #[display("{_0}")]
        CommandFailed(RunCommandError),
    }

    /// Error returned from [`DefyKeyboard::get_macros`].
    #[derive(Debug, Display, From, Error)]
    pub enum GetMacroMapError {
        /// Failed to run command.
        #[display("{_0}")]
        CommandFailed(RunCommandError),
        /// Macro map returned by the keyboard failed to parse.
        MacroMapParsingFailure(ParseMacroMapError),
    }

    /// Error returned from [`DefyKeyboard::apply_macros`].
    #[derive(Debug, Display, Error)]
    pub enum ApplyMacroError {
        /// Too many macros were in the map to fit in the macro memory.
        #[display("{_0}")]
        TooManyMacros(parsing::macros::TooManyMacrosError),
        /// A key code was too large for a single-byte macro action.
        #[display("{_0}")]
        KeyCodeTooLarge(parsing::macros::KeyCodeTooLargeError),
        /// Probing the macro memory with the `macros.memory` command
        /// failed.
        #[display("{_0}")]
        MemoryProbe(RunCommandError),
        /// Command failed to run.
        #[display("{_0}")]
        CommandFailed(RunCommandError),
    }

    /// Error returned when reading one of the single-value identity or
    /// settings commands (`version`, `hardware.*`, `keymap.onlyCustom`,
    /// `settings.defaultLayer`, `qukeys.*`, `superkeys.*` timings,
    /// `mouse.*`, `macros.memory`, `eeprom.contents`) or one of the
    /// wireless reads (`wireless.battery.{left,right}.{level,status}`,
    /// `wireless.battery.savingMode`, `wireless.rf.power`,
    /// `wireless.rf.channelHop`, `wireless.bluetooth.deviceName`,
    /// `wireless.bluetooth.infoChannel`).
    #[derive(Debug, Display, From, Error)]
    pub enum GetSettingError {
        /// Failed to run the command.
        #[display("{_0}")]
        CommandFailed(RunCommandError),
        /// The response could not be parsed.
        #[display("{_0}")]
        ParsingFailure(parsing::settings::ParseSettingsError),
        /// A wireless command response could not be parsed.
        #[display("{_0}")]
        WirelessParsingFailure(parsing::wireless::ParseWirelessError),
    }

    /// Error returned when writing one of the single-value settings
    /// commands.
    #[derive(Debug, Display, From, Error)]
    #[display("failed to write the setting: {_0}")]
    pub struct SetSettingError(RunCommandError);

    /// Error returned from [`DefyKeyboard::reset_eeprom`].
    #[derive(Debug, Display, From, Error)]
    pub enum ResetEepromError {
        /// Reading the current `eeprom.contents` failed, so the number
        /// of bytes to erase is unknown.
        #[display("{_0}")]
        ReadContents(GetSettingError),
        /// The device reported an empty EEPROM, so there is nothing to
        /// erase (an empty write would be treated as a read by the
        /// firmware).
        #[display("{_0}")]
        EmptyEeprom(parsing::settings::EmptyEepromError),
        /// Writing the erase payload failed.
        #[display("{_0}")]
        Erase(SetSettingError),
    }

    /// Error returned from [`DefyKeyboard::create_backup`].
    #[derive(Debug, Display, Error)]
    pub enum CreateBackupError {
        /// Querying the device's `help` command list failed, so the
        /// capture list could not be computed and nothing was
        /// captured. There is nothing to recover a partial capture
        /// from.
        #[display("failed to list the device's commands: {_0}")]
        ListCommands(GetCommandsError),
        /// The capture was interrupted before it could complete: one
        /// of the capture-list command reads failed mid-capture (the
        /// identity probes cannot interrupt a capture — they are
        /// best-effort; see [`CaptureIdentity`]). The file this run
        /// would have produced is a *partial* capture:
        /// [`parsing::backup::DefyBackup::partial_capture`] in
        /// `partial` holds every command captured before the failure
        /// and the [`parsing::backup::PartialCapture`] marker naming
        /// the capture-list commands it is missing. The backup still
        /// saves (and later completes, with
        /// [`DefyKeyboard::complete_backup`]) — it is never mistaken
        /// for a complete one: restore validation refuses it by
        /// default.
        #[display(
            "the backup capture was interrupted at `{command}`, a partial capture was preserved: {source}"
        )]
        IncompleteCapture {
            /// The partial backup: everything captured before the
            /// failure, marked with the commands it is missing. Boxed —
            /// the payload dwarfs the other variants, and errors are
            /// returned everywhere.
            partial: Box<parsing::backup::DefyBackup>,
            /// The capture-list command whose read failed and stopped
            /// the capture.
            command: String,
            /// The error the command read failed with.
            source: RunCommandError,
        },
    }

    /// Why a backup capture's identity probe produced no value.
    ///
    /// An alias of the shared best-effort probe machinery
    /// ([`IdentityProbeUnavailable`](crate::devices::probes::IdentityProbeUnavailable))
    /// — the capture probes are exactly the connect-time
    /// identification's probes, one rule for both. See
    /// [`DefyKeyboard::create_backup`] for how a capture treats each
    /// variant.
    #[doc(inline)]
    pub use crate::devices::probes::IdentityProbeUnavailable;

    /// The identity metadata a completed backup capture obtained, one
    /// field per probe: the value the device reported, or
    /// [`IdentityProbeUnavailable`] naming why it has none.
    ///
    /// A capture does not abort on a probe — it is complete when every
    /// capture-list command was captured, whatever the probes did —
    /// so the backup file itself carries only the identity that was
    /// obtained (an empty chip id, absent versions, and the documented
    /// wired descriptor flag when `hardware.wireless` is unknown; see
    /// [`DefyKeyboard::create_backup`]). This report is how the caller
    /// learns what was *not* obtained, so the gaps can be said out
    /// loud (the CLI prints a note) instead of passing silently.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct CaptureIdentity {
        /// The Neuron's `hardware.chip_id`, or why the capture has
        /// none. In the file, `Err` reads as: the backup's `neuronID`
        /// and the descriptor's `chipId` are empty, so the backup
        /// cannot key a Neuron names-store entry.
        pub chip_id: Result<String, IdentityProbeUnavailable>,
        /// The device's verbatim `version` response, or why the
        /// capture has none. In the file, `Err` reads as: the backup's
        /// `versions` object is empty (all three fields absent).
        pub version: Result<String, IdentityProbeUnavailable>,
        /// The `hardware.wireless` flag, or why the capture has none.
        /// In the file, `Err` reads as: the descriptor records the
        /// wired variant (see [`DefyKeyboard::create_backup`] for why
        /// that choice is safe).
        pub wireless: Result<bool, IdentityProbeUnavailable>,
    }

    impl CaptureIdentity {
        /// The identity fields the device did not provide, with the
        /// reason each is missing — the single source of the
        /// unavailable-probe iteration, so a report consumer cannot
        /// forget a field.
        pub fn unavailable(&self) -> impl Iterator<Item = IdentityProbeUnavailable> + '_ {
            [
                self.chip_id.as_ref().err(),
                self.version.as_ref().err(),
                self.wireless.as_ref().err(),
            ]
            .into_iter()
            .flatten()
            .copied()
        }
    }

    /// The Display message of [`BackupValidationError::PartialCapture`]:
    /// the shared partial-capture refusal wording — the missing
    /// commands, or, for a marker with an empty missing list, why the
    /// list is empty. The single source of the wording lives beside the
    /// marker type ([`PartialCapture::refusal_display`][refusal]),
    /// shared with the Sonsei's restore validation; this wrapper keeps
    /// the variant's `#[display]` pointing at a `&[String]`.
    ///
    /// [refusal]:
    ///     crate::focus_api::parsing::backup::PartialCapture::refusal_display
    fn partial_capture_display(missing: &[String]) -> String {
        parsing::backup::PartialCapture {
            missing: missing.to_vec(),
        }
        .refusal_display()
    }

    /// Error returned when a backup fails restore validation.
    ///
    /// Validation ([`DefyKeyboard::backup_restore_plan`]) runs entirely
    /// offline, before anything is sent: a backup either validates
    /// whole, or nothing reaches the device.
    #[derive(Clone, Debug, Display, Error)]
    pub enum BackupValidationError {
        /// The backup's device descriptor names a product other than
        /// the Defy. The Defy is the only product this crate restores
        /// onto — restoring another product's backup (a Raise's, say)
        /// would write foreign data to the device.
        #[display("the backup is for a `{product}`, not a Defy")]
        NotADefyBackup {
            /// The product the backup's device descriptor names.
            product: String,
        },
        /// One of the backup's captured commands carries data this
        /// crate can prove is not valid Defy wire data. The keymap,
        /// superkeys, macros, palette, and colormap entries are each
        /// validated by parsing them against their wire formats (the
        /// map types also enforce the write shapes); captured commands
        /// without a wire parser yet pass through verbatim, exactly
        /// the way Bazecor's restore sends them.
        #[display("the `{command}` entry failed restore validation: {source}")]
        InvalidEntry {
            /// The captured command whose data failed validation.
            command: String,
            /// The parse failure.
            source: BackupEntryError,
        },
        /// The backup carries the partial-capture marker
        /// ([`parsing::backup::PartialCapture`]): it was saved from a
        /// capture that failed partway, so it does not hold the whole
        /// device state a capture captures. Restoring it would write a
        /// half-captured configuration to the device and is refused
        /// unless explicitly allowed (see
        /// [`DefyKeyboard::backup_restore_plan_allowing_partial`]).
        #[display("{}", partial_capture_display(missing))]
        PartialCapture {
            /// The commands the capture list expects and the file does
            /// not hold — what restoring it would leave uncaptured.
            missing: Vec<String>,
        },
    }

    /// The parse failure behind
    /// [`BackupValidationError::InvalidEntry`]: one variant per
    /// captured command this crate can validate.
    #[derive(Clone, Debug, Display, Error)]
    pub enum BackupEntryError {
        /// The `keymap.custom` data is not a valid Defy keymap — either
        /// not valid wire data at all, or not the exactly ten layers
        /// every `keymap.custom` write carries (the wire parser itself
        /// accepts any layer count, so the [`DefyKeymap`] type enforces
        /// the count at construction).
        #[display("{_0}")]
        Keymap(ParseKeymapError),
        /// The `superkeys.map` data is not a valid superkeys map.
        #[display("{_0}")]
        Superkeys(ParseSuperkeyMapError),
        /// The `macros.map` data is not a valid macros map.
        #[display("{_0}")]
        Macros(ParseMacroMapError),
        /// The `palette` data is not a valid Defy palette — either not
        /// valid wire data at all, or not the exactly 64 decimals (16
        /// RGBW colors) a Defy palette carries.
        #[display("{_0}")]
        Palette(parsing::palette::ParsePaletteError),
        /// The `colormap.map` data is not a valid Defy colormap —
        /// either not valid wire data at all, or not the exactly ten
        /// layers every `colormap.map` write carries (the wire parser
        /// itself accepts any layer count, so the [`DefyColormap`] type
        /// enforces the count at construction).
        #[display("{_0}")]
        Colormap(ParseColormapError),
    }

    /// Error returned from [`DefyKeyboard::restore_backup`].
    #[derive(Debug, Display, Error)]
    pub enum RestoreBackupError {
        /// The backup failed restore validation, so nothing was sent
        /// to the device.
        #[display("{_0}")]
        Validation(BackupValidationError),
        /// Sending one of the backup's commands failed. The restore
        /// stopped at this command; everything sent before it was
        /// applied and the commands after it were not.
        #[display("failed to send the `{command}` command: {source}")]
        Send {
            /// The command that failed to send.
            command: String,
            /// The error the send failed with.
            source: RunCommandError,
        },
    }

    impl From<parsing::macros::ToCommandDataError> for ApplyMacroError {
        fn from(err: parsing::macros::ToCommandDataError) -> Self {
            match err {
                parsing::macros::ToCommandDataError::TooManyMacros(err) => Self::TooManyMacros(err),
                parsing::macros::ToCommandDataError::KeyCodeTooLarge(err) => {
                    Self::KeyCodeTooLarge(err)
                }
            }
        }
    }

    impl From<RunCommandError> for ApplyMacroError {
        fn from(err: RunCommandError) -> Self {
            Self::CommandFailed(err)
        }
    }

    /// Possible errors when clearing a [`DefyKeymap`] layer.
    #[derive(Clone, Copy, Debug, Display, Error)]
    pub enum ClearLayerError {
        /// Layer 0 is not a valid layer. Layers start with index 1.
        #[display("0 is not a valid layer, layers start at index 1")]
        ZeroIndexProvided,
        /// Tried to clear a layer that does not exist. A Defy keymap has
        /// exactly ten layers (numbered 1–10).
        #[display("the layer does not exist in this keymap")]
        LayerDoesNotExist,
    }
}

/// Type alias for the raw keymap data.
pub type DefyLayerData = [KeyKind; KEYS_PER_LAYER];

/// Number of keys per layer.
pub const KEYS_PER_LAYER: usize = 80;

/// Number of layers in a `keymap.custom` command.
///
/// This is the exact layer count of every Defy keymap: the device
/// stores and reports exactly this many, and [`DefyKeymap`] can only
/// hold this many.
pub const KEYMAP_CUSTOM_COMMAND_LAYERS: usize = 10;

/// Constant providing the Defy keymap layout.
pub const LAYOUT: &DefyLayout = &DefyLayout {
    left: DefyLayoutHalf {
        row_1: [0, 1, 2, 3, 4, 5, 6],
        row_2: [16, 17, 18, 19, 20, 21, 22],
        row_3: [32, 33, 34, 35, 36, 37, 38],
        row_4: [48, 49, 50, 51, 52, 53],
        thumb_cluster: DefyThumbClusterLayout {
            top: [64, 65, 66, 67],
            bottom: [71, 70, 69, 68],
        },
    },
    right: DefyLayoutHalf {
        row_1: [9, 10, 11, 12, 13, 14, 15],
        row_2: [25, 26, 27, 28, 29, 30, 31],
        row_3: [41, 42, 43, 44, 45, 46, 47],
        row_4: [58, 59, 60, 61, 62, 63],
        thumb_cluster: DefyThumbClusterLayout {
            top: [76, 77, 78, 79],
            bottom: [75, 74, 73, 72],
        },
    },
};

/// A handle to the Dygma Defy keyboard, allowing for programatic control.
#[derive(Debug, Deref, DerefMut, From)]
pub struct DefyKeyboard {
    focus_api: DynFocusApi,
}

impl DefyKeyboard {
    const HID_PRODUCT_ID: u16 = 18;

    /// The Defy's serial identities, both variants: the only devices
    /// this typed handle may reach. The matching is by USB (vendor id,
    /// product id) pair, the way Bazecor matches serial ports — the
    /// port's manufacturer/product strings are never consulted.
    const SERIAL_IDENTITIES: &[identities::DeviceIdentity] =
        &[identities::DEFY_WIRED, identities::DEFY_WIRELESS];

    const KEYMAP_CUSTOM_COMMAND_NAME: &str = "keymap.custom";
    const SUPERKEY_MAP_COMMAND_NAME: &str = "superkeys.map";
    const MACRO_MAP_COMMAND_NAME: &str = "macros.map";
    const VERSION_COMMAND_NAME: &str = "version";
    const HARDWARE_CHIP_ID_COMMAND_NAME: &str = "hardware.chip_id";
    const HARDWARE_WIRELESS_COMMAND_NAME: &str = "hardware.wireless";
    /// The `hardware.layout` wire command's name, shared with the
    /// connect-time identification's layout probe
    /// ([`DygmaConnection`](super::connection::DygmaConnection)) — the
    /// one source of the string.
    pub(crate) const HARDWARE_LAYOUT_COMMAND_NAME: &str = "hardware.layout";
    const KEYMAP_ONLY_CUSTOM_COMMAND_NAME: &str = "keymap.onlyCustom";
    const SETTINGS_DEFAULT_LAYER_COMMAND_NAME: &str = "settings.defaultLayer";
    const QUKEYS_HOLD_TIMEOUT_COMMAND_NAME: &str = "qukeys.holdTimeout";
    const QUKEYS_OVERLAP_THRESHOLD_COMMAND_NAME: &str = "qukeys.overlapThreshold";
    const QUKEYS_MINIMUM_HOLD_TIME_COMMAND_NAME: &str = "qukeys.minimumHoldTime";
    const QUKEYS_MINIMUM_PRIOR_INTERVAL_COMMAND_NAME: &str = "qukeys.minimumPriorInterval";
    const SUPERKEYS_TIMEOUT_COMMAND_NAME: &str = "superkeys.timeout";
    const SUPERKEYS_REPEAT_COMMAND_NAME: &str = "superkeys.repeat";
    const SUPERKEYS_WAIT_FOR_COMMAND_NAME: &str = "superkeys.waitfor";
    const SUPERKEYS_HOLD_START_COMMAND_NAME: &str = "superkeys.holdstart";
    const SUPERKEYS_OVERLAP_COMMAND_NAME: &str = "superkeys.overlap";
    const MOUSE_SPEED_COMMAND_NAME: &str = "mouse.speed";
    const MOUSE_SPEED_DELAY_COMMAND_NAME: &str = "mouse.speedDelay";
    const MOUSE_ACCEL_SPEED_COMMAND_NAME: &str = "mouse.accelSpeed";
    const MOUSE_ACCEL_DELAY_COMMAND_NAME: &str = "mouse.accelDelay";
    const MOUSE_WHEEL_SPEED_COMMAND_NAME: &str = "mouse.wheelSpeed";
    const MOUSE_WHEEL_DELAY_COMMAND_NAME: &str = "mouse.wheelDelay";
    const MOUSE_SPEED_LIMIT_COMMAND_NAME: &str = "mouse.speedLimit";
    const MACROS_MEMORY_COMMAND_NAME: &str = "macros.memory";
    const EEPROM_CONTENTS_COMMAND_NAME: &str = "eeprom.contents";
    const PALETTE_COMMAND_NAME: &str = "palette";
    const COLORMAP_MAP_COMMAND_NAME: &str = "colormap.map";
    const LED_BRIGHTNESS_COMMAND_NAME: &str = "led.brightness";
    const LED_BRIGHTNESS_UG_COMMAND_NAME: &str = "led.brightnessUG";
    const LED_BRIGHTNESS_WIRELESS_COMMAND_NAME: &str = "led.brightness.wireless";
    const LED_BRIGHTNESS_UG_WIRELESS_COMMAND_NAME: &str = "led.brightnessUG.wireless";
    const LED_FADE_COMMAND_NAME: &str = "led.fade";
    const IDLELEDS_TIME_LIMIT_COMMAND_NAME: &str = "idleleds.time_limit";
    const IDLELEDS_WIRELESS_COMMAND_NAME: &str = "idleleds.wireless";
    const IDLELEDS_TRUE_SLEEP_COMMAND_NAME: &str = "idleleds.true_sleep";
    const IDLELEDS_TRUE_SLEEP_TIME_COMMAND_NAME: &str = "idleleds.true_sleep_time";
    const WIRELESS_BATTERY_LEFT_LEVEL_COMMAND_NAME: &str = "wireless.battery.left.level";
    const WIRELESS_BATTERY_RIGHT_LEVEL_COMMAND_NAME: &str = "wireless.battery.right.level";
    const WIRELESS_BATTERY_LEFT_STATUS_COMMAND_NAME: &str = "wireless.battery.left.status";
    const WIRELESS_BATTERY_RIGHT_STATUS_COMMAND_NAME: &str = "wireless.battery.right.status";
    const WIRELESS_BATTERY_SAVING_MODE_COMMAND_NAME: &str = "wireless.battery.savingMode";
    const WIRELESS_BATTERY_FORCE_READ_COMMAND_NAME: &str = "wireless.battery.forceRead";
    const WIRELESS_RF_POWER_COMMAND_NAME: &str = "wireless.rf.power";
    const WIRELESS_RF_CHANNEL_HOP_COMMAND_NAME: &str = "wireless.rf.channelHop";
    const WIRELESS_RF_SYNC_PAIRING_COMMAND_NAME: &str = "wireless.rf.syncPairing";
    const WIRELESS_BLUETOOTH_DEVICE_NAME_COMMAND_NAME: &str = "wireless.bluetooth.deviceName";
    const WIRELESS_BLUETOOTH_INFO_CHANNEL_COMMAND_NAME: &str = "wireless.bluetooth.infoChannel";
    /// The memory size of the superkey map.
    pub const SUPERKEY_MEMORY_SIZE: usize = 512;

    /// The fallback memory size of the macro map, used when the
    /// `macros.memory` probe is missing or untrustworthy. Use
    /// [`DefyKeyboard::macro_memory_size`] for the real per-device
    /// value.
    pub const MACRO_MEMORY_SIZE: usize = parsing::settings::MacroMemorySize::FALLBACK as usize;

    /// Creates a handle to the keyboard.
    pub async fn new() -> Result<Self, CreateDefyKeyboardError> {
        let sp_focus_api_res =
            SerialPortFocusApi::new(Self::SERIAL_IDENTITIES, identities::BAUD_RATE)
                .await
                .map(|(api, _identity)| api.into());
        let bt_focus_api_res = HidFocusApi::new(Self::HID_PRODUCT_ID).await.map(Into::into);

        let focus_api = sp_focus_api_res.or(bt_focus_api_res)?;

        Ok(Self { focus_api })
    }

    /// Get the custom keymap from the keyboard.
    pub async fn get_custom_keymap(&mut self) -> Result<DefyKeymap, GetCustomKeymapError> {
        self.run_command(Self::KEYMAP_CUSTOM_COMMAND_NAME, None)
            .await?
            .parse()
            .map_err(Into::into)
    }

    /// Apply the keymap to the keyboard.
    pub async fn apply_custom_keymap(
        &mut self,
        keymap: &DefyKeymap,
    ) -> Result<(), ApplyCustomKeymapError> {
        let data = keymap.to_keymap_custom_data();

        self.run_command(Self::KEYMAP_CUSTOM_COMMAND_NAME, Some(&data))
            .await?;

        Ok(())
    }

    /// Apply the superkeys map to the keyboard.
    pub async fn apply_superkeys(
        &mut self,
        superkeys: &SuperkeyMap,
    ) -> Result<(), ApplySuperkeyError> {
        let data = parsing::superkeys::SuperkeyMap::from(superkeys)
            .to_command_data::<{ Self::SUPERKEY_MEMORY_SIZE }>()?;

        self.run_command(Self::SUPERKEY_MAP_COMMAND_NAME, Some(&data))
            .await?;

        Ok(())
    }

    /// Get the keyperkey map from the keyboard.
    pub async fn get_superkeys(&mut self) -> Result<SuperkeyMap, GetSuperkeyMapError> {
        let map = self
            .run_command(Self::SUPERKEY_MAP_COMMAND_NAME, None)
            .await?
            .parse::<SuperkeyMap>()?;

        Ok(map)
    }

    /// Get the macros from the keyboard.
    pub async fn get_macros(&mut self) -> Result<MacroMap, GetMacroMapError> {
        let map = self
            .run_command(Self::MACRO_MAP_COMMAND_NAME, None)
            .await?
            .parse::<MacroMap>()?;

        Ok(map)
    }

    /// Apply the macros to the keyboard.
    ///
    /// The macro map memory is probed per device with the
    /// `macros.memory` command first, so the map is serialized with the
    /// correct padding and erase semantics for this specific keyboard.
    ///
    /// The map cannot hold more macros than the firmware can address —
    /// [`MacroMap`] enforces that bound wherever it is constructed — so
    /// applying never emits a map the firmware could never trigger.
    pub async fn apply_macros(&mut self, macros: &MacroMap) -> Result<(), ApplyMacroError> {
        let memory_size = self
            .macro_memory_size()
            .await
            .map_err(ApplyMacroError::MemoryProbe)?;

        let data = parsing::macros::MacroMap::from(macros).to_command_data(memory_size.into())?;

        self.run_command(Self::MACRO_MAP_COMMAND_NAME, Some(&data))
            .await?;

        Ok(())
    }

    /// Probes the size of the device's macro map memory with the
    /// `macros.memory` command.
    ///
    /// The result is sanitized like Bazecor does: a missing probe (the
    /// command is not served by older firmwares) or a value below
    /// [`MacroMemorySize::MIN`](parsing::settings::MacroMemorySize::MIN)
    /// falls back to 2048.
    pub async fn macro_memory_size(
        &mut self,
    ) -> Result<parsing::settings::MacroMemorySize, RunCommandError> {
        let res = self
            .run_command(Self::MACROS_MEMORY_COMMAND_NAME, None)
            .await?;

        // An unparsable response means the firmware does not really
        // serve the command; treat it like Bazecor treats a missing
        // probe instead of failing.
        let value = res
            .parse::<parsing::settings::DecimalU16>()
            .ok()
            .map(|value| value.0);

        Ok(parsing::settings::MacroMemorySize::sanitize(value))
    }

    /// Get the device's 16-color palette (the `palette` command).
    ///
    /// The Defy's palette is RGBW: each color carries an independent
    /// white channel, and it is read verbatim — unlike Bazecor's app,
    /// which converts each color to display RGB at parse time (its
    /// `rgbw2b`) and drops the white channel, this returns the bytes
    /// the device stores. Rendering a color the way Bazecor's UI does
    /// is [`RgbwColor::to_rgb`](parsing::palette::RgbwColor::to_rgb).
    pub async fn get_palette(&mut self) -> Result<parsing::palette::Palette, GetPaletteError> {
        let palette = self
            .run_command(Self::PALETTE_COMMAND_NAME, None)
            .await?
            .parse()?;

        Ok(palette)
    }

    /// Apply the palette to the keyboard (the `palette` command).
    ///
    /// The palette is sent verbatim — all 16 RGBW colors, in palette
    /// order. Bazecor's app writes a *re-derived* palette instead (its
    /// in-app palette has no white channel, so it guesses one back with
    /// `rgb2w` on every save, which rewrites colors whose colored
    /// channels are all positive); a palette that came from
    /// [`DefyKeyboard::get_palette`] round-trips here byte for byte.
    pub async fn apply_palette(
        &mut self,
        palette: &parsing::palette::Palette,
    ) -> Result<(), ApplyPaletteError> {
        self.run_command(Self::PALETTE_COMMAND_NAME, Some(&palette.to_string()))
            .await?;

        Ok(())
    }

    /// Get the device's colormap (the `colormap.map` command): all ten
    /// layers of palette indices, one per LED.
    pub async fn get_colormap(&mut self) -> Result<DefyColormap, GetColormapError> {
        let colormap = self
            .run_command(Self::COLORMAP_MAP_COMMAND_NAME, None)
            .await?
            .parse()?;

        Ok(colormap)
    }

    /// Apply the colormap to the keyboard (the `colormap.map`
    /// command): all ten layers, layer-major, one palette index per
    /// LED.
    pub async fn apply_colormap(
        &mut self,
        colormap: &DefyColormap,
    ) -> Result<(), ApplyColormapError> {
        self.run_command(
            Self::COLORMAP_MAP_COMMAND_NAME,
            Some(&colormap.to_colormap_map_data()),
        )
        .await?;

        Ok(())
    }

    /// Get the firmware version triple (`version` command): the
    /// Bazecor, Kaleidoscope, and firmware versions.
    pub async fn get_version(
        &mut self,
    ) -> Result<parsing::settings::FirmwareVersion, GetSettingError> {
        self.get_setting(Self::VERSION_COMMAND_NAME).await
    }

    /// Get the Neuron identity string (`hardware.chip_id` command),
    /// which Bazecor keys its per-device name stores by.
    pub async fn get_chip_id(&mut self) -> Result<parsing::settings::ChipId, GetSettingError> {
        self.get_setting(Self::HARDWARE_CHIP_ID_COMMAND_NAME).await
    }

    /// Get whether this keyboard is the wireless (RF/BT) variant
    /// (`hardware.wireless` command).
    pub async fn is_wireless(&mut self) -> Result<bool, GetSettingError> {
        self.get_setting::<parsing::settings::BoolWord>(Self::HARDWARE_WIRELESS_COMMAND_NAME)
            .await
            .map(|value| value.0)
    }

    /// Get the physical key layout (`hardware.layout` command).
    pub async fn get_layout(
        &mut self,
    ) -> Result<parsing::settings::KeyboardLayout, GetSettingError> {
        self.get_setting(Self::HARDWARE_LAYOUT_COMMAND_NAME).await
    }

    /// Get whether the keyboard only uses the layers stored in
    /// `keymap.custom`, ignoring its hardcoded default layers
    /// (`keymap.onlyCustom` command).
    pub async fn get_only_custom(&mut self) -> Result<bool, GetSettingError> {
        self.get_setting::<parsing::settings::OnlyCustom>(Self::KEYMAP_ONLY_CUSTOM_COMMAND_NAME)
            .await
            .map(|value| value.0)
    }

    /// Set whether the keyboard only uses the layers stored in
    /// `keymap.custom` (`keymap.onlyCustom` command).
    pub async fn set_only_custom(&mut self, only_custom: bool) -> Result<(), SetSettingError> {
        self.set_setting(
            Self::KEYMAP_ONLY_CUSTOM_COMMAND_NAME,
            parsing::settings::only_custom_command_data(only_custom),
        )
        .await
    }

    /// Get the layer the keyboard boots into (`settings.defaultLayer`
    /// command).
    pub async fn get_default_layer(
        &mut self,
    ) -> Result<parsing::settings::DefaultLayer, GetSettingError> {
        self.get_setting(Self::SETTINGS_DEFAULT_LAYER_COMMAND_NAME)
            .await
    }

    /// Set the layer the keyboard boots into
    /// (`settings.defaultLayer` command).
    ///
    /// The type admits layers 0 to
    /// [`DEFAULT_LAYER_MAX`](parsing::settings::DEFAULT_LAYER_MAX), but
    /// the device only honors layers below its own layer count: the
    /// firmware silently ignores an out-of-range write, so a successful
    /// send does not guarantee the layer took effect.
    pub async fn set_default_layer(
        &mut self,
        layer: parsing::settings::DefaultLayer,
    ) -> Result<(), SetSettingError> {
        self.set_setting(
            Self::SETTINGS_DEFAULT_LAYER_COMMAND_NAME,
            &layer.to_command_data(),
        )
        .await
    }

    /// Get the qukeys hold timeout in ms (`qukeys.holdTimeout`
    /// command).
    pub async fn get_qukeys_hold_timeout(&mut self) -> Result<u16, GetSettingError> {
        self.get_setting::<parsing::settings::DecimalU16>(Self::QUKEYS_HOLD_TIMEOUT_COMMAND_NAME)
            .await
            .map(|value| value.0)
    }

    /// Set the qukeys hold timeout in ms (`qukeys.holdTimeout`
    /// command).
    ///
    /// The firmware's write path composes the value from a
    /// low-byte-then-high-byte decimal pair, which is what gets sent;
    /// a single decimal could only ever address values up to 255.
    pub async fn set_qukeys_hold_timeout(&mut self, timeout: u16) -> Result<(), SetSettingError> {
        self.set_setting(
            Self::QUKEYS_HOLD_TIMEOUT_COMMAND_NAME,
            &parsing::settings::split_u16_command_data(timeout),
        )
        .await
    }

    /// Get the qukeys overlap threshold as a percentage
    /// (`qukeys.overlapThreshold` command).
    pub async fn get_qukeys_overlap_threshold(&mut self) -> Result<u8, GetSettingError> {
        self.get_setting::<parsing::settings::DecimalU8>(
            Self::QUKEYS_OVERLAP_THRESHOLD_COMMAND_NAME,
        )
        .await
        .map(|value| value.0)
    }

    /// Set the qukeys overlap threshold as a percentage
    /// (`qukeys.overlapThreshold` command).
    pub async fn set_qukeys_overlap_threshold(
        &mut self,
        threshold: u8,
    ) -> Result<(), SetSettingError> {
        self.set_setting(
            Self::QUKEYS_OVERLAP_THRESHOLD_COMMAND_NAME,
            &threshold.to_string(),
        )
        .await
    }

    /// Get the minimum time a qukey must be held in ms to count as a
    /// hold (`qukeys.minimumHoldTime` command).
    ///
    /// The wire value is a single byte, so the 0-255 range of the
    /// `u8` parameter *is* the clamp Bazecor applies on top of it;
    /// larger values are unrepresentable here.
    pub async fn get_qukeys_minimum_hold_time(&mut self) -> Result<u8, GetSettingError> {
        self.get_setting::<parsing::settings::DecimalU8>(
            Self::QUKEYS_MINIMUM_HOLD_TIME_COMMAND_NAME,
        )
        .await
        .map(|value| value.0)
    }

    /// Set the minimum time a qukey must be held in ms to count as a
    /// hold (`qukeys.minimumHoldTime` command).
    ///
    /// The wire value is a single byte, so the 0-255 range of the
    /// `u8` parameter *is* the clamp Bazecor applies on top of it;
    /// larger values are unrepresentable here.
    pub async fn set_qukeys_minimum_hold_time(&mut self, time: u8) -> Result<(), SetSettingError> {
        self.set_setting(
            Self::QUKEYS_MINIMUM_HOLD_TIME_COMMAND_NAME,
            &time.to_string(),
        )
        .await
    }

    /// Get the minimum interval since the previous key press in ms for
    /// a qukey to be considered a hold (`qukeys.minimumPriorInterval`
    /// command).
    pub async fn get_qukeys_minimum_prior_interval(&mut self) -> Result<u8, GetSettingError> {
        self.get_setting::<parsing::settings::DecimalU8>(
            Self::QUKEYS_MINIMUM_PRIOR_INTERVAL_COMMAND_NAME,
        )
        .await
        .map(|value| value.0)
    }

    /// Set the minimum interval since the previous key press in ms for
    /// a qukey to be considered a hold
    /// (`qukeys.minimumPriorInterval` command).
    ///
    /// Note that Bazecor, when its stored value for this setting is
    /// unset, sends the command with an *empty* payload. The firmware
    /// treats an empty payload as a read, so that write is a no-op;
    /// this API does not replicate it and always writes an actual
    /// value.
    pub async fn set_qukeys_minimum_prior_interval(
        &mut self,
        interval: u8,
    ) -> Result<(), SetSettingError> {
        self.set_setting(
            Self::QUKEYS_MINIMUM_PRIOR_INTERVAL_COMMAND_NAME,
            &interval.to_string(),
        )
        .await
    }

    /// Get the superkey tap-hold timeout in ms (`superkeys.timeout`
    /// command).
    pub async fn get_superkeys_timeout(&mut self) -> Result<u16, GetSettingError> {
        self.get_setting::<parsing::settings::DecimalU16>(Self::SUPERKEYS_TIMEOUT_COMMAND_NAME)
            .await
            .map(|value| value.0)
    }

    /// Set the superkey tap-hold timeout in ms (`superkeys.timeout`
    /// command).
    pub async fn set_superkeys_timeout(&mut self, timeout: u16) -> Result<(), SetSettingError> {
        self.set_setting(Self::SUPERKEYS_TIMEOUT_COMMAND_NAME, &timeout.to_string())
            .await
    }

    /// Get the superkey auto-repeat interval in ms
    /// (`superkeys.repeat` command).
    pub async fn get_superkeys_repeat(&mut self) -> Result<u8, GetSettingError> {
        self.get_setting::<parsing::settings::DecimalU8>(Self::SUPERKEYS_REPEAT_COMMAND_NAME)
            .await
            .map(|value| value.0)
    }

    /// Set the superkey auto-repeat interval in ms
    /// (`superkeys.repeat` command).
    pub async fn set_superkeys_repeat(&mut self, interval: u8) -> Result<(), SetSettingError> {
        self.set_setting(Self::SUPERKEYS_REPEAT_COMMAND_NAME, &interval.to_string())
            .await
    }

    /// Get the superkey double-tap wait window in ms
    /// (`superkeys.waitfor` command).
    pub async fn get_superkeys_wait_for(&mut self) -> Result<u16, GetSettingError> {
        self.get_setting::<parsing::settings::DecimalU16>(Self::SUPERKEYS_WAIT_FOR_COMMAND_NAME)
            .await
            .map(|value| value.0)
    }

    /// Set the superkey double-tap wait window in ms
    /// (`superkeys.waitfor` command).
    pub async fn set_superkeys_wait_for(&mut self, wait: u16) -> Result<(), SetSettingError> {
        self.set_setting(Self::SUPERKEYS_WAIT_FOR_COMMAND_NAME, &wait.to_string())
            .await
    }

    /// Get the superkey hold start time in ms
    /// (`superkeys.holdstart` command).
    pub async fn get_superkeys_hold_start(&mut self) -> Result<u16, GetSettingError> {
        self.get_setting::<parsing::settings::DecimalU16>(Self::SUPERKEYS_HOLD_START_COMMAND_NAME)
            .await
            .map(|value| value.0)
    }

    /// Set the superkey hold start time in ms
    /// (`superkeys.holdstart` command).
    pub async fn set_superkeys_hold_start(
        &mut self,
        hold_start: u16,
    ) -> Result<(), SetSettingError> {
        self.set_setting(
            Self::SUPERKEYS_HOLD_START_COMMAND_NAME,
            &hold_start.to_string(),
        )
        .await
    }

    /// Get the superkey overlap threshold as a percentage
    /// (`superkeys.overlap` command).
    pub async fn get_superkeys_overlap(&mut self) -> Result<u8, GetSettingError> {
        self.get_setting::<parsing::settings::DecimalU8>(Self::SUPERKEYS_OVERLAP_COMMAND_NAME)
            .await
            .map(|value| value.0)
    }

    /// Set the superkey overlap threshold as a percentage
    /// (`superkeys.overlap` command).
    pub async fn set_superkeys_overlap(&mut self, threshold: u8) -> Result<(), SetSettingError> {
        self.set_setting(Self::SUPERKEYS_OVERLAP_COMMAND_NAME, &threshold.to_string())
            .await
    }

    /// Get the base mouse cursor speed (`mouse.speed` command).
    pub async fn get_mouse_speed(&mut self) -> Result<u8, GetSettingError> {
        self.get_setting::<parsing::settings::DecimalU8>(Self::MOUSE_SPEED_COMMAND_NAME)
            .await
            .map(|value| value.0)
    }

    /// Set the base mouse cursor speed (`mouse.speed` command).
    pub async fn set_mouse_speed(&mut self, speed: u8) -> Result<(), SetSettingError> {
        self.set_setting(Self::MOUSE_SPEED_COMMAND_NAME, &speed.to_string())
            .await
    }

    /// Get the mouse cursor start-up delay in ms
    /// (`mouse.speedDelay` command).
    pub async fn get_mouse_speed_delay(&mut self) -> Result<u16, GetSettingError> {
        self.get_setting::<parsing::settings::DecimalU16>(Self::MOUSE_SPEED_DELAY_COMMAND_NAME)
            .await
            .map(|value| value.0)
    }

    /// Set the mouse cursor start-up delay in ms
    /// (`mouse.speedDelay` command).
    ///
    /// The firmware's write path composes the value from a
    /// low-byte-then-high-byte decimal pair, which is what gets sent;
    /// a single decimal could only ever address values up to 255.
    pub async fn set_mouse_speed_delay(&mut self, delay: u16) -> Result<(), SetSettingError> {
        self.set_setting(
            Self::MOUSE_SPEED_DELAY_COMMAND_NAME,
            &parsing::settings::split_u16_command_data(delay),
        )
        .await
    }

    /// Get the mouse acceleration speed (`mouse.accelSpeed` command).
    pub async fn get_mouse_accel_speed(&mut self) -> Result<u8, GetSettingError> {
        self.get_setting::<parsing::settings::DecimalU8>(Self::MOUSE_ACCEL_SPEED_COMMAND_NAME)
            .await
            .map(|value| value.0)
    }

    /// Set the mouse acceleration speed (`mouse.accelSpeed` command).
    pub async fn set_mouse_accel_speed(&mut self, speed: u8) -> Result<(), SetSettingError> {
        self.set_setting(Self::MOUSE_ACCEL_SPEED_COMMAND_NAME, &speed.to_string())
            .await
    }

    /// Get the mouse acceleration delay in ms
    /// (`mouse.accelDelay` command).
    pub async fn get_mouse_accel_delay(&mut self) -> Result<u16, GetSettingError> {
        self.get_setting::<parsing::settings::DecimalU16>(Self::MOUSE_ACCEL_DELAY_COMMAND_NAME)
            .await
            .map(|value| value.0)
    }

    /// Set the mouse acceleration delay in ms
    /// (`mouse.accelDelay` command).
    ///
    /// The firmware's write path composes the value from a
    /// low-byte-then-high-byte decimal pair, which is what gets sent;
    /// a single decimal could only ever address values up to 255.
    pub async fn set_mouse_accel_delay(&mut self, delay: u16) -> Result<(), SetSettingError> {
        self.set_setting(
            Self::MOUSE_ACCEL_DELAY_COMMAND_NAME,
            &parsing::settings::split_u16_command_data(delay),
        )
        .await
    }

    /// Get the mouse wheel speed (`mouse.wheelSpeed` command).
    pub async fn get_mouse_wheel_speed(&mut self) -> Result<u8, GetSettingError> {
        self.get_setting::<parsing::settings::DecimalU8>(Self::MOUSE_WHEEL_SPEED_COMMAND_NAME)
            .await
            .map(|value| value.0)
    }

    /// Set the mouse wheel speed (`mouse.wheelSpeed` command).
    pub async fn set_mouse_wheel_speed(&mut self, speed: u8) -> Result<(), SetSettingError> {
        self.set_setting(Self::MOUSE_WHEEL_SPEED_COMMAND_NAME, &speed.to_string())
            .await
    }

    /// Get the mouse wheel delay in ms (`mouse.wheelDelay` command).
    pub async fn get_mouse_wheel_delay(&mut self) -> Result<u16, GetSettingError> {
        self.get_setting::<parsing::settings::DecimalU16>(Self::MOUSE_WHEEL_DELAY_COMMAND_NAME)
            .await
            .map(|value| value.0)
    }

    /// Set the mouse wheel delay in ms (`mouse.wheelDelay` command).
    ///
    /// The firmware's write path composes the value from a
    /// low-byte-then-high-byte decimal pair, which is what gets sent;
    /// a single decimal could only ever address values up to 255.
    pub async fn set_mouse_wheel_delay(&mut self, delay: u16) -> Result<(), SetSettingError> {
        self.set_setting(
            Self::MOUSE_WHEEL_DELAY_COMMAND_NAME,
            &parsing::settings::split_u16_command_data(delay),
        )
        .await
    }

    /// Get the mouse cursor speed limit (`mouse.speedLimit` command).
    pub async fn get_mouse_speed_limit(&mut self) -> Result<u8, GetSettingError> {
        self.get_setting::<parsing::settings::DecimalU8>(Self::MOUSE_SPEED_LIMIT_COMMAND_NAME)
            .await
            .map(|value| value.0)
    }

    /// Set the mouse cursor speed limit (`mouse.speedLimit` command).
    pub async fn set_mouse_speed_limit(&mut self, limit: u8) -> Result<(), SetSettingError> {
        self.set_setting(Self::MOUSE_SPEED_LIMIT_COMMAND_NAME, &limit.to_string())
            .await
    }

    /// Get the keyboard LED brightness (`led.brightness` command).
    ///
    /// The wire value is one byte, 0–255; Bazecor's UI maps it onto a
    /// 0–100% slider (`Math.round((value * 100) / 255)`).
    pub async fn get_led_brightness(&mut self) -> Result<u8, GetSettingError> {
        self.get_setting::<parsing::settings::DecimalU8>(Self::LED_BRIGHTNESS_COMMAND_NAME)
            .await
            .map(|value| value.0)
    }

    /// Set the keyboard LED brightness (`led.brightness` command).
    ///
    /// The wire value is one byte, so the 0–255 range of the `u8`
    /// parameter is the whole domain; the firmware's write path reads
    /// one decimal (`kaleidoscope/plugin/LEDControl.cpp`, BRIGHTNESS).
    pub async fn set_led_brightness(&mut self, brightness: u8) -> Result<(), SetSettingError> {
        self.set_setting(Self::LED_BRIGHTNESS_COMMAND_NAME, &brightness.to_string())
            .await
    }

    /// Get the underglow LED brightness (`led.brightnessUG` command).
    ///
    /// One byte, 0–255, like [`DefyKeyboard::get_led_brightness`].
    pub async fn get_led_underglow_brightness(&mut self) -> Result<u8, GetSettingError> {
        self.get_setting::<parsing::settings::DecimalU8>(Self::LED_BRIGHTNESS_UG_COMMAND_NAME)
            .await
            .map(|value| value.0)
    }

    /// Set the underglow LED brightness (`led.brightnessUG` command).
    pub async fn set_led_underglow_brightness(
        &mut self,
        brightness: u8,
    ) -> Result<(), SetSettingError> {
        self.set_setting(
            Self::LED_BRIGHTNESS_UG_COMMAND_NAME,
            &brightness.to_string(),
        )
        .await
    }

    /// Get the keyboard LED brightness used when the keyboard runs on
    /// battery (`led.brightness.wireless` command).
    ///
    /// One byte, 0–255. This command is served by the wireless
    /// Neuron's firmware, which is not in the local Kaleidoscope
    /// checkout; the evidence is Bazecor (it reads and writes a single
    /// decimal, `Preferences.tsx`) and the shipped Defy backups'
    /// `led.brightness.wireless` captures (single decimals, 0–255).
    pub async fn get_wireless_led_brightness(&mut self) -> Result<u8, GetSettingError> {
        self.get_setting::<parsing::settings::DecimalU8>(Self::LED_BRIGHTNESS_WIRELESS_COMMAND_NAME)
            .await
            .map(|value| value.0)
    }

    /// Set the wireless keyboard LED brightness
    /// (`led.brightness.wireless` command).
    pub async fn set_wireless_led_brightness(
        &mut self,
        brightness: u8,
    ) -> Result<(), SetSettingError> {
        self.set_setting(
            Self::LED_BRIGHTNESS_WIRELESS_COMMAND_NAME,
            &brightness.to_string(),
        )
        .await
    }

    /// Get the underglow LED brightness used when the keyboard runs on
    /// battery (`led.brightnessUG.wireless` command).
    ///
    /// One byte, 0–255; same evidence class as
    /// [`DefyKeyboard::get_wireless_led_brightness`].
    pub async fn get_wireless_led_underglow_brightness(&mut self) -> Result<u8, GetSettingError> {
        self.get_setting::<parsing::settings::DecimalU8>(
            Self::LED_BRIGHTNESS_UG_WIRELESS_COMMAND_NAME,
        )
        .await
        .map(|value| value.0)
    }

    /// Set the wireless underglow LED brightness
    /// (`led.brightnessUG.wireless` command).
    pub async fn set_wireless_led_underglow_brightness(
        &mut self,
        brightness: u8,
    ) -> Result<(), SetSettingError> {
        self.set_setting(
            Self::LED_BRIGHTNESS_UG_WIRELESS_COMMAND_NAME,
            &brightness.to_string(),
        )
        .await
    }

    /// Get whether the LEDs fade in on layer changes
    /// (`led.fade` command).
    ///
    /// A wireless-side power optimization (Bazecor exposes it under
    /// energy management, "highlight layer changing"). The read
    /// tolerates trailing tokens after the first decimal, because the
    /// only real `led.fade` response ever captured from a Dygma
    /// firmware reads `"1 0"` (see
    /// [`OnOff`](parsing::settings::OnOff)).
    pub async fn get_led_fade(&mut self) -> Result<bool, GetSettingError> {
        self.get_setting::<parsing::settings::OnOff>(Self::LED_FADE_COMMAND_NAME)
            .await
            .map(|value| value.0)
    }

    /// Set whether the LEDs fade in on layer changes (`led.fade`
    /// command).
    pub async fn set_led_fade(&mut self, fade: bool) -> Result<(), SetSettingError> {
        self.set_setting(
            Self::LED_FADE_COMMAND_NAME,
            parsing::settings::on_off_command_data(fade),
        )
        .await
    }

    /// Get the keyboard LED idle timeout in seconds
    /// (`idleleds.time_limit` command): how long the keyboard waits
    /// without key presses before turning its LEDs off. `0` disables
    /// the timeout.
    ///
    /// The firmware reads and writes the value as one decimal into a
    /// `uint16_t` (`kaleidoscope/plugin/IdleLEDs.cpp`) — NOT the
    /// low-byte/high-byte pair the qukeys and mouse-delay writes need.
    /// Its default is 600 (10 minutes); Bazecor's UI bounds the slider
    /// at 60 minutes (3600), which is a UI choice, not the wire's
    /// domain.
    pub async fn get_idle_leds_time_limit(&mut self) -> Result<u16, GetSettingError> {
        self.get_setting::<parsing::settings::DecimalU16>(Self::IDLELEDS_TIME_LIMIT_COMMAND_NAME)
            .await
            .map(|value| value.0)
    }

    /// Set the keyboard LED idle timeout in seconds
    /// (`idleleds.time_limit` command).
    pub async fn set_idle_leds_time_limit(&mut self, seconds: u16) -> Result<(), SetSettingError> {
        self.set_setting(Self::IDLELEDS_TIME_LIMIT_COMMAND_NAME, &seconds.to_string())
            .await
    }

    /// Get the wireless LED idle timeout in seconds (`idleleds.wireless`
    /// command) — the timeout that applies when the keyboard runs on
    /// battery. `0` disables it.
    ///
    /// One decimal, like the wired timeout; served by the wireless
    /// Neuron's firmware (not in the local checkout), evidenced by
    /// Bazecor and the shipped Defy backups' `idleleds.wireless`
    /// captures.
    pub async fn get_wireless_idle_leds_time_limit(&mut self) -> Result<u16, GetSettingError> {
        self.get_setting::<parsing::settings::DecimalU16>(Self::IDLELEDS_WIRELESS_COMMAND_NAME)
            .await
            .map(|value| value.0)
    }

    /// Set the wireless LED idle timeout in seconds
    /// (`idleleds.wireless` command).
    pub async fn set_wireless_idle_leds_time_limit(
        &mut self,
        seconds: u16,
    ) -> Result<(), SetSettingError> {
        self.set_setting(Self::IDLELEDS_WIRELESS_COMMAND_NAME, &seconds.to_string())
            .await
    }

    /// Get whether true sleep is enabled (`idleleds.true_sleep`
    /// command): when set, the keyboard enters a deeper sleep state
    /// after its wireless LED idle timeout has elapsed.
    ///
    /// One decimal, 0/1, read through
    /// [`OnOff`](parsing::settings::OnOff) (first-token tolerance).
    /// Boards whose firmware does not serve the command are a Bazecor
    /// concern (it hides the card and pins the setting off); this API
    /// surfaces whatever the connected device answers.
    pub async fn get_true_sleep(&mut self) -> Result<bool, GetSettingError> {
        self.get_setting::<parsing::settings::OnOff>(Self::IDLELEDS_TRUE_SLEEP_COMMAND_NAME)
            .await
            .map(|value| value.0)
    }

    /// Set whether true sleep is enabled (`idleleds.true_sleep`
    /// command).
    pub async fn set_true_sleep(&mut self, true_sleep: bool) -> Result<(), SetSettingError> {
        self.set_setting(
            Self::IDLELEDS_TRUE_SLEEP_COMMAND_NAME,
            parsing::settings::on_off_command_data(true_sleep),
        )
        .await
    }

    /// Get the true-sleep timeout in seconds (`idleleds.true_sleep_time`
    /// command): how long after the LEDs turn off the keyboard waits
    /// before entering true sleep. Only meaningful when
    /// [`DefyKeyboard::get_true_sleep`] answers `true`.
    ///
    /// One decimal; Bazecor's UI bounds it at 1–60 minutes (60–3600).
    pub async fn get_true_sleep_time(&mut self) -> Result<u16, GetSettingError> {
        self.get_setting::<parsing::settings::DecimalU16>(
            Self::IDLELEDS_TRUE_SLEEP_TIME_COMMAND_NAME,
        )
        .await
        .map(|value| value.0)
    }

    /// Set the true-sleep timeout in seconds
    /// (`idleleds.true_sleep_time` command).
    pub async fn set_true_sleep_time(&mut self, seconds: u16) -> Result<(), SetSettingError> {
        self.set_setting(
            Self::IDLELEDS_TRUE_SLEEP_TIME_COMMAND_NAME,
            &seconds.to_string(),
        )
        .await
    }

    /// Get one side's battery charge percentage
    /// (`wireless.battery.left.level` / `wireless.battery.right.level`).
    ///
    /// The read hits the device every call — this SDK caches nothing —
    /// which is the behavior Bazecor's battery poll needs (its
    /// `noCacheCommand` reads). The commands are deny-listed from
    /// backups (the `wireless.battery.left`/`right` prefixes in
    /// `Backup.ts`), so no real capture exists; the value is typed as
    /// the 0-100 percentage `FOCUS_API.md` documents, and a device
    /// that answered outside that domain fails the read with the
    /// value named instead of misreporting a charge. Wireless
    /// commands are only served by wireless hardware
    /// (`FOCUS_API.md`), so a wired Defy's firmware does not answer
    /// them at all.
    pub async fn get_battery_level(
        &mut self,
        side: parsing::wireless::BatterySide,
    ) -> Result<parsing::wireless::BatteryLevel, GetSettingError> {
        let command = match side {
            parsing::wireless::BatterySide::Left => Self::WIRELESS_BATTERY_LEFT_LEVEL_COMMAND_NAME,
            parsing::wireless::BatterySide::Right => {
                Self::WIRELESS_BATTERY_RIGHT_LEVEL_COMMAND_NAME
            }
        };

        self.get_wireless_setting(command).await
    }

    /// Get one side's battery status
    /// (`wireless.battery.left.status` /
    /// `wireless.battery.right.status`).
    ///
    /// The five documented codes are
    /// [`BatteryStatus::Discharging`](parsing::wireless::BatteryStatus::Discharging),
    /// [`Charging`](parsing::wireless::BatteryStatus::Charging),
    /// [`Full`](parsing::wireless::BatteryStatus::Full),
    /// [`Fault`](parsing::wireless::BatteryStatus::Fault), and
    /// [`Disconnected`](parsing::wireless::BatteryStatus::Disconnected);
    /// the firmware is also known to serve hex words in the wild,
    /// which arrive as
    /// [`BatteryStatus::Unknown`](parsing::wireless::BatteryStatus::Unknown)
    /// with their parsed value — the same responses Bazecor's poll
    /// collapses into one fatal-error state. Same evidence class as
    /// [`DefyKeyboard::get_battery_level`].
    pub async fn get_battery_status(
        &mut self,
        side: parsing::wireless::BatterySide,
    ) -> Result<parsing::wireless::BatteryStatus, GetSettingError> {
        let command = match side {
            parsing::wireless::BatterySide::Left => Self::WIRELESS_BATTERY_LEFT_STATUS_COMMAND_NAME,
            parsing::wireless::BatterySide::Right => {
                Self::WIRELESS_BATTERY_RIGHT_STATUS_COMMAND_NAME
            }
        };

        self.get_wireless_setting(command).await
    }

    /// Get whether the battery-saving mode is active
    /// (`wireless.battery.savingMode` command). The firmware enables
    /// it automatically at low charge; it can also be set manually.
    pub async fn get_battery_saving_mode(&mut self) -> Result<bool, GetSettingError> {
        self.get_setting::<parsing::settings::OnOff>(
            Self::WIRELESS_BATTERY_SAVING_MODE_COMMAND_NAME,
        )
        .await
        .map(|value| value.0)
    }

    /// Set the battery-saving mode (`wireless.battery.savingMode`
    /// command).
    ///
    /// A write over the Bluetooth transport is subject to the upstream
    /// BLE large-payload failure (issue #11) — prefer a wired or RF
    /// connection for writes.
    pub async fn set_battery_saving_mode(&mut self, enabled: bool) -> Result<(), SetSettingError> {
        self.set_setting(
            Self::WIRELESS_BATTERY_SAVING_MODE_COMMAND_NAME,
            parsing::settings::on_off_command_data(enabled),
        )
        .await
    }

    /// Force the Neuron to re-read both sides' battery state
    /// (`wireless.battery.forceRead` command).
    ///
    /// The command has no direct output (`FOCUS_API.md`); the sides'
    /// level and status answers update a couple of seconds later, so
    /// follow it with
    /// [`DefyKeyboard::get_battery_level`] /
    /// [`DefyKeyboard::get_battery_status`] — Bazecor's UI re-polls
    /// immediately after sending it, retrying (up to five reads, 500 ms
    /// apart) while a side still answers disconnected. No capture of
    /// this command's response exists in any of Bazecor's data
    /// sources, so no response shape is claimed here. Same write
    /// caveat as
    /// [`DefyKeyboard::set_battery_saving_mode`].
    pub async fn force_battery_read(&mut self) -> Result<(), RunCommandError> {
        self.run_command(Self::WIRELESS_BATTERY_FORCE_READ_COMMAND_NAME, None)
            .await?;

        Ok(())
    }

    /// Get the RF signal strength (`wireless.rf.power` command): how
    /// strongly the Neuron talks to the sides. Lower means less range
    /// and better battery life.
    pub async fn get_rf_power(&mut self) -> Result<parsing::wireless::RfPower, GetSettingError> {
        self.get_wireless_setting(Self::WIRELESS_RF_POWER_COMMAND_NAME)
            .await
    }

    /// Set the RF signal strength (`wireless.rf.power` command).
    ///
    /// The three levels
    /// ([`Low`](parsing::wireless::RfPower::Low) /
    /// [`Medium`](parsing::wireless::RfPower::Medium) /
    /// [`High`](parsing::wireless::RfPower::High)) are the documented
    /// wire domain. Same write caveat as
    /// [`DefyKeyboard::set_battery_saving_mode`].
    pub async fn set_rf_power(
        &mut self,
        power: parsing::wireless::RfPower,
    ) -> Result<(), SetSettingError> {
        self.set_setting(
            Self::WIRELESS_RF_POWER_COMMAND_NAME,
            power.to_command_data(),
        )
        .await
    }

    /// Get whether the RF link hops channels
    /// (`wireless.rf.channelHop` command).
    ///
    /// The command is absent from `FOCUS_API.md` and from this
    /// Bazecor's wireless UI (the view only reads it and writes the
    /// value back), so the setting's observable semantics rest on the
    /// command's name; the captured reads are a decimal `0` on real
    /// devices and the bool word `false` on the app's virtual
    /// keyboards — both parse.
    pub async fn get_rf_channel_hop(&mut self) -> Result<bool, GetSettingError> {
        self.get_wireless_setting::<parsing::wireless::RfChannelHop>(
            Self::WIRELESS_RF_CHANNEL_HOP_COMMAND_NAME,
        )
        .await
        .map(|value| value.0)
    }

    /// Set whether the RF link hops channels
    /// (`wireless.rf.channelHop` command), as the decimal the real
    /// devices read back. Same write caveat as
    /// [`DefyKeyboard::set_battery_saving_mode`].
    pub async fn set_rf_channel_hop(&mut self, enabled: bool) -> Result<(), SetSettingError> {
        self.set_setting(
            Self::WIRELESS_RF_CHANNEL_HOP_COMMAND_NAME,
            parsing::wireless::rf_channel_hop_command_data(enabled),
        )
        .await
    }

    /// Re-sync the Neuron with both sides over the RF link
    /// (`wireless.rf.syncPairing` command), rebuilding the shared
    /// pairing identity.
    ///
    /// `FOCUS_API.md` documents the operation as usable only in wired
    /// mode with all cables connected. The command has no direct
    /// output; the shipped backups capture it as an empty response.
    /// Same write caveat as
    /// [`DefyKeyboard::set_battery_saving_mode`].
    pub async fn sync_rf_pairing(&mut self) -> Result<(), RunCommandError> {
        self.run_command(Self::WIRELESS_RF_SYNC_PAIRING_COMMAND_NAME, None)
            .await?;

        Ok(())
    }

    /// Get the keyboard's Bluetooth device name
    /// (`wireless.bluetooth.deviceName` command).
    ///
    /// The name travels on the wire unquoted and may be empty — every
    /// shipped Defy backup captures the factory empty string. The
    /// command is not in `FOCUS_API.md`; the read-and-write-verbatim
    /// behavior is Bazecor's.
    pub async fn get_bluetooth_device_name(
        &mut self,
    ) -> Result<parsing::wireless::DeviceName, GetSettingError> {
        let res = self
            .run_command(Self::WIRELESS_BLUETOOTH_DEVICE_NAME_COMMAND_NAME, None)
            .await
            .map_err(GetSettingError::CommandFailed)?;

        Ok(parsing::wireless::DeviceName::new(res))
    }

    /// Set the keyboard's Bluetooth device name
    /// (`wireless.bluetooth.deviceName` command), sent verbatim and
    /// unquoted like Bazecor's write. How the firmware reads a
    /// multi-word name back is unverified — no wireless firmware
    /// source is in the local checkout, and `FOCUS_API.md` does not
    /// document the command.
    ///
    /// An empty name writes an empty payload, which the firmware
    /// answers as a read — a silent no-op, not a blank name. Same
    /// write caveat as
    /// [`DefyKeyboard::set_battery_saving_mode`].
    pub async fn set_bluetooth_device_name(
        &mut self,
        name: &parsing::wireless::DeviceName,
    ) -> Result<(), SetSettingError> {
        self.set_setting(
            Self::WIRELESS_BLUETOOTH_DEVICE_NAME_COMMAND_NAME,
            &name.to_command_data(),
        )
        .await
    }

    /// Read one of the five Bluetooth info channels
    /// (`wireless.bluetooth.infoChannel <1..5>` command).
    ///
    /// The response is served verbatim, trimmed of the separator
    /// whitespace a Focus response carries: Bazecor reads channels
    /// 1-5, stores each answer as a raw string, and never renders or
    /// interprets it, and no capture of any response exists in any of
    /// its data sources — so this claims nothing about the content.
    /// Same evidence class as
    /// [`DefyKeyboard::get_bluetooth_device_name`].
    pub async fn get_bluetooth_info_channel(
        &mut self,
        channel: parsing::wireless::BluetoothChannelId,
    ) -> Result<String, GetSettingError> {
        let res = self
            .run_command(
                Self::WIRELESS_BLUETOOTH_INFO_CHANNEL_COMMAND_NAME,
                Some(&channel.command_data()),
            )
            .await
            .map_err(GetSettingError::CommandFailed)?;

        Ok(res.trim().to_owned())
    }

    /// Reset every plugin setting stored in the device's EEPROM to the
    /// erased state.
    ///
    /// This reads `eeprom.contents` to learn how many bytes the device
    /// stores, then writes back the same number of `255` bytes. It
    /// wipes **all** of them: qukeys, superkeys timings, mouse
    /// settings, LED settings, and everything else plugins keep there.
    /// Keymaps, superkey maps, and macro maps live in their own EEPROM
    /// slices and are wiped too. This cannot be undone.
    ///
    /// Fails with [`ResetEepromError::EmptyEeprom`] when the device
    /// reports an empty EEPROM: there would be nothing to erase, and
    /// the empty write would be answered as a read by the firmware.
    pub async fn reset_eeprom(&mut self) -> Result<(), ResetEepromError> {
        let contents = self
            .get_setting::<parsing::settings::EepromContents>(Self::EEPROM_CONTENTS_COMMAND_NAME)
            .await
            .map_err(ResetEepromError::ReadContents)?;

        let data = contents
            .erase_command_data()
            .map_err(ResetEepromError::EmptyEeprom)?;

        self.set_setting(Self::EEPROM_CONTENTS_COMMAND_NAME, &data)
            .await
            .map_err(ResetEepromError::Erase)?;

        Ok(())
    }

    /// Captures a full configuration backup from the device, in the
    /// Bazecor-compatible backup file format
    /// ([`parsing::backup::DefyBackup`]).
    ///
    /// The capture list is the device's `help` output minus the
    /// deny-list (`parsing::backup::backup_capture_commands`), exactly
    /// what Bazecor captures; every listed command is sent as a *read*
    /// and its response stored verbatim, so creating a backup never
    /// writes to the device and is always safe to run — e.g. before an
    /// apply. Captured `keymap.custom`/`superkeys.map`/`macros.map`
    /// data is preserved as the device sent it (padded, with trailing
    /// separators), not transformed the way Bazecor post-processes its
    /// captures.
    ///
    /// The file's neuron object carries the chip id (empty when that
    /// probe was unavailable — see # Interrupted captures) and the
    /// Defy device descriptor, but no names: layer, macro, and
    /// superkey names are app-side in Bazecor, and a capture never
    /// attaches them — the `names` store applies them on restore (see
    /// [`parsing::backup::BackupNeuron`]).
    ///
    /// # Interrupted captures
    ///
    /// When one of the capture-list command reads fails mid-capture,
    /// the error carries the *partial capture* in
    /// [`CreateBackupError::IncompleteCapture`]: everything captured
    /// before the failure, marked
    /// ([`parsing::backup::PartialCapture`]) with the capture-list
    /// commands it is missing. Save it (the CLI does
    /// automatically); it is never mistaken for a complete capture
    /// because the marker is on it, and it can be completed later with
    /// [`DefyKeyboard::complete_backup`] (capture commands are
    /// idempotent reads, so re-capturing the missing ones is safe).
    ///
    /// The identity probes are **not** part of this: they are
    /// best-effort, and a capture whose every command was captured is
    /// complete — marker-free, restorable, Bazecor-shaped — even when
    /// the device provided no identity at all. A probe the device's
    /// `help` output does not list is never sent (the wireless Defy's
    /// firmware does not serve `hardware.wireless` and answers any
    /// attempt with an empty response — sending it would abort an
    /// otherwise complete capture on exactly the device that most
    /// needs backups), and a served probe that fails — a transport
    /// error, an empty response, a response that does not parse — is
    /// recorded as unavailable instead of failing the capture. What
    /// was not obtained is reported in the returned
    /// [`CaptureIdentity`]; the file itself carries the identity that
    /// was obtained: an empty `neuronID` when the chip id is unknown,
    /// an empty `versions` object when the version is unknown, and a
    /// device descriptor recording the *wired* variant when
    /// `hardware.wireless` is unknown. That last choice is safe to
    /// make blind: restore validation
    /// ([`DefyKeyboard::backup_restore_plan`]) reads only
    /// `neuron.device.info.product` out of the descriptor — the
    /// `wireless` flag is inert for it, and Bazecor's restore flow
    /// likewise reads only the `backup` array, the neuron object, and
    /// `neuron.device.info.product`.
    pub async fn create_backup(
        &mut self,
    ) -> Result<(parsing::backup::DefyBackup, CaptureIdentity), CreateBackupError> {
        capture_backup(&mut self.focus_api, &[]).await
    }

    /// Completes a partial capture: captures **only the commands the
    /// partial file is missing**, merges them into the file's entries,
    /// re-reads the identity probes, and recomputes the partial marker
    /// against the device's *current* capture list.
    ///
    /// Every capture command is an idempotent read, so re-capturing
    /// the missing ones never writes to the device and cannot corrupt
    /// anything — completing a partial capture is exactly as safe as
    /// creating one. The file's existing entries are kept verbatim, in
    /// their original capture order; the newly captured ones are
    /// appended in the current capture order. A command the file holds
    /// that the current capture list does not mention (the device's
    /// `help` changed, say) is kept and counts against nothing.
    ///
    /// On success the returned backup is complete: the marker is gone
    /// and the identity fields describe this device. When the
    /// completion is itself interrupted the error carries the updated
    /// partial capture — same shape as
    /// [`DefyKeyboard::create_backup`]'s failure, with the missing
    /// list recomputed against the current capture list.
    ///
    /// The identity fields (chip id, versions, device descriptor) are
    /// always re-probed from the connected device — they replace
    /// whatever the partial file carried (which, for a file an
    /// interrupted [`DefyKeyboard::create_backup`] saved, is the empty
    /// store shape anyway: the interrupted probes are exactly what
    /// completion re-runs). The probes are best-effort exactly as in
    /// [`DefyKeyboard::create_backup`]: a probe the device does not
    /// serve or that fails is reported as unavailable, and the marker
    /// clears whenever every capture-list command is present — a
    /// marker-only partial (a file saved by an earlier version of this
    /// crate from a capture that died at the probes, as the wireless
    /// Defy's older captures were) completes even on a device that
    /// serves none of the probes.
    pub async fn complete_backup(
        &mut self,
        partial: &parsing::backup::DefyBackup,
    ) -> Result<(parsing::backup::DefyBackup, CaptureIdentity), CreateBackupError> {
        capture_backup(&mut self.focus_api, &partial.backup).await
    }

    /// Builds the Defy device descriptor a backup's neuron object
    /// carries, as Bazecor ships it in its Defy backups: the static
    /// descriptor with the per-device `wireless` flag and `chipId`
    /// filled in.
    ///
    /// Bazecor's restore flow reads `neuron.device.info.product` out of
    /// a backup file, so a backup without a descriptor is not
    /// restorable there; captures made through
    /// [`DefyKeyboard::create_backup`] always carry this descriptor.
    /// When the capture could not probe `hardware.wireless`, the
    /// descriptor records the wired variant (`false`) — a documented
    /// blind choice, inert for restore validation (both this crate's
    /// and Bazecor's restore read only `info.product`); see
    /// [`DefyKeyboard::create_backup`].
    pub fn backup_device_descriptor(wireless: bool, chip_id: &str) -> serde_json::Value {
        let mut descriptor = backup_device_descriptor_template();

        descriptor["wireless"] = wireless.into();
        descriptor["chipId"] = chip_id.into();

        descriptor
    }

    /// Computes the exact command sequence
    /// [`DefyKeyboard::restore_backup`] sends for a backup, validating
    /// the backup first — **without opening a connection to the
    /// device**.
    ///
    /// A backup carrying the partial-capture marker
    /// ([`parsing::backup::PartialCapture`]) is refused: it does not
    /// hold the whole device state a capture captures. See
    /// [`DefyKeyboard::backup_restore_plan_allowing_partial`] for the
    /// explicit override.
    ///
    /// The plan is Bazecor's restore order
    /// ([`restore_command_sequence`][seq]: `superkeys.map` entries
    /// before `keymap.custom`, empty-command capture artifacts dropped)
    /// with the trailing `led.mode 0` Bazecor appends after the
    /// sequence ([`Backup.ts`][bazecor] `restoreBackup`) added as the
    /// last entry, so the plan *is* the complete device write sequence.
    ///
    /// Validation, before anything is returned: every captured command
    /// this crate can prove things about is parsed against its wire
    /// format — `keymap.custom`, `superkeys.map`, `macros.map`,
    /// `palette`, and `colormap.map` — the maps through the typed
    /// shapes, which also enforce the firmware's addressing limits and
    /// (for the keymap and the colormap) the exactly-ten-layer write
    /// shape by construction — and the backup's device
    /// descriptor, when it carries one, must name the Defy. Everything
    /// else passes through verbatim the way Bazecor's restore sends it.
    /// An invalid backup fails here with a typed error, so
    /// [`DefyKeyboard::restore_backup`] never sends a partial restore.
    ///
    /// [seq]: crate::focus_api::parsing::backup::restore_command_sequence
    /// [bazecor]: https://github.com/Dygmalab/Bazecor/blob/main/src/api/backup/Backup.ts
    pub fn backup_restore_plan(
        backup: &parsing::backup::DefyBackup,
    ) -> Result<Vec<parsing::backup::BackupCommand>, BackupValidationError> {
        Self::backup_restore_plan_with(backup, false)
    }

    /// [`DefyKeyboard::backup_restore_plan`] with the
    /// partial-capture refusal lifted: an explicitly
    /// `--allow-partial`-style restore of a marked backup gets its
    /// plan computed like any other, after acknowledging it restores
    /// only part of a captured state.
    ///
    /// Everything else — the product guard, the per-entry wire
    /// validation, the ordering, the `led.mode 0` tail — is identical.
    /// The marker is data, not damage: the plan restores exactly what
    /// the file holds.
    pub fn backup_restore_plan_allowing_partial(
        backup: &parsing::backup::DefyBackup,
    ) -> Result<Vec<parsing::backup::BackupCommand>, BackupValidationError> {
        Self::backup_restore_plan_with(backup, true)
    }

    /// The plan computation behind the refusal pair: `allow_partial`
    /// says whether a backup carrying the partial-capture marker is
    /// planned anyway.
    ///
    /// The skeleton ([`plan::restore_plan`][core]) holds the
    /// product-independent shape — the partial refusal, the product
    /// guard, the shared superkeys/macros entries, the plan order, the
    /// `led.mode 0` tail — and [`DefyShapes`] names the Defy's own
    /// pieces: the product name, the refusals' wording, and the
    /// Defy-shaped entries.
    ///
    /// [core]: plan::restore_plan
    fn backup_restore_plan_with(
        backup: &parsing::backup::DefyBackup,
        allow_partial: bool,
    ) -> Result<Vec<parsing::backup::BackupCommand>, BackupValidationError> {
        super::plan::restore_plan::<DefyShapes>(backup, allow_partial)
    }

    /// Restores a captured backup to the device: validates it
    /// ([`DefyKeyboard::backup_restore_plan`]) and then sends every
    /// planned command with its data, in Bazecor's restore order,
    /// ending with the `led.mode 0` tail Bazecor's restore sends.
    ///
    /// A backup carrying the partial-capture marker is refused before
    /// anything is sent — restoring it would write a half-captured
    /// configuration to the device. The explicit override is
    /// [`DefyKeyboard::restore_backup_allowing_partial`].
    ///
    /// This **rewrites the captured parts of the device's
    /// configuration** — the keymap, superkeys, macros, settings, and
    /// LED state the backup holds. It cannot be undone except by
    /// restoring another backup; capture one with
    /// [`DefyKeyboard::create_backup`] first.
    ///
    /// Data is trimmed at send time and an empty payload is sent as a
    /// bare command (a read), exactly how Bazecor's restore treats it
    /// (`String(val).trim()` in `Backup.ts` `restoreBackup`; the
    /// firmware answers an empty write as a read). A send failure stops
    /// the restore at that command with [`RestoreBackupError::Send`],
    /// leaving everything sent before it applied and the rest not —
    /// capture a fresh backup before retrying.
    pub async fn restore_backup(
        &mut self,
        backup: &parsing::backup::DefyBackup,
    ) -> Result<(), RestoreBackupError> {
        self.send_restore_plan(Self::backup_restore_plan(backup))
            .await
    }

    /// [`DefyKeyboard::restore_backup`] with the partial-capture
    /// refusal lifted: the marked backup's plan is computed with
    /// [`DefyKeyboard::backup_restore_plan_allowing_partial`] and sent
    /// like any other. Everything the file holds is restored, in the
    /// usual order; what the file is missing stays whatever it is —
    /// the caller acknowledged the marker (the CLI prints the missing
    /// list before this is reachable).
    pub async fn restore_backup_allowing_partial(
        &mut self,
        backup: &parsing::backup::DefyBackup,
    ) -> Result<(), RestoreBackupError> {
        self.send_restore_plan(Self::backup_restore_plan_allowing_partial(backup))
            .await
    }

    /// The device seam behind the restore pair: validate (the `Result`
    /// carries the validation outcome), then send.
    async fn send_restore_plan(
        &mut self,
        plan: Result<Vec<parsing::backup::BackupCommand>, BackupValidationError>,
    ) -> Result<(), RestoreBackupError> {
        let plan = plan.map_err(RestoreBackupError::Validation)?;

        // The device seam of the restore: the only place backup
        // commands are sent, a flat loop over the validated plan —
        // every write this backup performs goes through here, and
        // nothing is sent before `backup_restore_plan` accepted the
        // whole backup.
        for entry in &plan {
            let data = entry.data.trim();

            self.run_command(
                &entry.command,
                if data.is_empty() { None } else { Some(data) },
            )
            .await
            .map_err(|source| RestoreBackupError::Send {
                command: entry.command.clone(),
                source,
            })?;
        }

        Ok(())
    }

    /// Runs a single-value settings command and parses its response.
    async fn get_setting<T>(&mut self, command: &str) -> Result<T, GetSettingError>
    where
        T: std::str::FromStr<Err = parsing::settings::ParseSettingsError>,
    {
        read_setting::<T>(&mut self.focus_api, command).await
    }

    /// Runs a single-value wireless command and parses its response —
    /// [`DefyKeyboard::get_setting`] with the wireless parse-error
    /// bound. Both it and [`DefyKeyboard::get_setting`] share the one
    /// generic read helper ([`read_setting`]), which absorbs either
    /// parser's error through `GetSettingError`'s `From` conversions.
    async fn get_wireless_setting<T>(&mut self, command: &str) -> Result<T, GetSettingError>
    where
        T: std::str::FromStr<Err = parsing::wireless::ParseWirelessError>,
    {
        read_setting::<T>(&mut self.focus_api, command).await
    }

    /// Runs a settings command with the given write data.
    async fn set_setting(&mut self, command: &str, data: &str) -> Result<(), SetSettingError> {
        self.run_command(command, Some(data)).await?;

        Ok(())
    }
}

/// Runs a single-value settings command over any connection and parses
/// its response — [`DefyKeyboard::get_setting`]'s body, shared with the
/// backup capture core and the wireless reads so every single-value
/// command runs exactly one way. The parse error lands in
/// [`GetSettingError`] through its `From` conversions, which cover
/// every parse-error type the settings and wireless parsers produce.
async fn read_setting<T>(
    connection: &mut impl FocusApiConnection,
    command: &str,
) -> Result<T, GetSettingError>
where
    T: std::str::FromStr,
    T::Err: Into<GetSettingError>,
{
    let res = connection
        .run_command(command, None)
        .await
        .map_err(GetSettingError::CommandFailed)?;

    T::from_str(&res).map_err(Into::into)
}

/// The capture core of [`DefyKeyboard::create_backup`] and
/// [`DefyKeyboard::complete_backup`], over any connection so tests can
/// drive it against scripted responses.
///
/// `already_captured` is the entries a partial file being completed
/// already holds (empty for a fresh capture). The steps, in order:
/// query `help` and compute the capture list; capture only the
/// capture-list commands the file does not hold yet (multiset
/// semantics — a twice-listed command needs two captures); then probe
/// the identity fields, best-effort ([`probe_identity`]). A failed
/// command read assembles the partial capture — everything captured
/// so far, plus the [`parsing::backup::PartialCapture`] marker naming
/// what is missing — and returns it in
/// [`CreateBackupError::IncompleteCapture`]; a `help` failure cannot
/// produce one worth keeping, so only the error remains. A probe
/// failure never assembles anything: the identity the device did
/// provide fills the file, the rest is reported in the returned
/// [`CaptureIdentity`], and the marker is computed from the captured
/// commands alone.
async fn capture_backup(
    connection: &mut impl FocusApiConnection,
    already_captured: &[parsing::backup::BackupCommand],
) -> Result<(parsing::backup::DefyBackup, CaptureIdentity), CreateBackupError> {
    let help_commands = connection
        .available_commands()
        .await
        .map_err(CreateBackupError::ListCommands)?;
    let capture_list = parsing::backup::backup_capture_commands(&help_commands);
    let missing = parsing::backup::missing_capture_commands(&capture_list, already_captured);

    // Sequential on purpose: each command is a device round-trip, and
    // the capture order is the `help` order the format keeps.
    let mut captured = Vec::with_capacity(missing.len());

    for command in &missing {
        // A failed read contributes no entry: the partial marker is
        // recomputed from what actually was captured, and names the
        // failed command among the missing.
        let data = match connection.run_command(command, None).await {
            Ok(data) => data,
            Err(source) => {
                return Err(interrupted(
                    &capture_list,
                    already_captured,
                    &captured,
                    command.clone(),
                    source,
                ));
            }
        };

        captured.push(parsing::backup::BackupCommand::new(command.clone(), data));
    }

    // The identity probes: best-effort, in probe order. A probe the
    // device does not serve is never sent; a served probe that fails
    // or answers nothing usable is recorded as unavailable — neither
    // can lose the commands captured above.
    let chip_id = probe_identity::<parsing::settings::ChipId>(
        connection,
        &help_commands,
        DefyKeyboard::HARDWARE_CHIP_ID_COMMAND_NAME,
    )
    .await
    .map(|chip_id| chip_id.as_str().to_owned());

    let version = probe_identity::<String>(
        connection,
        &help_commands,
        DefyKeyboard::VERSION_COMMAND_NAME,
    )
    .await;

    let wireless = probe_identity::<parsing::settings::BoolWord>(
        connection,
        &help_commands,
        DefyKeyboard::HARDWARE_WIRELESS_COMMAND_NAME,
    )
    .await
    .map(|word| word.0);

    let identity = CaptureIdentity {
        chip_id,
        version,
        wireless,
    };

    let mut entries = already_captured.to_owned();
    entries.extend(captured);

    // The final marker is computed against the current capture list:
    // if the device's `help` changed since the partial file was
    // captured, the file's marker describes the capture it belongs to,
    // this one the device answers now.
    let partial = parsing::backup::PartialCapture::from_capture(&capture_list, &entries);

    // The complete capture's assembly — chip id, descriptor, versions
    // — is `DefyBackup::capture`'s shape; the marker is this path's
    // only addition, present only when something is still missing.
    // Unknown identity fills from the empty side: no chip id reads as
    // the empty neuron id, no version as the empty versions object,
    // and no `hardware.wireless` as the wired descriptor flag — the
    // documented blind choice, inert for restore validation (see
    // [`DefyKeyboard::create_backup`]). The chip id is resolved once:
    // the file's `neuronID` and the descriptor's `chipId` are the same
    // value by construction, not by two agreeing fallbacks.
    let chip_id = identity.chip_id.as_deref().unwrap_or("");

    let mut backup = parsing::backup::DefyBackup::capture(
        chip_id,
        parsing::backup::BackupVersions::from_version_response(
            identity.version.as_deref().unwrap_or(""),
        ),
        entries,
        DefyKeyboard::backup_device_descriptor(identity.wireless.unwrap_or(false), chip_id),
    );

    backup.partial = partial.has_missing().then_some(partial);

    // The multiset invariant: `has_missing` was computed from exactly
    // these `entries` against the capture list, so a success return
    // (only taken when nothing is missing) provably carries no marker
    // — a marked file here would let a partial capture print as a
    // complete one downstream.
    debug_assert!(!backup.is_partial());

    Ok((backup, identity))
}

/// Assembles the interrupted-capture error: the partial backup holding
/// `already_captured` (the completed file's entries) plus `captured`
/// (this run's entries, in capture order), marked against the capture
/// list.
fn interrupted(
    capture_list: &[String],
    already_captured: &[parsing::backup::BackupCommand],
    captured: &[parsing::backup::BackupCommand],
    command: String,
    source: RunCommandError,
) -> CreateBackupError {
    let mut entries = already_captured.to_owned();
    entries.extend(captured.iter().cloned());

    CreateBackupError::IncompleteCapture {
        partial: Box::new(parsing::backup::DefyBackup::partial_capture(
            entries,
            capture_list,
        )),
        command,
        source,
    }
}

/// The Defy's restore-validation shapes: the pieces
/// [`plan::restore_plan`](super::plan::restore_plan) wires together —
/// the product name the guard matches, the refusals' wording, and the
/// Defy-shaped entries (the keymap, palette, and colormap; the
/// superkeys and macros maps are the skeleton's).
struct DefyShapes;

impl super::plan::ProductRestoreShapes for DefyShapes {
    type ValidationError = BackupValidationError;
    type EntryError = BackupEntryError;
    const EXPECTED_PRODUCT: &'static str = "Defy";

    fn wrong_product(product: String) -> BackupValidationError {
        BackupValidationError::NotADefyBackup { product }
    }

    fn refused_partial(missing: Vec<String>) -> BackupValidationError {
        BackupValidationError::PartialCapture { missing }
    }

    fn invalid_entry(command: String, source: BackupEntryError) -> BackupValidationError {
        BackupValidationError::InvalidEntry { command, source }
    }

    fn shared_map_error(source: super::plan::SharedMapError) -> BackupEntryError {
        match source {
            super::plan::SharedMapError::Superkeys(source) => BackupEntryError::Superkeys(source),
            super::plan::SharedMapError::Macros(source) => BackupEntryError::Macros(source),
        }
    }

    fn validate_product_entry(command: &str, data: &str) -> Option<Result<(), BackupEntryError>> {
        match command {
            // A `keymap.custom` write carries exactly the ten layers;
            // [`DefyKeymap`] cannot be constructed from anything else,
            // so the parse here is the whole validation.
            DefyKeyboard::KEYMAP_CUSTOM_COMMAND_NAME => Some(
                DefyKeymap::from_str(data)
                    .map(|_| ())
                    .map_err(BackupEntryError::Keymap),
            ),
            // A Defy `palette` write is exactly 16 RGBW colors; the
            // type cannot be constructed from anything else.
            DefyKeyboard::PALETTE_COMMAND_NAME => Some(
                parsing::palette::Palette::from_str(data)
                    .map(|_| ())
                    .map_err(BackupEntryError::Palette),
            ),
            // A `colormap.map` write carries exactly the ten layers;
            // [`DefyColormap`] cannot be constructed from anything
            // else.
            DefyKeyboard::COLORMAP_MAP_COMMAND_NAME => Some(
                DefyColormap::from_str(data)
                    .map(|_| ())
                    .map_err(BackupEntryError::Colormap),
            ),
            _ => None,
        }
    }
}

/// Builds the static part of the Defy device descriptor a backup
/// carries: exactly the object Bazecor ships in its Defy backup files
/// (`src/defaultBackups/Defy-*-backup.json`, `neuron.device`), minus
/// the per-device `wireless` and `chipId` fields, which
/// [`DefyKeyboard::backup_device_descriptor`] fills in.
///
/// The two shipped Defy backups carry byte-identical descriptors
/// apart from `wireless`; the key set and every value below are
/// theirs, and kept in sync with them by
/// [`backup_device_descriptor_matches_bazecors_shipped_descriptor`].
fn backup_device_descriptor_template() -> serde_json::Value {
    serde_json::json!({
        "info": {
            "vendor": "Dygma",
            "product": "Defy",
            "keyboardType": "wireless",
            "displayName": "Dygma Defy wireless",
            "urls": [
                { "name": "Homepage", "url": "https://www.dygma.com/defy/" }
            ]
        },
        "usb": { "vendorId": 13807, "productId": 18 },
        "keyboard": {
            "rows": 5,
            "columns": 16,
            "left": [
                [0, 1, 2, 3, 4, 5, 6],
                [16, 17, 18, 19, 20, 21, 22],
                [32, 33, 34, 35, 36, 37, 38],
                [48, 49, 50, 51, 52, 53],
                [64, 65, 66, 67, 68, 69, 70, 71]
            ],
            "right": [
                [9, 10, 11, 12, 13, 14, 15],
                [25, 26, 27, 28, 29, 30, 31],
                [41, 42, 43, 44, 45, 46, 47],
                [57, 58, 59, 60, 61, 62, 63],
                [72, 73, 74, 75, 76, 77, 78, 79]
            ],
            "ledsLeft": [
                0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16,
                17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31,
                32, 33, 34
            ],
            "ledsRight": [
                35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49,
                50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64,
                65, 66, 67, 68, 69
            ]
        },
        "keyboardUnderglow": {
            "rows": 2,
            "columns": 89,
            "ledsLeft": [
                70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84,
                85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95, 96, 97, 98, 99,
                100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111,
                112, 113, 114, 115, 116, 117, 118, 119, 120, 121, 122
            ],
            "ledsRight": [
                123, 124, 125, 126, 127, 128, 129, 130, 131, 132, 133, 134,
                135, 136, 137, 138, 139, 140, 141, 142, 143, 144, 145, 146,
                147, 148, 149, 150, 151, 152, 153, 154, 155, 156, 157, 158,
                159, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 170,
                171, 172, 173, 174, 175
            ]
        },
        "RGBWMode": true,
        "components": {},
        "instructions": {
            "en": {
                "updateInstructions": "To update the firmware, the keyboard needs a special reset. When the countdown starts, press and hold the Escape key. Soon after the countdown finished, the Neuron's light should start a blue pulsing pattern, and the flashing will proceed. At this point, you should release the Escape key."
            }
        },
        "wireless": true,
        "chipId": ""
    })
}

/// Static dispatch for focus API connections.
///
/// Product-agnostic by design — it dispatches transports, not products
/// ([`DygmaConnection`](super::connection::DygmaConnection) builds on
/// it for every supported keyboard). It and the
/// `hardware.layout` command name live here only for history: moving
/// both to a neutral devices-level module is the recorded
/// post-commit systemic-pass queue item (see FEATURE-PARITY.md, the
/// #24 unit-4 gauntlet disposition).
#[derive(Debug, From)]
pub enum DynFocusApi {
    /// Connections to the device over serial port,
    /// which includes both wired over USB, as well as
    /// wireless over RF.
    Serial(SerialPortFocusApi),
    /// Connections to the device over BTLE.
    Bluetooth(HidFocusApi),
}

impl FocusApiConnection for DynFocusApi {
    async fn run_command(
        &mut self,
        command: &str,
        data: Option<&str>,
    ) -> Result<String, crate::focus_api::RunCommandError> {
        match self {
            Self::Serial(sp) => sp.run_command(command, data).await,
            Self::Bluetooth(hid) => hid.run_command(command, data).await,
        }
    }
}

/// Structure representing the physical layout of the Defy keyboard.
#[derive(Clone, Copy, Debug)]
pub struct DefyLayout {
    /// Left half of the keyboard.
    pub left: DefyLayoutHalf,
    /// Right half of the keyboard.
    pub right: DefyLayoutHalf,
}

/// Right half layout of the Defy keyboard.
#[derive(Clone, Copy, Debug)]
pub struct DefyLayoutHalf {
    /// Row 1 key indices.
    pub row_1: [u8; 7],
    /// Row 2 key indices.
    pub row_2: [u8; 7],
    /// Row 3 key indices.
    pub row_3: [u8; 7],
    /// Row 4 key indices.
    pub row_4: [u8; 6],
    /// Thumb cluster layout.
    pub thumb_cluster: DefyThumbClusterLayout,
}

/// Thumb cluster layout of the Defy keyboard.
#[derive(Clone, Copy, Debug)]
pub struct DefyThumbClusterLayout {
    /// The top 4 keys of the thumb cluster.
    pub top: [u8; 4],
    /// The bottom 4 keys of the thumb cluster.
    pub bottom: [u8; 4],
}

/// Full Defy keymap.
///
/// A Defy keymap holds exactly
/// [`KEYMAP_CUSTOM_COMMAND_LAYERS`]
/// layers — the device stores exactly that many, and every
/// `keymap.custom` write carries exactly that many — so a
/// [`DefyKeymap`] cannot hold a different number of layers by
/// construction. Building one from a count that is not known at compile
/// time (wire data, a JSON file) is fallible: it goes through
/// [`TryFrom<Vec<DefyKeymapLayer>>`](TryFrom) and fails with
/// [`KeymapDoesNotHave10LayersError`] when the count is wrong.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Deref, DerefMut, From)]
pub struct DefyKeymap {
    /// The keymap's ten layers, in wire order (layer 1 first).
    pub layers: [DefyKeymapLayer; KEYMAP_CUSTOM_COMMAND_LAYERS],
}

impl TryFrom<Vec<DefyKeymapLayer>> for DefyKeymap {
    type Error = KeymapDoesNotHave10LayersError;

    fn try_from(layers: Vec<DefyKeymapLayer>) -> Result<Self, Self::Error> {
        let layers = layers
            .try_into()
            .map_err(|_| KeymapDoesNotHave10LayersError)?;

        Ok(Self { layers })
    }
}

impl FromStr for DefyKeymap {
    type Err = ParseKeymapError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let layers: Vec<DefyKeymapLayer> = s
            .parse::<parsing::keymap::Keymap>()?
            .iter()
            .map(DefyKeymapLayer::from)
            .collect();

        Ok(Self::try_from(layers)?)
    }
}

impl serde::Serialize for DefyKeymap {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.layers
            .iter()
            .copied()
            .enumerate()
            .map(|(i, layer)| DefyKeymapLayer {
                layer_number: i as u8 + 1,
                ..layer
            })
            .collect::<Vec<_>>()
            .serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for DefyKeymap {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let layers = <Vec<DefyKeymapLayer> as serde::Deserialize>::deserialize(deserializer)?;

        // Checked at the JSON boundary like `FromStr` checks it at the
        // wire boundary: a config file holding a different number of
        // layers cannot come into existence as a [`DefyKeymap`] either,
        // so every consumer of the type can rely on the write shape.
        Self::try_from(layers).map_err(serde::de::Error::custom)
    }
}

impl DefyKeymap {
    /// Converts this keymap into a form suitable for sending over to the keyboard
    /// as the data of a `keymap.custom` command.
    ///
    /// Infallible by construction: a [`DefyKeymap`] holds exactly the
    /// ten layers the command requires.
    pub fn to_keymap_custom_data(&self) -> String {
        let data = self
            .layers
            .iter()
            .map(|layer| {
                layer
                    .to_keymap_data()
                    .map(|opt| opt.unwrap_or(KeyKind::from(0)))
            })
            .collect::<Vec<_>>();

        let keymap = parsing::keymap::Keymap(data);

        keymap.to_string()
    }

    /// Clears the layer to the provided key.
    pub fn clear_layer_to(&mut self, layer: usize, key: KeyKind) -> Result<(), ClearLayerError> {
        if layer == 0 {
            return Err(ClearLayerError::ZeroIndexProvided);
        }

        let layer = layer - 1;

        if layer >= KEYMAP_CUSTOM_COMMAND_LAYERS {
            return Err(ClearLayerError::LayerDoesNotExist);
        }

        self.layers[layer] = DefyKeymapLayer::new_cleared_to(key);

        Ok(())
    }
}

/// A single human-readable Defy layer.
#[derive(Clone, Copy, Debug, Eq, Serialize, Deserialize)]
pub struct DefyKeymapLayer {
    /// A human-readable label for knowing what layer your editing in the
    /// JSON file.
    ///
    /// **Note**: This number is only a UX aid, and is not taken
    /// into account when deserializing from the config file.
    #[serde(skip_deserializing)]
    pub layer_number: u8,
    /// Left half of the keyboard.
    pub left: DefyKeymapLeft,
    /// Right half of the keyboard.
    pub right: DefyKeymapRight,
}

impl From<&DefyLayerData> for DefyKeymapLayer {
    fn from(layer_data: &DefyLayerData) -> Self {
        Self {
            layer_number: 0,
            left: DefyKeymapLeft::from(layer_data),
            right: DefyKeymapRight::from(layer_data),
        }
    }
}

impl PartialEq for DefyKeymapLayer {
    fn eq(&self, other: &Self) -> bool {
        self.left == other.left && self.right == other.right
    }
}

impl std::hash::Hash for DefyKeymapLayer {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.left.hash(state);
        self.right.hash(state);
    }
}

impl DefyKeymapLayer {
    /// Get's the corresponding key given the key offset.
    ///
    /// The key offset is an unsigned integer in the range `0..80`. Please refer
    /// to the [`LAYOUT`] constant for getting the key offset of a specific key.
    ///
    /// Note that keys are in `0..80`, but the keyboard only has 70 keys, therefore,
    /// some indices will return `None`, even though a keymap will contain a key code.
    /// In these cases, you can use either `u16::MIN` or `u16::MAX`, as it
    /// is only a padded placeholder.
    fn get_key_by_index(&self, index: u8) -> Option<KeyKind> {
        if index >= KEYS_PER_LAYER as u8 {
            return None;
        }

        macro_rules! get_index {
            ($side:ident: {
                $( ( $($path:ident),* ) ),* $(,)?
            }) => {
                None
                  $(
                    .or_else(|| {
                        LAYOUT
                            .$side
                            $(.$path)*
                            .iter()
                            .copied()
                            .position(|key_index| key_index == index)
                            .map(|i| self.$side$(.$path)*[i])
        })
                  )*
            };
        }

        let left = get_index! {
            left: {
                (row_1),
                (row_2),
                (row_3),
                (row_4),
                (thumb_cluster, top),
                (thumb_cluster, bottom),
            }
        };

        let right = get_index! {
            right: {
                (row_1),
                (row_2),
                (row_3),
                (row_4),
                (thumb_cluster, top),
                (thumb_cluster, bottom),
            }
        };

        left.or(right)
    }

    /// Converts this layer into a form suitable for using with keymap commands.
    ///
    /// **Note**: This function returns [`Option<KeyKind>`], rather than [`KeyKind`].
    /// This is done because a keymap layer must contain 80 keys, but the keyboard only
    /// has 70 keys. Therefore, there are 10 missing keys. You should therefore pick
    /// a difault placeholder key, usually `u16::MIN` or `u16::MAX`.
    pub fn to_keymap_data(&self) -> [Option<KeyKind>; KEYS_PER_LAYER] {
        array::from_fn(|i| self.get_key_by_index(i as u8))
    }

    /// Clears the entire layer to the provided key.
    pub fn new_cleared_to(key: KeyKind) -> Self {
        let keys: DefyLayerData = [key; KEYS_PER_LAYER];

        DefyKeymapLayer::from(&keys)
    }
}

/// Left half human-readable Defy keymap.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DefyKeymapLeft {
    /// Row 1.
    pub row_1: [KeyKind; 7],
    /// Row 2.
    pub row_2: [KeyKind; 7],
    /// Row 3.
    pub row_3: [KeyKind; 7],
    /// Row 4.
    pub row_4: [KeyKind; 6],
    /// Thumb cluster.
    pub thumb_cluster: DefyThumbclusterKeymapLeft,
}

impl From<&DefyLayerData> for DefyKeymapLeft {
    fn from(layer_data: &DefyLayerData) -> Self {
        let left_layout = &LAYOUT.left;

        let row_1 = left_layout.row_1.map(|index| layer_data[index as usize]);

        let row_2 = left_layout.row_2.map(|index| layer_data[index as usize]);

        let row_3 = left_layout.row_3.map(|index| layer_data[index as usize]);

        let row_4 = left_layout.row_4.map(|index| layer_data[index as usize]);

        let thumb_cluster = DefyThumbclusterKeymapLeft::from(layer_data);

        Self {
            row_1,
            row_2,
            row_3,
            row_4,
            thumb_cluster,
        }
    }
}

/// Left Defy thumb cluster keymap.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DefyThumbclusterKeymapLeft {
    /// The top four keys of the thumb cluster, from left to right.
    pub top: [KeyKind; 4],
    /// The bottom four keys of the thumb cluster, from left to right.
    pub bottom: [KeyKind; 4],
}

impl From<&DefyLayerData> for DefyThumbclusterKeymapLeft {
    fn from(layer_data: &DefyLayerData) -> Self {
        let left_layout = &LAYOUT.left;

        let top = left_layout
            .thumb_cluster
            .top
            .map(|index| layer_data[index as usize]);

        let bottom = left_layout
            .thumb_cluster
            .bottom
            .map(|index| layer_data[index as usize]);

        Self { top, bottom }
    }
}

/// Right half human-readable Defy keymap.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DefyKeymapRight {
    /// Row 1.
    pub row_1: [KeyKind; 7],
    /// Row 2.
    pub row_2: [KeyKind; 7],
    /// Row 3.
    pub row_3: [KeyKind; 7],
    /// Row 4.
    pub row_4: [KeyKind; 6],
    /// Thumb cluster.
    pub thumb_cluster: DefyThumbclusterKeymapRight,
}

impl From<&DefyLayerData> for DefyKeymapRight {
    fn from(layer_data: &DefyLayerData) -> Self {
        let right_layout = &LAYOUT.right;

        let row_1 = right_layout.row_1.map(|index| layer_data[index as usize]);

        let row_2 = right_layout.row_2.map(|index| layer_data[index as usize]);

        let row_3 = right_layout.row_3.map(|index| layer_data[index as usize]);

        let row_4 = right_layout.row_4.map(|index| layer_data[index as usize]);

        let thumb_cluster = DefyThumbclusterKeymapRight::from(layer_data);

        Self {
            row_1,
            row_2,
            row_3,
            row_4,
            thumb_cluster,
        }
    }
}

/// Left Defy thumb cluster keymap.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DefyThumbclusterKeymapRight {
    /// The top four keys of the thumb cluster, from left to right.
    pub top: [KeyKind; 4],
    /// The bottom four keys of the thumb cluster, from left to right.
    pub bottom: [KeyKind; 4],
}

impl From<&DefyLayerData> for DefyThumbclusterKeymapRight {
    fn from(layer_data: &DefyLayerData) -> Self {
        let right_layout = &LAYOUT.right;

        let top = right_layout
            .thumb_cluster
            .top
            .map(|index| layer_data[index as usize]);

        let bottom = right_layout
            .thumb_cluster
            .bottom
            .map(|index| layer_data[index as usize]);

        Self { top, bottom }
    }
}

/// Holds a superkey map.
///
/// A map holds at most [`SuperkeyMap::MAX_SUPERKEYS`] superkeys, and
/// cannot hold more: the inner vector is private and every way of
/// constructing a map — parsing from wire data, deserializing from
/// JSON, or [`TryFrom<Vec<Superkey>>`](TryFrom) — rejects oversized
/// maps. Converting a [`SuperkeyMap`] to command data (offline or on
/// [`DefyKeyboard::apply_superkeys`]) therefore can never emit a map
/// the firmware could not address, and the 1-based superkey numbering
/// can never overflow.
#[derive(Clone, Debug, Default, Deref, PartialEq, Eq)]
pub struct SuperkeyMap {
    superkeys: Vec<Superkey>,
}

impl SuperkeyMap {
    /// The maximum number of superkeys a [`SuperkeyMap`] can hold.
    ///
    /// The firmware triggers superkeys through a keycode range exactly
    /// 128 codes wide (`DYNAMIC_SUPER_FIRST` to `DYNAMIC_SUPER_LAST` in
    /// the firmware's `Kaleidoscope-Ranges.h`), so superkeys beyond that
    /// count could never be triggered.
    pub const MAX_SUPERKEYS: usize = 128;
}

impl TryFrom<Vec<Superkey>> for SuperkeyMap {
    type Error = SuperkeyMapTooLargeError;

    fn try_from(superkeys: Vec<Superkey>) -> Result<Self, Self::Error> {
        if superkeys.len() > Self::MAX_SUPERKEYS {
            return Err(SuperkeyMapTooLargeError);
        }

        Ok(Self { superkeys })
    }
}

impl serde::Serialize for SuperkeyMap {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.superkeys
            .iter()
            .copied()
            .enumerate()
            .map(|(i, key)| Superkey {
                // Cannot wrap: the type's invariant bounds the map to
                // `MAX_SUPERKEYS` (128), well within `u8`.
                superkey_number: i as u8 + 1,
                ..key
            })
            .collect::<Vec<_>>()
            .serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for SuperkeyMap {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let superkeys = <Vec<Superkey> as serde::Deserialize>::deserialize(deserializer)?;

        // Checked at the JSON boundary like `FromStr` checks it at the
        // wire boundary: an oversized map cannot come into existence
        // from a config file either, so the CLI's offline
        // `superkeys to-command-data` path fails at read time instead of
        // emitting apply-ready command data for a map the firmware
        // could never address.
        Self::try_from(superkeys).map_err(serde::de::Error::custom)
    }
}

impl FromStr for SuperkeyMap {
    type Err = ParseSuperkeyMapError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let wire_map = s.parse::<parsing::superkeys::SuperkeyMap>()?;

        // Checked before the 1-based `u8` numbering below: the wire
        // parse itself imposes no count cap, and numbering a position
        // past `u8` would overflow before [`TryFrom`] could reject the
        // count.
        if wire_map.0.len() > Self::MAX_SUPERKEYS {
            return Err(SuperkeyMapTooLargeError.into());
        }

        let map = Self {
            // Cannot wrap: the check above bounds every position to
            // `MAX_SUPERKEYS` (128), well within `u8`.
            superkeys: wire_map
                .0
                .into_iter()
                .enumerate()
                .map(|(i, key)| Superkey {
                    superkey_number: i as u8 + 1,
                    tap: key.tap,
                    hold: key.hold,
                    tap_hold: key.tap_hold,
                    double_tap: key.double_tap,
                    double_tap_hold: key.double_tap_hold,
                })
                .collect(),
        };

        Ok(map)
    }
}

impl From<&SuperkeyMap> for parsing::superkeys::SuperkeyMap {
    fn from(map: &SuperkeyMap) -> Self {
        Self(map.superkeys.iter().copied().map(Into::into).collect())
    }
}

/// Represents a single superkey.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Superkey {
    /// User-facing ID used to make reading superkey map arrays easier.
    ///
    /// **Note**: This is purely for UX. When deserializing, the ID the user
    /// writes is entirely ignored. The idea is that you define the superkeys
    /// you want, and use this number to know what [`KeyKind`] to use to assign
    /// the particular superkey.
    ///
    /// Numbers are assigned from a superkey's position in the map
    /// (1-based) when parsing or serializing, and a [`SuperkeyMap`] can
    /// hold at most [`SuperkeyMap::MAX_SUPERKEYS`] superkeys, so the
    /// number never wraps.
    #[serde(skip_deserializing)]
    pub superkey_number: u8,
    /// Action performed when the key is tapped.
    pub tap: Option<KeyKind>,
    /// Action performed when the key is held.
    pub hold: Option<KeyKind>,
    /// Action performed when the key is tapped and held.
    pub tap_hold: Option<KeyKind>,
    /// Action performed when the key is double tapped.
    pub double_tap: Option<KeyKind>,
    /// Action performed when the key is double tapped and held.
    pub double_tap_hold: Option<KeyKind>,
}

impl From<Superkey> for parsing::superkeys::Superkey {
    fn from(key: Superkey) -> Self {
        let Superkey {
            superkey_number: _,
            tap,
            hold,
            tap_hold,
            double_tap,
            double_tap_hold,
        } = key;

        Self {
            tap,
            hold,
            tap_hold,
            double_tap,
            double_tap_hold,
        }
    }
}

/// Holds a macro map.
///
/// A map holds at most [`MacroMap::MAX_MACROS`] macros, and cannot hold
/// more: the inner vector is private and every way of constructing a
/// map — parsing from wire data, deserializing from JSON, or
/// [`TryFrom<Vec<Macro>>`](TryFrom) — rejects oversized maps. Converting
/// a [`MacroMap`] to command data (offline or on
/// [`DefyKeyboard::apply_macros`]) therefore can never emit a map the
/// firmware could not address, and the 1-based macro numbering can
/// never overflow.
#[derive(Clone, Debug, Default, Deref, PartialEq, Eq)]
pub struct MacroMap {
    macros: Vec<Macro>,
}

impl MacroMap {
    /// The maximum number of macros a [`MacroMap`] can hold.
    ///
    /// The firmware triggers macros through a keycode range exactly 128
    /// codes wide (`DYNAMIC_MACRO_FIRST` to `DYNAMIC_MACRO_LAST` in the
    /// firmware's `Kaleidoscope-Ranges.h`), so macros beyond that count
    /// could never be triggered.
    pub const MAX_MACROS: usize = 128;
}

impl TryFrom<Vec<Macro>> for MacroMap {
    type Error = MacroMapTooLargeError;

    fn try_from(macros: Vec<Macro>) -> Result<Self, Self::Error> {
        if macros.len() > Self::MAX_MACROS {
            return Err(MacroMapTooLargeError);
        }

        Ok(Self { macros })
    }
}

impl serde::Serialize for MacroMap {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.macros
            .iter()
            .enumerate()
            .map(|(i, m)| Macro {
                // Cannot wrap: the type's invariant bounds the map to
                // `MAX_MACROS` (128), well within `u8`.
                macro_number: i as u8 + 1,
                actions: m.actions.clone(),
            })
            .collect::<Vec<_>>()
            .serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for MacroMap {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let macros = <Vec<Macro> as serde::Deserialize>::deserialize(deserializer)?;

        // Checked at the JSON boundary like `FromStr` checks it at the
        // wire boundary: an oversized map cannot come into existence
        // from a config file either, so the CLI's offline
        // `macros to-command-data` path fails at read time instead of
        // emitting apply-ready command data for a map the firmware
        // could never address.
        Self::try_from(macros).map_err(serde::de::Error::custom)
    }
}

impl FromStr for MacroMap {
    type Err = ParseMacroMapError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let wire_map = s.parse::<parsing::macros::MacroMap>()?;

        // Checked before the 1-based `u8` numbering below: the wire
        // parse itself imposes no count cap, and numbering a position
        // past `u8` would overflow before [`TryFrom`] could reject the
        // count.
        if wire_map.0.len() > Self::MAX_MACROS {
            return Err(MacroMapTooLargeError.into());
        }

        let map = Self {
            // Cannot wrap: the check above bounds every position to
            // `MAX_MACROS` (128), well within `u8`.
            macros: wire_map
                .0
                .into_iter()
                .enumerate()
                .map(|(i, macro_data)| Macro {
                    macro_number: i as u8 + 1,
                    actions: macro_data.actions.into_iter().map(Into::into).collect(),
                })
                .collect(),
        };

        Ok(map)
    }
}

impl From<&MacroMap> for parsing::macros::MacroMap {
    fn from(map: &MacroMap) -> Self {
        Self(
            map.macros
                .iter()
                .map(|m| parsing::macros::Macro {
                    actions: m.actions.iter().copied().map(Into::into).collect(),
                })
                .collect(),
        )
    }
}

/// Represents a single macro.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Macro {
    /// User-facing ID used to make reading the macro map easier.
    ///
    /// **Note**: This is purely for UX. When deserializing, the ID the user
    /// writes is entirely ignored. The idea is that you define the macros
    /// you want, and use this number to know what [`KeyKind`] to use to
    /// assign the particular macro to a key.
    ///
    /// Numbers are assigned from a macro's position in the map (1-based)
    /// when parsing or serializing, and a [`MacroMap`] can hold at most
    /// [`MacroMap::MAX_MACROS`] macros, so the number never wraps.
    #[serde(skip_deserializing)]
    pub macro_number: u8,
    /// The actions this macro will perform.
    pub actions: Vec<MacroAction>,
}

/// The possible actions a macro can perform.
///
/// The firmware has two flavors of key actions: ones carrying a full u16
/// key code, and ones carrying the key code in a single byte, which can
/// only refer to keys without modifiers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MacroAction {
    /// Applies a random delay between the min and max range, in ms.
    Interval {
        /// The minimum delay amount in ms.
        min: u16,
        /// The maximum delay amount in ms.
        max: u16,
    },
    /// Waits for a fixed amount of ms.
    Wait(u16),
    /// Holds down the key. Carries a full u16 key code.
    KeyDown(KeyKind),
    /// Releases the held key. Carries a full u16 key code.
    KeyUp(KeyKind),
    /// Taps the key. Carries a full u16 key code.
    Tap(KeyKind),
    /// Holds down the key. Carries the key code in a single byte, so only
    /// keys without modifiers can be used.
    KeyCodeDown(KeyKind),
    /// Releases the held key. Carries the key code in a single byte, so
    /// only keys without modifiers can be used.
    KeyCodeUp(KeyKind),
    /// Taps the key. Carries the key code in a single byte, so only keys
    /// without modifiers can be used.
    TapCode(KeyKind),
}

impl From<parsing::macros::MacroAction> for MacroAction {
    fn from(action: parsing::macros::MacroAction) -> Self {
        match action {
            parsing::macros::MacroAction::Interval { min, max } => Self::Interval { min, max },
            parsing::macros::MacroAction::Wait(delay) => Self::Wait(delay),
            parsing::macros::MacroAction::KeyDown(key) => Self::KeyDown(key),
            parsing::macros::MacroAction::KeyUp(key) => Self::KeyUp(key),
            parsing::macros::MacroAction::Tap(key) => Self::Tap(key),
            parsing::macros::MacroAction::KeyCodeDown(key) => Self::KeyCodeDown(key),
            parsing::macros::MacroAction::KeyCodeUp(key) => Self::KeyCodeUp(key),
            parsing::macros::MacroAction::TapCode(key) => Self::TapCode(key),
        }
    }
}

impl From<MacroAction> for parsing::macros::MacroAction {
    fn from(action: MacroAction) -> Self {
        match action {
            MacroAction::Interval { min, max } => Self::Interval { min, max },
            MacroAction::Wait(delay) => Self::Wait(delay),
            MacroAction::KeyDown(key) => Self::KeyDown(key),
            MacroAction::KeyUp(key) => Self::KeyUp(key),
            MacroAction::Tap(key) => Self::Tap(key),
            MacroAction::KeyCodeDown(key) => Self::KeyCodeDown(key),
            MacroAction::KeyCodeUp(key) => Self::KeyCodeUp(key),
            MacroAction::TapCode(key) => Self::TapCode(key),
        }
    }
}

/// Full Defy colormap: the palette indices painting every LED, on
/// every layer.
///
/// A Defy colormap holds exactly
/// [`KEYMAP_CUSTOM_COMMAND_LAYERS`] layers — the device stores and
/// reports exactly that many, and every `colormap.map` write carries
/// them — so a [`DefyColormap`] cannot hold a different number of
/// layers by construction. Building one from a count that is not known
/// at compile time (wire data, a JSON file) is fallible: it goes
/// through [`TryFrom<Vec<DefyColormapLayer>>`](TryFrom) and fails with
/// [`ColormapDoesNotHave10LayersError`] when the count is wrong.
///
/// Each layer splits at the hardware's LED boundary: the first
/// [`KEYBOARD_LEDS`] positions paint the keyboard's keys, the
/// remaining [`UNDERGLOW_LEDS`] the underglow.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Deref, DerefMut, From)]
pub struct DefyColormap {
    /// The colormap's ten layers, in wire order (layer 1 first).
    pub layers: [DefyColormapLayer; KEYMAP_CUSTOM_COMMAND_LAYERS],
}

impl TryFrom<Vec<DefyColormapLayer>> for DefyColormap {
    type Error = ColormapDoesNotHave10LayersError;

    fn try_from(layers: Vec<DefyColormapLayer>) -> Result<Self, Self::Error> {
        let layers = layers
            .try_into()
            .map_err(|_| ColormapDoesNotHave10LayersError)?;

        Ok(Self { layers })
    }
}

impl FromStr for DefyColormap {
    type Err = ParseColormapError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let layers: Vec<DefyColormapLayer> = s
            .parse::<parsing::colormap::Colormap>()?
            .iter()
            .map(DefyColormapLayer::from)
            .collect();

        Ok(Self::try_from(layers)?)
    }
}

impl serde::Serialize for DefyColormap {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.layers
            .iter()
            .copied()
            .enumerate()
            .map(|(i, layer)| DefyColormapLayer {
                layer_number: i as u8 + 1,
                ..layer
            })
            .collect::<Vec<_>>()
            .serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for DefyColormap {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let layers = <Vec<DefyColormapLayer> as serde::Deserialize>::deserialize(deserializer)?;

        // Checked at the JSON boundary like `FromStr` checks it at the
        // wire boundary: a config file holding a different number of
        // layers cannot come into existence as a [`DefyColormap`]
        // either, so every consumer of the type can rely on the write
        // shape.
        Self::try_from(layers).map_err(serde::de::Error::custom)
    }
}

impl DefyColormap {
    /// Converts this colormap into a form suitable for sending over to
    /// the keyboard as the data of a `colormap.map` command.
    ///
    /// Infallible by construction: a [`DefyColormap`] holds exactly
    /// the ten layers the command requires.
    pub fn to_colormap_map_data(&self) -> String {
        parsing::colormap::Colormap(
            self.layers
                .iter()
                .map(parsing::colormap::ColormapLayer::from)
                .collect(),
        )
        .to_string()
    }
}

/// A single human-readable Defy colormap layer.
///
/// Serialized as a JSON object of three fields; `layer_number` is a
/// write-only UX aid like the keymap layers' (a file carrying one has
/// it ignored on the way in), and the `keyboard`/`underglow` arrays
/// are length-checked on the way in — exactly 70 and 108 indices.
/// (The serde impls come from the shared `colormap_layer_serde!`
/// macro — serde's array impls stop at 64 elements.)
#[derive(Clone, Copy, Debug, Eq)]
pub struct DefyColormapLayer {
    /// A human-readable label for knowing what layer you're editing in
    /// the JSON file.
    ///
    /// **Note**: This number is only a UX aid, and is not taken
    /// into account when deserializing from the config file.
    pub layer_number: u8,
    /// The keyboard LEDs' palette indices, in wire order (positions
    /// 0–69 of the layer).
    pub keyboard: [parsing::colormap::PaletteIndex; KEYBOARD_LEDS],
    /// The underglow LEDs' palette indices, in wire order (positions
    /// 70–177 of the layer).
    pub underglow: [parsing::colormap::PaletteIndex; UNDERGLOW_LEDS],
}

colormap_layer_serde! {
    DefyColormapLayer,
    keyboard => Keyboard [KEYBOARD_LEDS] "keyboard",
    underglow => Underglow [UNDERGLOW_LEDS] "underglow",
}

impl PartialEq for DefyColormapLayer {
    fn eq(&self, other: &Self) -> bool {
        self.keyboard == other.keyboard && self.underglow == other.underglow
    }
}

impl std::hash::Hash for DefyColormapLayer {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.keyboard.hash(state);
        self.underglow.hash(state);
    }
}

impl From<&parsing::colormap::ColormapLayer> for DefyColormapLayer {
    fn from(layer: &parsing::colormap::ColormapLayer) -> Self {
        let (keyboard, underglow) = layer.0.split_at(KEYBOARD_LEDS);

        Self {
            layer_number: 0,
            keyboard: keyboard
                .try_into()
                .expect("the wire layer carries exactly KEYBOARD_LEDS keyboard positions"),
            underglow: underglow
                .try_into()
                .expect("the wire layer carries exactly UNDERGLOW_LEDS underglow positions"),
        }
    }
}

impl From<&DefyColormapLayer> for parsing::colormap::ColormapLayer {
    fn from(layer: &DefyColormapLayer) -> Self {
        Self(
            layer
                .keyboard
                .into_iter()
                .chain(layer.underglow)
                .collect::<Vec<_>>()
                .try_into()
                .expect("keyboard + underglow are exactly one wire layer's worth of positions"),
        )
    }
}

#[cfg(test)]
mod tests {
    use itertools::Itertools;

    use crate::keycode_tables::Blank;

    use super::*;

    const KEYMAP_DATA: &str = "41 30 31 32 33 34 0 0 0 0 35 36 37 38 39 0 43 20 26 8 21 23 0 0 0 0 28 24 12 18 19 0 57 4 22 7 9 10 17152 0 0 0 11 13 14 15 51 52 53980 29 27 6 25 5 0 0 0 0 17 16 54 55 56 0 53853 17452 44 49467 49209 226 227 0 0 231 76 49209 52028 44 49162 230 41 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 43 85 95 96 97 87 0 0 0 0 75 74 82 77 0 0 0 84 92 93 94 86 83 0 0 0 78 80 81 79 70 0 0 46 89 90 91 99 0 0 0 0 0 0 0 0 0 0 0 0 98 65535 65535 65535 0 0 0 0 0 65535 65535 65535 65535 0 0 58 59 60 61 62 63 65535 65535 64 65 66 67 68 69 0 0 0 0 22710 22709 23785 0 65535 65535 0 0 23663 0 0 65535 0 0 0 22713 22711 22733 23785 0 65535 65535 0 0 23664 20866 20865 0 0 0 0 0 0 0 19682 65535 65535 65535 65535 0 0 0 0 0 0 0 65535 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 65535 65535 0 0 0 0 0 0 0 0 53 2079 2080 2081 2101 0 65535 65535 0 2083 2095 2096 2093 2094 0 0 2078 56 2102 2103 2082 0 65535 65535 0 2084 2086 2087 45 46 0 0 0 0 49 2097 0 65535 65535 65535 65535 0 47 48 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 65535 0 0 0 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 ";
    const SUPERKEY_DATA: &str = "262 281 1 1 1 0 0 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 ";

    /// The full real-shaped keymap data as its 800 raw codes, and the
    /// [`DefyKeymap`] it parses into (the only shape a [`DefyKeymap`]
    /// can have: exactly ten layers).
    fn defy_keymap() -> (Vec<u16>, DefyKeymap) {
        let layer_data = KEYMAP_DATA
            .split(' ')
            .take(KEYMAP_CUSTOM_COMMAND_LAYERS * KEYS_PER_LAYER)
            .map(|s| s.parse().unwrap())
            .collect();

        let keymap = KEYMAP_DATA.parse::<DefyKeymap>().unwrap();

        (layer_data, keymap)
    }

    #[test]
    fn wireless_command_spellings_match_the_wire() {
        // The exact spellings Bazecor sends (`Wireless.tsx`,
        // `BatteryStatus.tsx`) — the highest-risk strings in the
        // wireless unit, pinned here because a typo'd command name
        // would only fail on hardware otherwise.
        assert_eq!(
            DefyKeyboard::WIRELESS_BATTERY_LEFT_LEVEL_COMMAND_NAME,
            "wireless.battery.left.level"
        );
        assert_eq!(
            DefyKeyboard::WIRELESS_BATTERY_RIGHT_LEVEL_COMMAND_NAME,
            "wireless.battery.right.level"
        );
        assert_eq!(
            DefyKeyboard::WIRELESS_BATTERY_LEFT_STATUS_COMMAND_NAME,
            "wireless.battery.left.status"
        );
        assert_eq!(
            DefyKeyboard::WIRELESS_BATTERY_RIGHT_STATUS_COMMAND_NAME,
            "wireless.battery.right.status"
        );
        assert_eq!(
            DefyKeyboard::WIRELESS_BATTERY_SAVING_MODE_COMMAND_NAME,
            "wireless.battery.savingMode"
        );
        assert_eq!(
            DefyKeyboard::WIRELESS_BATTERY_FORCE_READ_COMMAND_NAME,
            "wireless.battery.forceRead"
        );
        assert_eq!(
            DefyKeyboard::WIRELESS_RF_POWER_COMMAND_NAME,
            "wireless.rf.power"
        );
        assert_eq!(
            DefyKeyboard::WIRELESS_RF_CHANNEL_HOP_COMMAND_NAME,
            "wireless.rf.channelHop"
        );
        assert_eq!(
            DefyKeyboard::WIRELESS_RF_SYNC_PAIRING_COMMAND_NAME,
            "wireless.rf.syncPairing"
        );
        assert_eq!(
            DefyKeyboard::WIRELESS_BLUETOOTH_DEVICE_NAME_COMMAND_NAME,
            "wireless.bluetooth.deviceName"
        );
        assert_eq!(
            DefyKeyboard::WIRELESS_BLUETOOTH_INFO_CHANNEL_COMMAND_NAME,
            "wireless.bluetooth.infoChannel"
        );
    }

    #[tokio::test]
    async fn wireless_read_joins_command_spelling_response_and_parse_offline() {
        // The unit's highest-risk join — the command spelled on the
        // wire, the response served for it, the typed value parsed out
        // — driven end-to-end over the scripted double, the same way
        // the capture core is (the production command-name constants
        // feed the same free read helper the methods use).
        let mut connection = ScriptedConnection {
            script: [
                ScriptedConnection::ok("wireless.battery.left.level", "80 "),
                ScriptedConnection::ok("wireless.rf.power", "2"),
            ]
            .into(),
        };

        let level: parsing::wireless::BatteryLevel = read_setting(
            &mut connection,
            DefyKeyboard::WIRELESS_BATTERY_LEFT_LEVEL_COMMAND_NAME,
        )
        .await
        .unwrap();
        assert_eq!(level.get(), 80);

        let power: parsing::wireless::RfPower = read_setting(
            &mut connection,
            DefyKeyboard::WIRELESS_RF_POWER_COMMAND_NAME,
        )
        .await
        .unwrap();
        assert_eq!(power, parsing::wireless::RfPower::High);

        // Both scripted commands were actually issued, in order.
        connection.assert_spent();
    }

    #[test]
    fn keymap_round_trips_from_str() {
        let (mut layer_data, keymap) = defy_keymap();

        let res = keymap
            .layers
            .into_iter()
            .enumerate()
            .flat_map(|(i, layer)| {
                layer
                    .to_keymap_data()
                    .into_iter()
                    .enumerate()
                    .map(|(j, key)| {
                        if key.is_none() {
                            layer_data[i * KEYS_PER_LAYER + j] = 0;
                        }

                        key.unwrap_or(Blank::NoKey.into())
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        assert_eq!(res, layer_data);
    }

    #[test]
    fn keymap_command_data_round_trips() {
        // The real 10-layer capture serializes to its command data
        // infallibly (the type cannot hold a different layer count),
        // the data has the exact write shape, and it parses back to
        // the same keymap.
        let (_layer_data, keymap) = defy_keymap();

        let data = keymap.to_keymap_custom_data();

        assert_eq!(
            data.split(' ').count(),
            KEYMAP_CUSTOM_COMMAND_LAYERS * KEYS_PER_LAYER
        );
        assert_eq!(data.parse::<DefyKeymap>().unwrap(), keymap);
    }

    #[test]
    fn keymap_json_round_trips() {
        // The hand-written `Deserialize` impl keeps accepting exactly
        // the JSON the `Serialize` impl (and a config file) carries,
        // with the UX layer numbers skipped on the way in.
        let (_layer_data, keymap) = defy_keymap();

        let json = serde_json::to_string(&keymap).unwrap();

        for number in 1..=KEYMAP_CUSTOM_COMMAND_LAYERS as u8 {
            assert!(json.contains(&format!(r#""layer_number":{number}"#)));
        }

        let parsed: DefyKeymap = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed, keymap);
    }

    #[test]
    fn keymap_from_layers_of_the_wrong_count_is_unrepresentable() {
        let (_, keymap) = defy_keymap();

        // Nine layers parse as wire data but no such keymap exists: the
        // type rejects the count at construction.
        let nine = DefyKeymap::try_from(keymap.layers[..9].to_vec());

        assert!(matches!(nine, Err(KeymapDoesNotHave10LayersError)));
        assert_eq!(
            nine.unwrap_err().to_string(),
            "keymap does not have exactly 10 layers"
        );

        // So does eleven.
        let mut eleven = keymap.layers.to_vec();
        eleven.push(keymap.layers[0]);

        assert!(DefyKeymap::try_from(eleven).is_err());
    }

    #[test]
    fn keymap_json_of_the_wrong_layer_count_fails_to_deserialize() {
        // Nine whole layers are valid wire data, but a config file
        // holding them cannot become a `DefyKeymap` — the offline
        // `keymap to-command-data` path reads files through this impl,
        // so the write shape is enforced at read time.
        let nine_layer_wire = KEYMAP_DATA.split(' ').take(80 * 9).join(" ");
        let nine_layer_json = {
            let layers: Vec<DefyKeymapLayer> = nine_layer_wire
                .parse::<parsing::keymap::Keymap>()
                .unwrap()
                .iter()
                .map(DefyKeymapLayer::from)
                .collect();

            serde_json::to_string(&layers).unwrap()
        };

        let err = serde_json::from_str::<DefyKeymap>(&nine_layer_json).unwrap_err();

        assert!(err.to_string().contains("exactly 10 layers"));
    }

    #[test]
    fn clear_layer_bounds_are_layer_one_through_ten() {
        let (_, mut keymap) = defy_keymap();

        // Layer 0 is not a layer (layers are 1-based UX numbers).
        assert!(matches!(
            keymap.clear_layer_to(0, Blank::NoKey.into()),
            Err(ClearLayerError::ZeroIndexProvided)
        ));

        // The device has exactly ten layers; nothing beyond exists.
        assert!(matches!(
            keymap.clear_layer_to(11, Blank::NoKey.into()),
            Err(ClearLayerError::LayerDoesNotExist)
        ));

        // Both bounds clear fine.
        keymap.clear_layer_to(1, Blank::NoKey.into()).unwrap();
        keymap.clear_layer_to(10, Blank::NoKey.into()).unwrap();
    }

    #[test]
    fn superkeys_round_trips_from_str() {
        let map = SUPERKEY_DATA.parse::<SuperkeyMap>().unwrap();

        let str_data = parsing::superkeys::SuperkeyMap::from(&map)
            .to_command_data::<512>()
            .unwrap();

        assert_eq!(format!("{str_data} "), SUPERKEY_DATA);
    }

    #[test]
    fn macros_round_trips_from_str() {
        // A real-shaped map exercising both key action flavors: a
        // single-byte key down of `A` and a 300 ms wait, followed by a
        // full u16 key down of the mute key, a full u16 tap of `A`, and a
        // random interval of 100 to 300 ms.
        let data = "6 4 2 1 44 0 3 76 226 5 0 4 1 0 100 1 44 0 0 ";

        let map = data.parse::<MacroMap>().unwrap();

        assert_eq!(map[0].macro_number, 1);
        assert_eq!(map[1].macro_number, 2);

        let str_data = parsing::macros::MacroMap::from(&map)
            .to_command_data(19)
            .unwrap();

        assert_eq!(format!("{str_data} "), data);
    }

    /// The exact `neuron.device` object of the shipped Defy wireless
    /// backup (`bazecor/src/defaultBackups/Defy-wireless-backup.json`),
    /// byte-faithful. The wired backup's descriptor is identical apart
    /// from `"wireless": false`.
    const BAZECOR_SHIPPED_DEFY_DEVICE_JSON: &str = r#"{"info": {"vendor": "Dygma", "product": "Defy", "keyboardType": "wireless", "displayName": "Dygma Defy wireless", "urls": [{"name": "Homepage", "url": "https://www.dygma.com/defy/"}]}, "usb": {"vendorId": 13807, "productId": 18}, "keyboard": {"rows": 5, "columns": 16, "left": [[0, 1, 2, 3, 4, 5, 6], [16, 17, 18, 19, 20, 21, 22], [32, 33, 34, 35, 36, 37, 38], [48, 49, 50, 51, 52, 53], [64, 65, 66, 67, 68, 69, 70, 71]], "right": [[9, 10, 11, 12, 13, 14, 15], [25, 26, 27, 28, 29, 30, 31], [41, 42, 43, 44, 45, 46, 47], [57, 58, 59, 60, 61, 62, 63], [72, 73, 74, 75, 76, 77, 78, 79]], "ledsLeft": [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34], "ledsRight": [35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69]}, "keyboardUnderglow": {"rows": 2, "columns": 89, "ledsLeft": [70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111, 112, 113, 114, 115, 116, 117, 118, 119, 120, 121, 122], "ledsRight": [123, 124, 125, 126, 127, 128, 129, 130, 131, 132, 133, 134, 135, 136, 137, 138, 139, 140, 141, 142, 143, 144, 145, 146, 147, 148, 149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 170, 171, 172, 173, 174, 175]}, "RGBWMode": true, "components": {}, "instructions": {"en": {"updateInstructions": "To update the firmware, the keyboard needs a special reset. When the countdown starts, press and hold the Escape key. Soon after the countdown finished, the Neuron's light should start a blue pulsing pattern, and the flashing will proceed. At this point, you should release the Escape key."}}, "wireless": true, "chipId": "8ed7f33db024a5b5"}"#;

    #[test]
    fn backup_device_descriptor_matches_bazecors_shipped_descriptor() {
        let shipped: serde_json::Value =
            serde_json::from_str(BAZECOR_SHIPPED_DEFY_DEVICE_JSON).unwrap();

        // The wireless variant reproduces the shipped descriptor
        // exactly, per-device fields included.
        let wireless = DefyKeyboard::backup_device_descriptor(true, "8ed7f33db024a5b5");

        assert_eq!(wireless, shipped);

        // The wired variant flips only the `wireless` flag — matching
        // the diff between the two shipped Defy backups.
        let wired = DefyKeyboard::backup_device_descriptor(false, "another-chip-id");

        assert_eq!(wired, {
            let mut wired = shipped.clone();
            wired["wireless"] = false.into();
            wired["chipId"] = "another-chip-id".into();
            wired
        });
    }

    #[test]
    fn macros_json_round_trips() {
        let map = "6 4 0 8 44 0 0 ".parse::<MacroMap>().unwrap();

        let json = serde_json::to_string(&map).unwrap();

        assert!(json.contains(r#""macro_number":1"#));
        assert!(json.contains(r#""macro_number":2"#));

        let parsed: MacroMap = serde_json::from_str(&json).unwrap();

        let actions: Vec<_> = parsed.iter().map(|m| m.actions.clone()).collect();
        let expected: Vec<_> = map.iter().map(|m| m.actions.clone()).collect();

        assert_eq!(actions, expected);
    }

    /// Builds the wire data for a macro map of `count` macros, each a
    /// single-byte key down of `A` (`6 44 0`), followed by the final
    /// map terminator.
    fn macro_map_data(count: usize) -> String {
        let macros: String = std::iter::repeat_n("6 44 0 ", count).collect();

        format!("{macros}0")
    }

    /// Builds the JSON of a macro map holding `count` macros, each a
    /// single `Wait` action, as a hand-written config file would look.
    fn macro_map_json(count: usize) -> String {
        let macros = std::iter::repeat_n(r#"{"actions":[{"Wait":1000}]}"#, count)
            .collect::<Vec<_>>()
            .join(",");

        format!("[{macros}]")
    }

    #[test]
    fn macros_number_up_to_the_firmware_limit_without_wrapping() {
        // 128 one-action macros is the firmware's maximum (its macro
        // keycode range is 128 codes wide): numbering must run 1..=128.
        let map = macro_map_data(MacroMap::MAX_MACROS)
            .parse::<MacroMap>()
            .unwrap();

        assert_eq!(map.len(), MacroMap::MAX_MACROS);
        assert_eq!(map.first().unwrap().macro_number, 1);
        assert_eq!(map.last().unwrap().macro_number, 128);

        let json = serde_json::to_string(&map).unwrap();

        assert!(json.contains(r#""macro_number":128"#));
        assert!(!json.contains(r#""macro_number":129"#));
        assert!(!json.contains(r#""macro_number":0"#));
    }

    #[test]
    fn macros_past_the_firmware_limit_are_rejected_when_parsing() {
        // One macro past the 128-macro addressing space of the firmware:
        // rejected instead of parsed, so the 1-based numbering below can
        // never wrap.
        let res = macro_map_data(MacroMap::MAX_MACROS + 1).parse::<MacroMap>();

        assert!(matches!(
            res,
            Err(ParseMacroMapError::TooManyMacros(MacroMapTooLargeError))
        ));

        assert_eq!(
            res.unwrap_err().to_string(),
            "the macro map holds more than 128 macros — beyond what the firmware can address"
        );
    }

    #[test]
    fn macros_past_the_numbering_range_are_rejected_when_parsing() {
        // 257 macros drives a naive 1-based `u8` numbering past 255: the
        // count check must reject the map BEFORE numbering, returning
        // the typed error rather than overflowing (a debug-build panic
        // at position 256) or wrapping (...254, 255, 0, 1...).
        let res = macro_map_data(257).parse::<MacroMap>();

        assert!(matches!(
            res,
            Err(ParseMacroMapError::TooManyMacros(MacroMapTooLargeError))
        ));
    }

    #[test]
    fn macros_past_the_firmware_limit_are_unrepresentable() {
        // 257 macros is well past both the firmware's 128-macro
        // addressing space and the old `u8` numbering wrap at position
        // 256 (numbers went ...254, 255, 0, 1...). No such map can come
        // into existence: the count is rejected at construction, so a
        // serialization of one (and the numbering overflow it used to
        // guard against) is not even expressible.
        let macros: Vec<Macro> = std::iter::repeat_n(
            Macro {
                macro_number: 0,
                actions: vec![MacroAction::Wait(1000)],
            },
            257,
        )
        .collect();

        assert!(matches!(
            MacroMap::try_from(macros),
            Err(MacroMapTooLargeError)
        ));

        // The boundary itself, one past the limit, is rejected too.
        let macros_129: Vec<Macro> = std::iter::repeat_n(
            Macro {
                macro_number: 0,
                actions: vec![MacroAction::Wait(1000)],
            },
            MacroMap::MAX_MACROS + 1,
        )
        .collect();

        assert!(matches!(
            MacroMap::try_from(macros_129),
            Err(MacroMapTooLargeError)
        ));
    }

    #[test]
    fn macros_past_the_firmware_limit_are_rejected_when_deserializing_json() {
        // One macro past the firmware's 128-macro addressing space: a
        // hand-written or Bazecor-imported config file like this must
        // fail at deserialization — the offline
        // `macros to-command-data` path reads files through this impl,
        // so it can never emit apply-ready command data for a map the
        // firmware could never address.
        let err = serde_json::from_str::<MacroMap>(&macro_map_json(MacroMap::MAX_MACROS + 1))
            .unwrap_err();

        assert!(err.to_string().contains("more than 128 macros"));
    }

    #[test]
    fn macros_at_the_firmware_limit_deserialize_from_json() {
        // Exactly 128 macros is the firmware's maximum and must keep
        // deserializing: the guard rejects only oversized maps.
        let map = serde_json::from_str::<MacroMap>(&macro_map_json(MacroMap::MAX_MACROS)).unwrap();

        assert_eq!(map.len(), MacroMap::MAX_MACROS);
        assert_eq!(map.first().unwrap().macro_number, 0);
        assert_eq!(map.last().unwrap().actions, vec![MacroAction::Wait(1000)]);

        // The accepted map round-trips back out through serialization.
        assert_eq!(
            serde_json::from_str::<MacroMap>(&serde_json::to_string(&map).unwrap()).unwrap(),
            map
        );
    }

    /// Builds the wire data for a superkey map of `count` superkeys,
    /// each with no actions (`1 1 1 1 1 0` — five empty action slots
    /// and the superkey terminator), followed by the final map
    /// terminator.
    fn superkey_map_data(count: usize) -> String {
        let superkeys: String = std::iter::repeat_n("1 1 1 1 1 0 ", count).collect();

        format!("{superkeys}0")
    }

    /// Builds the JSON of a superkey map holding `count` superkeys,
    /// each with no actions, as a hand-written config file would look.
    fn superkey_map_json(count: usize) -> String {
        let superkeys = std::iter::repeat_n(
            r#"{"tap":null,"hold":null,"tap_hold":null,"double_tap":null,"double_tap_hold":null}"#,
            count,
        )
        .collect::<Vec<_>>()
        .join(",");

        format!("[{superkeys}]")
    }

    #[test]
    fn superkeys_number_up_to_the_firmware_limit_without_wrapping() {
        // 128 superkeys is the firmware's maximum (its superkey keycode
        // range is 128 codes wide): numbering must run 1..=128.
        let map = superkey_map_data(SuperkeyMap::MAX_SUPERKEYS)
            .parse::<SuperkeyMap>()
            .unwrap();

        assert_eq!(map.len(), SuperkeyMap::MAX_SUPERKEYS);
        assert_eq!(map.first().unwrap().superkey_number, 1);
        assert_eq!(map.last().unwrap().superkey_number, 128);

        let json = serde_json::to_string(&map).unwrap();

        assert!(json.contains(r#""superkey_number":128"#));
        assert!(!json.contains(r#""superkey_number":129"#));
        assert!(!json.contains(r#""superkey_number":0"#));
    }

    #[test]
    fn superkeys_past_the_firmware_limit_are_rejected_when_parsing() {
        // One superkey past the 128-superkey addressing space of the
        // firmware: rejected instead of parsed, so the 1-based numbering
        // below can never wrap.
        let res = superkey_map_data(SuperkeyMap::MAX_SUPERKEYS + 1).parse::<SuperkeyMap>();

        assert!(matches!(
            res,
            Err(ParseSuperkeyMapError::TooManySuperkeys(
                SuperkeyMapTooLargeError
            ))
        ));

        assert_eq!(
            res.unwrap_err().to_string(),
            "the superkey map holds more than 128 superkeys — beyond what the firmware can address"
        );
    }

    #[test]
    fn superkeys_past_the_firmware_limit_are_unrepresentable() {
        // 257 superkeys is well past both the firmware's 128-superkey
        // addressing space and the `u8` numbering overflow at position
        // 256. No such map can come into existence: the count is
        // rejected at construction, so a serialization of one (and the
        // numbering overflow it used to guard against) is not even
        // expressible.
        let superkeys: Vec<Superkey> = std::iter::repeat_n(
            Superkey {
                superkey_number: 0,
                tap: None,
                hold: None,
                tap_hold: None,
                double_tap: None,
                double_tap_hold: None,
            },
            257,
        )
        .collect();

        assert!(matches!(
            SuperkeyMap::try_from(superkeys),
            Err(SuperkeyMapTooLargeError)
        ));

        // The boundary itself, one past the limit, is rejected too.
        let superkeys_129: Vec<Superkey> = std::iter::repeat_n(
            Superkey {
                superkey_number: 0,
                tap: None,
                hold: None,
                tap_hold: None,
                double_tap: None,
                double_tap_hold: None,
            },
            SuperkeyMap::MAX_SUPERKEYS + 1,
        )
        .collect();

        assert!(matches!(
            SuperkeyMap::try_from(superkeys_129),
            Err(SuperkeyMapTooLargeError)
        ));
    }

    #[test]
    fn superkeys_past_the_firmware_limit_are_rejected_when_deserializing_json() {
        // One superkey past the firmware's 128-superkey addressing
        // space: a hand-written or Bazecor-imported config file like
        // this must fail at deserialization — the offline
        // `superkeys to-command-data` path reads files through this
        // impl, so it can never emit apply-ready command data for a map
        // the firmware could never address.
        let err =
            serde_json::from_str::<SuperkeyMap>(&superkey_map_json(SuperkeyMap::MAX_SUPERKEYS + 1))
                .unwrap_err();

        assert!(err.to_string().contains("more than 128 superkeys"));
    }

    #[test]
    fn superkeys_at_the_firmware_limit_deserialize_from_json() {
        // Exactly 128 superkeys is the firmware's maximum and must keep
        // deserializing: the guard rejects only oversized maps.
        let map =
            serde_json::from_str::<SuperkeyMap>(&superkey_map_json(SuperkeyMap::MAX_SUPERKEYS))
                .unwrap();

        assert_eq!(map.len(), SuperkeyMap::MAX_SUPERKEYS);
        assert_eq!(map.first().unwrap().superkey_number, 0);
        assert_eq!(map.last().unwrap().tap, None);

        // Numbering runs to exactly the firmware's limit when the
        // accepted map is serialized back out.
        let json = serde_json::to_string(&map).unwrap();

        assert!(json.contains(r#""superkey_number":128"#));
        assert!(!json.contains(r#""superkey_number":129"#));

        // The accepted map round-trips back out through serialization.
        assert_eq!(serde_json::from_str::<SuperkeyMap>(&json).unwrap(), map);
    }
    // === Restore planning (phase 2): the real shipped Defy wireless
    // === backup's data, byte for byte.

    /// the real `keymap.custom` data of the shipped Defy wireless backup: 10 full layers of 80 keys each.
    const REAL_KEYMAP_CUSTOM_DATA: &str = "41 30 31 32 33 34 45 0 0 46 35 36 37 38 39 42 43 20 26 8 21 23 47 0 0 48 28 24 12 18 19 76 57 4 22 7 9 10 17152 0 0 49 11 13 14 15 51 52 225 29 27 6 25 5 0 0 0 0 17 16 54 55 56 229 53980 17452 44 49211 49721 227 0 0 54109 54108 101 49209 49162 44 230 53852 41 0 0 0 0 0 0 0 0 0 83 0 84 85 46 42 43 0 74 82 77 75 0 0 0 0 0 95 96 97 86 0 0 70 80 81 79 78 0 0 0 0 0 92 93 94 87 0 225 0 0 0 0 0 0 0 0 0 99 89 90 91 88 0 0 0 65535 65535 65535 65535 0 0 0 0 0 0 65535 98 0 0 53 58 59 60 61 62 63 65535 65535 64 65 66 67 68 69 0 0 0 0 22710 22709 23785 0 65535 65535 0 0 23663 0 0 65535 0 0 0 22713 22711 22733 23786 0 65535 65535 0 0 23664 20866 20865 0 0 0 0 0 0 0 19682 65535 65535 65535 65535 0 0 0 0 0 0 0 65535 0 0 0 0 0 0 0 0 0 0 0 0 0 0 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535";

    /// the real `palette` data of the shipped Defy wireless backup (16 RGBW colors).
    const REAL_PALETTE_DATA: &str = "255 196 0 0 0 254 24 0 0 0 0 255 231 255 0 0 0 254 234 0 0 52 255 0 255 0 232 0 0 77 168 87 125 0 235 19 20 0 36 219 85 0 126 129 255 9 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0";

    /// the real `colormap.map` data of the shipped Defy wireless backup: 10 layers of 178 palette indices.
    const REAL_COLORMAP_DATA: &str = "10 4 4 4 4 4 7 10 4 4 4 4 4 7 2 4 4 4 4 4 0 2 4 4 4 4 4 10 3 1 5 8 2 0 0 5 4 4 4 4 4 7 11 4 4 4 4 4 7 7 7 4 4 4 4 7 2 7 7 7 4 4 10 2 1 3 8 2 6 6 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 10 12 12 12 12 12 12 10 12 9 3 9 10 12 12 1 3 3 3 10 12 2 12 12 12 12 12 12 12 1 6 5 2 12 12 5 0 0 0 12 11 12 12 0 3 3 3 12 12 12 0 3 0 3 12 12 12 6 3 3 3 3 12 12 9 3 12 12 12 12 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 7 4 8 8 8 8 8 8 15 12 15 7 7 9 15 15 12 3 0 1 9 15 15 15 15 15 15 9 15 8 15 12 15 15 15 15 15 8 8 8 8 8 8 15 15 15 12 3 15 15 15 12 11 0 3 12 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 3 6 6 6 6 6 3 3 6 6 6 6 6 3 3 6 6 6 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15";

    /// the real `superkeys.map` data of the shipped Defy wireless backup: one superkey, live bytes only.
    const REAL_SUPERKEYS_DATA: &str = "262 281 2329 283 1 0 0";

    /// the real `macros.map` data of the shipped Defy wireless backup: the Dygmate macro.
    const REAL_MACROS_DATA: &str =
        "6 229 8 11 7 229 8 8 8 28 8 54 8 44 6 229 8 7 7 229 8 28 8 10 8 16 8 4 8 23 8 8 0 0";

    /// The real-shaped macro map data with one macro too many for the
    /// firmware's 128-macro addressing space (`MacroMap::MAX_MACROS`):
    /// 129 single-keycode-down macros plus the map terminator.
    fn oversized_macros_data() -> String {
        std::iter::repeat_n("6 229 0", MacroMap::MAX_MACROS + 1)
            .chain(std::iter::once("0"))
            .join(" ")
    }

    /// A backup holding a representative slice of the shipped Defy
    /// wireless backup's capture, in its real order: `keymap.custom`
    /// first, `superkeys.map` later — the order Bazecor's reorder
    /// exists for. All map data is the real, full-shaped data.
    fn real_ordered_backup() -> parsing::backup::DefyBackup {
        let entries = [
            ("keymap.custom", REAL_KEYMAP_CUSTOM_DATA),
            ("keymap.onlyCustom", "1"),
            ("settings.defaultLayer", "0"),
            ("led.brightness", "153"),
            ("palette", REAL_PALETTE_DATA),
            ("colormap.map", REAL_COLORMAP_DATA),
            ("idleleds.time_limit", "300"),
            ("qukeys.holdTimeout", "250"),
            ("qukeys.overlapThreshold", "80"),
            ("superkeys.map", REAL_SUPERKEYS_DATA),
            ("superkeys.waitfor", "500"),
            ("macros.map", REAL_MACROS_DATA),
            ("mouse.speedLimit", "127"),
            ("wireless.bluetooth.macs", ""),
            ("wireless.rf.power", "0"),
        ];

        parsing::backup::DefyBackup {
            neuron_id: "8ed7f33db024a5b5".to_owned(),
            neuron: parsing::backup::BackupNeuron {
                id: "8ed7f33db024a5b5".to_owned(),
                device: Some(DefyKeyboard::backup_device_descriptor(
                    true,
                    "8ed7f33db024a5b5",
                )),
                ..Default::default()
            },
            versions: Default::default(),
            backup: entries
                .iter()
                .map(|(command, data)| {
                    parsing::backup::BackupCommand::new((*command).to_owned(), (*data).to_owned())
                })
                .collect(),
            partial: None,
        }
    }

    #[test]
    fn real_backup_yields_bazecors_full_restore_plan() {
        let plan = DefyKeyboard::backup_restore_plan(&real_ordered_backup()).unwrap();

        let commands: Vec<&str> = plan.iter().map(|entry| entry.command.as_str()).collect();

        // Bazecor's restore order: the superkeys map moved directly
        // before the keymap, everything else in its captured order, and
        // the `led.mode 0` tail appended (Bazecor's
        // `noCacheCommand("led.mode 0")`; `led.mode` is deny-listed
        // from captures, so the tail is always an append). The plan is
        // the complete device write sequence, tail included.
        assert_eq!(
            commands,
            [
                "superkeys.map",
                "keymap.custom",
                "keymap.onlyCustom",
                "settings.defaultLayer",
                "led.brightness",
                "palette",
                "colormap.map",
                "idleleds.time_limit",
                "qukeys.holdTimeout",
                "qukeys.overlapThreshold",
                "superkeys.waitfor",
                "macros.map",
                "mouse.speedLimit",
                "wireless.bluetooth.macs",
                "wireless.rf.power",
                "led.mode",
            ]
        );

        assert_eq!(plan.last().unwrap().data, "0");

        // Validation parsed the full real data and the plan carries it
        // through verbatim — restores re-send captures byte for byte.
        let keymap = plan
            .iter()
            .find(|entry| entry.command == "keymap.custom")
            .unwrap();

        assert_eq!(keymap.data, REAL_KEYMAP_CUSTOM_DATA);
    }

    #[test]
    fn plan_rejects_a_backup_for_another_product() {
        let mut backup = real_ordered_backup();

        backup.neuron.device = Some(serde_json::json!({ "info": { "product": "Raise" } }));

        let err = DefyKeyboard::backup_restore_plan(&backup).unwrap_err();

        assert_eq!(err.to_string(), "the backup is for a `Raise`, not a Defy");
        assert!(matches!(err, BackupValidationError::NotADefyBackup { .. }));
    }

    #[test]
    fn plan_allows_a_backup_without_a_device_descriptor() {
        // Nothing to check the product against: the entries carry the
        // proof (a foreign keymap or map data fails their parse).
        let mut backup = real_ordered_backup();

        backup.neuron.device = None;

        assert!(DefyKeyboard::backup_restore_plan(&backup).is_ok());
    }

    #[test]
    fn plan_rejects_truncated_keymap_data() {
        // The real keymap data with the tenth layer cut off mid-layer:
        // half a restore is worse than none, so validation fails the
        // whole backup with the parse failure.
        let mut backup = real_ordered_backup();

        backup.backup[0].data = REAL_KEYMAP_CUSTOM_DATA
            .split(' ')
            .take(80 * 9 + 40)
            .join(" ");

        let err = DefyKeyboard::backup_restore_plan(&backup).unwrap_err();

        assert!(matches!(
            err,
            BackupValidationError::InvalidEntry {
                ref command,
                source: BackupEntryError::Keymap(_),
            } if command == "keymap.custom"
        ));
    }

    #[test]
    fn plan_rejects_a_keymap_with_the_wrong_layer_count() {
        // Nine *whole* layers parse as a keymap (the wire parser
        // accepts any layer count), but a `DefyKeymap` cannot be built
        // from anything but the exactly ten layers every
        // `keymap.custom` write carries — the type's construction is
        // the whole validation.
        let mut backup = real_ordered_backup();

        backup.backup[0].data = REAL_KEYMAP_CUSTOM_DATA.split(' ').take(80 * 9).join(" ");

        let err = DefyKeyboard::backup_restore_plan(&backup).unwrap_err();

        assert!(matches!(
            err,
            BackupValidationError::InvalidEntry {
                ref command,
                source: BackupEntryError::Keymap(
                    ParseKeymapError::IncorrectLayerCount(_)
                ),
            } if command == "keymap.custom"
        ));
        assert!(err.to_string().contains("keymap.custom"));
        assert!(err.to_string().contains("exactly 10 layers"));
    }

    #[test]
    fn plan_rejects_a_macro_map_past_the_firmware_limit() {
        // One macro past the firmware's 128-macro addressing space:
        // validation parses the map through `MacroMap`, whose limit
        // (issue #17) rejects it before anything is sent.
        let mut backup = real_ordered_backup();

        backup
            .backup
            .iter_mut()
            .find(|entry| entry.command == "macros.map")
            .unwrap()
            .data = oversized_macros_data();

        let err = DefyKeyboard::backup_restore_plan(&backup).unwrap_err();

        assert!(matches!(
            err,
            BackupValidationError::InvalidEntry {
                ref command,
                source: BackupEntryError::Macros(_),
            } if command == "macros.map"
        ));
    }

    #[test]
    fn plan_carries_the_real_palette_and_colormap_through_verbatim() {
        // The real shipped palette and colormap data validate through
        // the WS-C wire types (exactly 16 RGBW colors; exactly ten
        // layers of 178 indices) and the plan sends them verbatim, the
        // way Bazecor's restore sends captures.
        let plan = DefyKeyboard::backup_restore_plan(&real_ordered_backup()).unwrap();

        let colormap = plan
            .iter()
            .find(|entry| entry.command == "colormap.map")
            .unwrap();
        let palette = plan
            .iter()
            .find(|entry| entry.command == "palette")
            .unwrap();

        assert_eq!(colormap.data, REAL_COLORMAP_DATA);
        assert_eq!(palette.data, REAL_PALETTE_DATA);
    }

    #[test]
    fn plan_rejects_corrupt_palette_data() {
        // Three colors short of a palette: not a Defy state, refused
        // before anything is sent.
        let mut backup = real_ordered_backup();

        backup
            .backup
            .iter_mut()
            .find(|entry| entry.command == "palette")
            .unwrap()
            .data = REAL_PALETTE_DATA.split(' ').take(60).join(" ");

        let err = DefyKeyboard::backup_restore_plan(&backup).unwrap_err();

        assert!(matches!(
            err,
            BackupValidationError::InvalidEntry {
                ref command,
                source: BackupEntryError::Palette(_),
            } if command == "palette"
        ));
        assert!(err.to_string().contains("palette"));
        assert!(err.to_string().contains("60"));
    }

    #[test]
    fn plan_rejects_a_colormap_with_the_wrong_layer_count() {
        // Nine *whole* layers parse as wire data (the wire parser
        // accepts any layer count), but a `DefyColormap` cannot be
        // built from anything but the exactly ten layers every
        // `colormap.map` write carries.
        let mut backup = real_ordered_backup();

        backup
            .backup
            .iter_mut()
            .find(|entry| entry.command == "colormap.map")
            .unwrap()
            .data = REAL_COLORMAP_DATA.split(' ').take(178 * 9).join(" ");

        let err = DefyKeyboard::backup_restore_plan(&backup).unwrap_err();

        assert!(matches!(
            err,
            BackupValidationError::InvalidEntry {
                ref command,
                source: BackupEntryError::Colormap(
                    ParseColormapError::IncorrectLayerCount(_)
                ),
            } if command == "colormap.map"
        ));
        assert!(err.to_string().contains("colormap.map"));
        assert!(err.to_string().contains("exactly 10 layers"));
    }

    #[test]
    fn plan_rejects_colormap_data_with_an_out_of_domain_index() {
        // A `16` is a well-formed number but not a storable palette
        // index (the firmware packs indices two per EEPROM byte); the
        // parse catches it, so no restore can corrupt a neighboring
        // LED's color.
        let mut backup = real_ordered_backup();

        backup
            .backup
            .iter_mut()
            .find(|entry| entry.command == "colormap.map")
            .unwrap()
            .data = REAL_COLORMAP_DATA.replacen("10 4", "10 16", 1);

        let err = DefyKeyboard::backup_restore_plan(&backup).unwrap_err();

        assert!(matches!(
            err,
            BackupValidationError::InvalidEntry {
                ref command,
                source: BackupEntryError::Colormap(_),
            } if command == "colormap.map"
        ));
        assert!(err.to_string().contains("palette index (0-15)"));
    }

    /// A full [`DefyColormap`] built from the real shipped colormap
    /// data — the only shape a `DefyColormap` can have.
    fn real_colormap() -> DefyColormap {
        REAL_COLORMAP_DATA.parse().unwrap()
    }

    #[test]
    fn colormap_round_trips_from_str() {
        let colormap = real_colormap();

        let data = colormap.to_colormap_map_data();

        assert_eq!(data.split(' ').count(), 10 * 178);
        assert_eq!(data.parse::<DefyColormap>().unwrap(), colormap);
    }

    #[test]
    fn colormap_from_layers_of_the_wrong_count_is_unrepresentable() {
        let colormap = real_colormap();

        // Nine layers parse as wire data but no such colormap exists:
        // the type rejects the count at construction.
        let nine = DefyColormap::try_from(colormap.layers[..9].to_vec());

        assert!(matches!(nine, Err(ColormapDoesNotHave10LayersError)));
        assert_eq!(
            nine.unwrap_err().to_string(),
            "colormap does not have exactly 10 layers"
        );

        // So does eleven.
        let mut eleven = colormap.layers.to_vec();
        eleven.push(colormap.layers[0]);

        assert!(DefyColormap::try_from(eleven).is_err());
    }

    #[test]
    fn colormap_json_round_trips_with_layer_numbers() {
        // The hand-written `Deserialize` impl keeps accepting exactly
        // the JSON the `Serialize` impl (and a config file) carries,
        // with the UX layer numbers skipped on the way in, and the
        // keyboard/underglow split pinned to the hardware's LED
        // boundary.
        let colormap = real_colormap();

        let json = serde_json::to_string(&colormap).unwrap();

        for number in 1..=KEYMAP_CUSTOM_COMMAND_LAYERS as u8 {
            assert!(json.contains(&format!(r#""layer_number":{number}"#)));
        }

        // Layer 1's keyboard array is the wire's first 70 indices; its
        // underglow the remaining 108.
        assert!(json.contains(&format!(
                r#""keyboard":[{}],"#,
                REAL_COLORMAP_DATA
                    .split(' ')
                    .take(70)
                    .collect::<Vec<_>>()
                    .join(",")
            )));

        let parsed: DefyColormap = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed, colormap);

        // A file carrying the wrong keyboard-array length fails at the
        // JSON boundary, with the counts named.
        let wrong = json.replacen(
            &format!(
                r#""keyboard":[{}]"#,
                REAL_COLORMAP_DATA
                    .split(' ')
                    .take(70)
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            "\"keyboard\":[0]",
            1,
        );

        let err = serde_json::from_str::<DefyColormap>(&wrong).unwrap_err();

        // serde's exact shape: the FOUND length first, then the
        // expected one.
        assert!(
            err.to_string()
                .contains("invalid length 1, expected a `keyboard` array of exactly 70 indices")
        );

        // Derived-struct serde tolerance, hand-written faithfully: an
        // unknown field is ignored wholesale (the derived default is
        // NOT `deny_unknown_fields`).
        let decorated = json.replacen(
            r#"{"layer_number":1,"#,
            r#"{"note":"ignored","layer_number":1,"#,
            1,
        );

        let parsed: DefyColormap = serde_json::from_str(&decorated).unwrap();

        assert_eq!(parsed, colormap);

        // And a file omitting the write-only `layer_number`s entirely
        // reads back the same colormap: they are restored from layer
        // position on serialization.
        let stripped = (1..=KEYMAP_CUSTOM_COMMAND_LAYERS as u8)
            .fold(json.clone(), |json, number| {
                json.replace(&format!(r#""layer_number":{number},"#,), "")
            });

        let parsed: DefyColormap = serde_json::from_str(&stripped).unwrap();

        assert_eq!(parsed, colormap);
    }

    #[test]
    fn colormap_json_rejects_duplicate_fields_like_the_derive() {
        // serde-derive parity for the hand-written visitor: a repeated
        // `keyboard`/`underglow` field is a `duplicate field` error, not
        // a silent last-one-wins overwrite. (The error fires on
        // re-sighting the KEY, so the duplicate value is never parsed.)
        let layer_json = serde_json::to_string(&real_colormap().layers[0]).unwrap();

        let duplicated_keyboard =
            layer_json.replacen(r#""keyboard":["#, r#""keyboard":[0],"keyboard":["#, 1);

        let err = serde_json::from_str::<DefyColormapLayer>(&duplicated_keyboard).unwrap_err();

        assert!(err.to_string().contains("duplicate field `keyboard`"));

        let duplicated_underglow =
            layer_json.replacen(r#""underglow":["#, r#""underglow":[0],"underglow":["#, 1);

        let err = serde_json::from_str::<DefyColormapLayer>(&duplicated_underglow).unwrap_err();

        assert!(err.to_string().contains("duplicate field `underglow`"));

        // A repeated write-only `layer_number` stays tolerated, the way
        // the derived skip-deserializing fields are: it is discarded
        // either time.
        let duplicated_number = layer_json.replacen(
            r#"{"layer_number":1,"#,
            r#"{"layer_number":9,"layer_number":1,"#,
            1,
        );

        let parsed = serde_json::from_str::<DefyColormapLayer>(&duplicated_number).unwrap();

        assert_eq!(parsed, real_colormap().layers[0]);
    }

    // =================================================================================
    // Partial-capture recovery: `create_backup`/`complete_backup`
    // against a scripted connection (no hardware), and the restore
    // plan's partial refusal.
    // =================================================================================

    /// A scripted Focus API connection: each `run_command` checks the
    /// command against the next script entry and hands back its
    /// response. The capture's command sequence is deterministic
    /// (`help`, then the uncaptured capture-list commands, then the
    /// identity probes), so a script is the whole conversation.
    ///
    /// The capture core ([`capture_backup`]) is generic over
    /// [`FocusApiConnection`], so the tests drive it through this
    /// double — no hardware, no transports.
    #[derive(Debug)]
    struct ScriptedConnection {
        script: std::collections::VecDeque<(&'static str, Result<String, RunCommandError>)>,
    }

    impl ScriptedConnection {
        /// A `help` response whose capture list is exactly three
        /// commands (`led.brightnessUG` is force-appended by the
        /// filter, so a full capture runs four reads) and which lists
        /// all three identity probes — the probes never join the
        /// capture list (the deny list filters `hardware`/`version`
        /// out), but the capture consults this list to decide which
        /// probes to send.
        fn help() -> String {
            Self::help_serving(&["version", "hardware.chip_id", "hardware.wireless"])
        }

        /// The scripted `help` response with only `served_probes`
        /// listed after the three capture commands: the scenario's
        /// knob for which identity probes the device serves (and the
        /// capture may send).
        fn help_serving(served_probes: &[&str]) -> String {
            ["keymap.custom", "palette", "mouse.speed"]
                .into_iter()
                .chain(served_probes.iter().copied())
                .collect::<Vec<_>>()
                .join("\n")
                + "\n"
        }

        fn ok(
            command: &'static str,
            response: &str,
        ) -> (&'static str, Result<String, RunCommandError>) {
            (command, Ok(response.to_owned()))
        }

        fn failed(command: &'static str) -> (&'static str, Result<String, RunCommandError>) {
            (
                command,
                Err(RunCommandError::ResponseStreamTerminatedPrematurely),
            )
        }

        /// The full successful capture script: `help`, the four
        /// capture reads, then the three identity probes.
        fn full_capture() -> Self {
            Self {
                script: [
                    Self::ok("help", &Self::help()),
                    Self::ok("keymap.custom", "41 30 31"),
                    Self::ok("palette", "255 196 0 0"),
                    Self::ok("mouse.speed", "20"),
                    Self::ok("led.brightnessUG", "255"),
                    Self::ok("hardware.chip_id", "8ed7f33db024a5b5"),
                    Self::ok("version", "v2.2.0 7622bb53 c9d9b7b-dirty"),
                    Self::ok("hardware.wireless", "true"),
                ]
                .into(),
            }
        }

        /// The proof a test's script covered exactly the conversation
        /// the capture ran: once the capture has returned, every entry
        /// must be consumed — a leftover means the capture under-ran
        /// (a scripted command was never issued), and the test must
        /// not pass over that.
        fn assert_spent(&self) {
            assert!(
                self.script.is_empty(),
                "the script was not fully consumed, {:?} left",
                self.script
            );
        }
    }

    impl FocusApiConnection for ScriptedConnection {
        async fn run_command(
            &mut self,
            command: &str,
            _data: Option<&str>,
        ) -> Result<String, RunCommandError> {
            let (expected, response) = self
                .script
                .pop_front()
                .expect("the script must cover every command the capture runs");

            assert_eq!(command, expected, "the capture ran an unscripted command");

            response
        }
    }

    /// A partial-capture backup holding `captured` of the scripted
    /// three-command capture list, as an interrupted capture leaves
    /// one.
    fn partial_of(captured: &[&str]) -> parsing::backup::DefyBackup {
        let expected: Vec<String> = [
            "keymap.custom",
            "palette",
            "mouse.speed",
            "led.brightnessUG",
        ]
        .iter()
        .map(|command| (*command).to_owned())
        .collect();

        let entries: Vec<parsing::backup::BackupCommand> = captured
            .iter()
            .map(|command| {
                parsing::backup::BackupCommand::new((*command).to_owned(), String::new())
            })
            .collect();

        parsing::backup::DefyBackup::partial_capture(entries, &expected)
    }

    #[tokio::test]
    async fn create_backup_captures_the_whole_list_and_carries_no_marker() {
        let mut connection = ScriptedConnection::full_capture();

        let (backup, identity) = capture_backup(&mut connection, &[]).await.unwrap();

        // The entries in `help` order, the force-add last.
        let commands: Vec<&str> = backup.backup.iter().map(|e| e.command.as_str()).collect();

        assert_eq!(
            commands,
            [
                "keymap.custom",
                "palette",
                "mouse.speed",
                "led.brightnessUG"
            ]
        );

        // The probes filled the identity fields, and no partial marker
        // exists: a complete capture, byte-shaped like Bazecor's.
        assert_eq!(backup.neuron_id, "8ed7f33db024a5b5");
        assert_eq!(backup.neuron.id, "8ed7f33db024a5b5");
        assert_eq!(
            backup.neuron.device.as_ref().unwrap()["info"]["product"],
            "Defy"
        );
        assert_eq!(backup.versions.bazecor.as_deref(), Some("v2.2.0"));
        assert!(!backup.is_partial());
        assert!(backup.partial.is_none());

        // The report agrees with the file: every probe obtained, so
        // the CLI's note would stay silent.
        assert_eq!(
            identity,
            CaptureIdentity {
                chip_id: Ok("8ed7f33db024a5b5".to_owned()),
                version: Ok("v2.2.0 7622bb53 c9d9b7b-dirty".to_owned()),
                wireless: Ok(true),
            }
        );

        connection.assert_spent();
    }

    #[tokio::test]
    async fn create_backup_preserves_a_partial_when_a_command_fails() {
        // The `mouse.speed` read fails: everything before it is kept,
        // everything from it on is the missing list.
        let mut connection = ScriptedConnection {
            script: [
                ScriptedConnection::ok("help", &ScriptedConnection::help()),
                ScriptedConnection::ok("keymap.custom", "41 30 31"),
                ScriptedConnection::ok("palette", "255 196 0 0"),
                ScriptedConnection::failed("mouse.speed"),
            ]
            .into(),
        };

        let err = capture_backup(&mut connection, &[]).await.unwrap_err();

        let CreateBackupError::IncompleteCapture {
            partial,
            command,
            source,
        } = err
        else {
            panic!("expected an interrupted capture, got {err:?}");
        };

        assert_eq!(command, "mouse.speed");
        assert!(matches!(
            source,
            RunCommandError::ResponseStreamTerminatedPrematurely
        ));

        // The partial holds exactly what was captured, and the marker
        // names the rest of the capture list in order.
        let commands: Vec<&str> = partial.backup.iter().map(|e| e.command.as_str()).collect();

        assert_eq!(commands, ["keymap.custom", "palette"]);
        assert_eq!(
            partial.missing_commands(),
            ["mouse.speed", "led.brightnessUG"]
        );

        // The probes never ran: empty identity, no descriptor.
        assert_eq!(partial.neuron_id, "");
        assert_eq!(partial.neuron.device, None);
        assert_eq!(partial.versions, parsing::backup::BackupVersions::default());

        connection.assert_spent();
    }

    #[tokio::test]
    async fn create_backup_skips_an_identity_probe_the_device_does_not_serve() {
        // The live-hardware scenario (wireless Defy over Bluetooth,
        // 2026-10-12): the device's `help` output does not list
        // `hardware.wireless` — its firmware does not serve the
        // command and answers any attempt with an empty response. The
        // probe must never be SENT (the script's assert_spent below
        // proves it), the capture must COMPLETE — marker-free,
        // Bazecor-shaped — and the descriptor must record the
        // documented wired choice.
        let mut connection = ScriptedConnection {
            script: [
                ScriptedConnection::ok(
                    "help",
                    &ScriptedConnection::help_serving(&["version", "hardware.chip_id"]),
                ),
                ScriptedConnection::ok("keymap.custom", "41 30 31"),
                ScriptedConnection::ok("palette", "255 196 0 0"),
                ScriptedConnection::ok("mouse.speed", "20"),
                ScriptedConnection::ok("led.brightnessUG", "255"),
                ScriptedConnection::ok("hardware.chip_id", "8ed7f33db024a5b5"),
                ScriptedConnection::ok("version", "v2.2.0 7622bb53 c9d9b7b-dirty"),
            ]
            .into(),
        };

        let (backup, identity) = capture_backup(&mut connection, &[]).await.unwrap();

        connection.assert_spent();

        // Complete: every command captured, no marker.
        assert_eq!(backup.backup.len(), 4);
        assert!(!backup.is_partial());
        assert!(backup.partial.is_none());

        // The obtained identity is in the file; the wireless flag is
        // the documented unknown-wireless choice (wired), inert for
        // restore validation, which reads `info.product` only.
        assert_eq!(backup.neuron_id, "8ed7f33db024a5b5");
        assert_eq!(backup.versions.bazecor.as_deref(), Some("v2.2.0"));
        assert_eq!(backup.neuron.device.as_ref().unwrap()["wireless"], false);
        assert_eq!(
            backup.neuron.device.as_ref().unwrap()["info"]["product"],
            "Defy"
        );

        // The report says exactly what was not obtained, and why.
        assert_eq!(
            identity,
            CaptureIdentity {
                chip_id: Ok("8ed7f33db024a5b5".to_owned()),
                version: Ok("v2.2.0 7622bb53 c9d9b7b-dirty".to_owned()),
                wireless: Err(IdentityProbeUnavailable::NotServed("hardware.wireless")),
            }
        );
        assert_eq!(
            identity.unavailable().collect::<Vec<_>>(),
            [IdentityProbeUnavailable::NotServed("hardware.wireless")]
        );
    }

    #[tokio::test]
    async fn create_backup_tolerates_a_served_but_empty_probe() {
        // Defensively, the response shape the skip above prevents is
        // also tolerated: `hardware.wireless` IS listed, but the
        // device answers it with an empty response. Same completion:
        // the empty answer reads as "no value", not as a capture
        // failure.
        let mut connection = ScriptedConnection {
            script: [
                ScriptedConnection::ok("help", &ScriptedConnection::help()),
                ScriptedConnection::ok("keymap.custom", "41 30 31"),
                ScriptedConnection::ok("palette", "255 196 0 0"),
                ScriptedConnection::ok("mouse.speed", "20"),
                ScriptedConnection::ok("led.brightnessUG", "255"),
                ScriptedConnection::ok("hardware.chip_id", "8ed7f33db024a5b5"),
                ScriptedConnection::ok("version", "v2.2.0 7622bb53 c9d9b7b-dirty"),
                ScriptedConnection::ok("hardware.wireless", ""),
            ]
            .into(),
        };

        let (backup, identity) = capture_backup(&mut connection, &[]).await.unwrap();

        connection.assert_spent();

        assert!(!backup.is_partial());
        assert_eq!(backup.backup.len(), 4);
        assert_eq!(backup.neuron_id, "8ed7f33db024a5b5");
        assert_eq!(backup.neuron.device.as_ref().unwrap()["wireless"], false);
        assert_eq!(
            identity,
            CaptureIdentity {
                chip_id: Ok("8ed7f33db024a5b5".to_owned()),
                version: Ok("v2.2.0 7622bb53 c9d9b7b-dirty".to_owned()),
                wireless: Err(IdentityProbeUnavailable::ReadFailed("hardware.wireless")),
            }
        );
    }

    #[tokio::test]
    async fn create_backup_tolerates_a_served_probe_answering_garbage() {
        // The last ReadFailed shape: the probe is served and answers
        // something non-empty that does not parse ("true"/"false" is
        // all `hardware.wireless` may say). Same completion as the
        // empty answer: recorded, not fatal.
        let mut connection = ScriptedConnection {
            script: [
                ScriptedConnection::ok("help", &ScriptedConnection::help()),
                ScriptedConnection::ok("keymap.custom", "41 30 31"),
                ScriptedConnection::ok("palette", "255 196 0 0"),
                ScriptedConnection::ok("mouse.speed", "20"),
                ScriptedConnection::ok("led.brightnessUG", "255"),
                ScriptedConnection::ok("hardware.chip_id", "8ed7f33db024a5b5"),
                ScriptedConnection::ok("version", "v2.2.0 7622bb53 c9d9b7b-dirty"),
                ScriptedConnection::ok("hardware.wireless", "yes"),
            ]
            .into(),
        };

        let (backup, identity) = capture_backup(&mut connection, &[]).await.unwrap();

        connection.assert_spent();

        assert!(!backup.is_partial());
        assert_eq!(backup.backup.len(), 4);
        assert_eq!(backup.neuron.device.as_ref().unwrap()["wireless"], false);
        assert_eq!(
            identity,
            CaptureIdentity {
                chip_id: Ok("8ed7f33db024a5b5".to_owned()),
                version: Ok("v2.2.0 7622bb53 c9d9b7b-dirty".to_owned()),
                wireless: Err(IdentityProbeUnavailable::ReadFailed("hardware.wireless")),
            }
        );
    }

    #[tokio::test]
    async fn create_backup_tolerates_a_probe_transport_failure() {
        // A served probe whose read dies at the transport is also
        // recorded, not fatal: the capture completes, the chip id side
        // of the file stays empty (no Neuron key), the rest of the
        // identity is filled.
        let mut connection = ScriptedConnection {
            script: [
                ScriptedConnection::ok("help", &ScriptedConnection::help()),
                ScriptedConnection::ok("keymap.custom", "41 30 31"),
                ScriptedConnection::ok("palette", "255 196 0 0"),
                ScriptedConnection::ok("mouse.speed", "20"),
                ScriptedConnection::ok("led.brightnessUG", "255"),
                ScriptedConnection::failed("hardware.chip_id"),
                ScriptedConnection::ok("version", "v2.2.0 7622bb53 c9d9b7b-dirty"),
                ScriptedConnection::ok("hardware.wireless", "true"),
            ]
            .into(),
        };

        let (backup, identity) = capture_backup(&mut connection, &[]).await.unwrap();

        connection.assert_spent();

        assert!(!backup.is_partial());
        assert_eq!(backup.backup.len(), 4);
        assert_eq!(backup.neuron_id, "");
        assert_eq!(backup.neuron.device.as_ref().unwrap()["chipId"], "");
        assert_eq!(backup.neuron.device.as_ref().unwrap()["wireless"], true);
        assert_eq!(backup.versions.bazecor.as_deref(), Some("v2.2.0"));
        assert_eq!(
            identity,
            CaptureIdentity {
                chip_id: Err(IdentityProbeUnavailable::ReadFailed("hardware.chip_id")),
                version: Ok("v2.2.0 7622bb53 c9d9b7b-dirty".to_owned()),
                wireless: Ok(true),
            }
        );
    }

    #[tokio::test]
    async fn create_backup_with_no_identity_served_completes_with_empty_identity() {
        // The extreme of the same design: the device serves none of
        // the probes (none are listed, so none are sent — the script
        // holds only the four capture reads). The capture is still
        // complete, with the empty identity shapes in the file.
        let mut connection = ScriptedConnection {
            script: [
                ScriptedConnection::ok("help", &ScriptedConnection::help_serving(&[])),
                ScriptedConnection::ok("keymap.custom", "41 30 31"),
                ScriptedConnection::ok("palette", "255 196 0 0"),
                ScriptedConnection::ok("mouse.speed", "20"),
                ScriptedConnection::ok("led.brightnessUG", "255"),
            ]
            .into(),
        };

        let (backup, identity) = capture_backup(&mut connection, &[]).await.unwrap();

        connection.assert_spent();

        assert!(!backup.is_partial());
        assert_eq!(backup.backup.len(), 4);
        assert_eq!(backup.neuron_id, "");
        assert_eq!(backup.versions, parsing::backup::BackupVersions::default());
        assert_eq!(backup.neuron.device.as_ref().unwrap()["chipId"], "");
        assert_eq!(backup.neuron.device.as_ref().unwrap()["wireless"], false);
        assert_eq!(
            identity.unavailable().collect::<Vec<_>>(),
            [
                IdentityProbeUnavailable::NotServed("hardware.chip_id"),
                IdentityProbeUnavailable::NotServed("version"),
                IdentityProbeUnavailable::NotServed("hardware.wireless"),
            ]
        );
    }

    #[tokio::test]
    async fn create_backup_help_failure_has_nothing_to_preserve() {
        // `help` failing means no capture list and no captures: the
        // plain error carries no partial, because there is nothing.
        let mut connection = ScriptedConnection {
            script: [ScriptedConnection::failed("help")].into(),
        };

        let err = capture_backup(&mut connection, &[]).await.unwrap_err();

        assert!(matches!(err, CreateBackupError::ListCommands(_)));

        connection.assert_spent();
    }

    #[tokio::test]
    async fn complete_backup_captures_only_the_missing_commands() {
        // A partial holding the first two commands of the four-command
        // capture list. The completion script covers ONLY the missing
        // two + the probes: a command the file holds is never re-read.
        let partial = partial_of(&["keymap.custom", "palette"]);
        let mut connection = ScriptedConnection {
            script: [
                ScriptedConnection::ok("help", &ScriptedConnection::help()),
                ScriptedConnection::ok("mouse.speed", "20"),
                ScriptedConnection::ok("led.brightnessUG", "255"),
                ScriptedConnection::ok("hardware.chip_id", "8ed7f33db024a5b5"),
                ScriptedConnection::ok("version", "v2.2.0 7622bb53 c9d9b7b-dirty"),
                ScriptedConnection::ok("hardware.wireless", "true"),
            ]
            .into(),
        };

        let (backup, identity) = capture_backup(&mut connection, &partial.backup)
            .await
            .unwrap();

        // Complete: the marker is gone, and the file's original
        // entries kept their order with the recaptured ones appended.
        let commands: Vec<&str> = backup.backup.iter().map(|e| e.command.as_str()).collect();

        assert_eq!(
            commands,
            [
                "keymap.custom",
                "palette",
                "mouse.speed",
                "led.brightnessUG"
            ]
        );
        assert!(!backup.is_partial());
        assert_eq!(backup.neuron_id, "8ed7f33db024a5b5");
        assert_eq!(
            backup.neuron.device.as_ref().unwrap()["chipId"],
            "8ed7f33db024a5b5"
        );
        assert!(identity.chip_id.is_ok());

        // The file's original entries are verbatim; the recaptured
        // ones carry the scripted responses.
        assert_eq!(backup.backup[0].data, "");
        assert_eq!(backup.backup[2].data, "20");

        connection.assert_spent();
    }

    #[tokio::test]
    async fn complete_backup_failure_yields_an_updated_partial() {
        // The completion dies on `led.brightnessUG`: the updated
        // partial holds the file's entries plus the one recapture, and
        // the missing list shrank accordingly.
        let partial = partial_of(&["keymap.custom", "palette"]);
        let mut connection = ScriptedConnection {
            script: [
                ScriptedConnection::ok("help", &ScriptedConnection::help()),
                ScriptedConnection::ok("mouse.speed", "20"),
                ScriptedConnection::failed("led.brightnessUG"),
            ]
            .into(),
        };

        let err = capture_backup(&mut connection, &partial.backup)
            .await
            .unwrap_err();

        let CreateBackupError::IncompleteCapture {
            partial, command, ..
        } = err
        else {
            panic!("expected an interrupted capture, got {err:?}");
        };

        assert_eq!(command, "led.brightnessUG");

        let commands: Vec<&str> = partial.backup.iter().map(|e| e.command.as_str()).collect();

        assert_eq!(commands, ["keymap.custom", "palette", "mouse.speed"]);
        assert_eq!(partial.missing_commands(), ["led.brightnessUG"]);

        connection.assert_spent();
    }

    #[tokio::test]
    async fn complete_backup_of_a_marker_only_partial_runs_the_probes_only() {
        // A file saved by an earlier build from a capture that died at
        // a probe with every command already in the file — the marker's
        // missing list is empty, a shape current captures can no longer
        // produce. Completion must run ZERO command reads — only the
        // probes — before the file is complete.
        let partial = partial_of(&[
            "keymap.custom",
            "palette",
            "mouse.speed",
            "led.brightnessUG",
        ]);

        assert!(partial.is_partial());
        assert!(!partial.partial.as_ref().unwrap().has_missing());

        let mut connection = ScriptedConnection {
            script: [
                ScriptedConnection::ok("help", &ScriptedConnection::help()),
                ScriptedConnection::ok("hardware.chip_id", "8ed7f33db024a5b5"),
                ScriptedConnection::ok("version", "v2.2.0 7622bb53 c9d9b7b-dirty"),
                ScriptedConnection::ok("hardware.wireless", "true"),
            ]
            .into(),
        };

        let (backup, identity) = capture_backup(&mut connection, &partial.backup)
            .await
            .unwrap();

        connection.assert_spent();

        // The probes alone completed it: the marker is gone, and the
        // entries are exactly the file's, verbatim and in order.
        assert!(!backup.is_partial());
        assert_eq!(backup.backup, partial.backup);
        assert_eq!(backup.neuron_id, "8ed7f33db024a5b5");
        assert_eq!(
            backup.neuron.device.as_ref().unwrap()["chipId"],
            "8ed7f33db024a5b5"
        );
        assert!(identity.unavailable().next().is_none());
    }

    #[tokio::test]
    async fn complete_backup_of_an_old_marker_only_partial_completes_over_a_probeless_device() {
        // The 2026-10-12 incident's recovery path: a file saved by the
        // previous build as a marker-only partial (every command
        // captured, the `hardware.wireless` probe aborted the
        // assembly) is completed on the very device that caused it —
        // which still does not serve the probe. The completion runs
        // the two served probes only (assert_spent proves
        // `hardware.wireless` is never sent), clears the marker, and
        // records the wireless flag as the documented wired choice.
        let partial = partial_of(&[
            "keymap.custom",
            "palette",
            "mouse.speed",
            "led.brightnessUG",
        ]);

        assert!(partial.is_partial());
        assert!(!partial.partial.as_ref().unwrap().has_missing());

        let mut connection = ScriptedConnection {
            script: [
                ScriptedConnection::ok(
                    "help",
                    &ScriptedConnection::help_serving(&["version", "hardware.chip_id"]),
                ),
                ScriptedConnection::ok("hardware.chip_id", "8ed7f33db024a5b5"),
                ScriptedConnection::ok("version", "v2.2.0 7622bb53 c9d9b7b-dirty"),
            ]
            .into(),
        };

        let (backup, identity) = capture_backup(&mut connection, &partial.backup)
            .await
            .unwrap();

        connection.assert_spent();

        assert!(!backup.is_partial());
        assert_eq!(backup.backup, partial.backup);
        assert_eq!(backup.neuron_id, "8ed7f33db024a5b5");
        assert_eq!(backup.neuron.device.as_ref().unwrap()["wireless"], false);
        assert_eq!(
            identity.unavailable().collect::<Vec<_>>(),
            [IdentityProbeUnavailable::NotServed("hardware.wireless")]
        );
    }

    #[tokio::test]
    async fn complete_backup_recomputes_against_the_current_capture_list() {
        // The device's `help` changed between the interrupted capture
        // and the completion: `macros.map` is listed now (it was not
        // in the original capture), and `mouse.speed` — the file's
        // stale missing entry — is gone from `help`. The completion
        // trusts the CURRENT list only: it captures `macros.map`,
        // never re-reads `mouse.speed` (the script would reject the
        // unscripted read), and the recomputed marker comes out empty.
        let partial = partial_of(&["keymap.custom", "palette"]);

        assert_eq!(
            partial.missing_commands(),
            ["mouse.speed", "led.brightnessUG"]
        );

        let mut connection = ScriptedConnection {
            script: [
                // The current `help` lists `macros.map` and NOT
                // `mouse.speed` — the changed-capture-list scenario —
                // with the probes spelled out so they are served.
                ScriptedConnection::ok(
                    "help",
                    "keymap.custom\npalette\nmacros.map\nversion\nhardware.chip_id\n\
                     hardware.wireless\n",
                ),
                ScriptedConnection::ok("macros.map", "4 1 2 3"),
                ScriptedConnection::ok("led.brightnessUG", "255"),
                ScriptedConnection::ok("hardware.chip_id", "8ed7f33db024a5b5"),
                ScriptedConnection::ok("version", "v2.2.0 7622bb53 c9d9b7b-dirty"),
                ScriptedConnection::ok("hardware.wireless", "true"),
            ]
            .into(),
        };

        let (backup, _identity) = capture_backup(&mut connection, &partial.backup)
            .await
            .unwrap();

        connection.assert_spent();

        // The current capture list governed: the newly listed command
        // was captured, the file's own entries kept their order, and
        // nothing is missing against the list the device answers now.
        let commands: Vec<&str> = backup.backup.iter().map(|e| e.command.as_str()).collect();

        assert_eq!(
            commands,
            ["keymap.custom", "palette", "macros.map", "led.brightnessUG"]
        );
        assert!(!backup.is_partial());
        assert_eq!(backup.backup[1].data, "");
        assert_eq!(backup.backup[2].data, "4 1 2 3");
    }

    #[test]
    fn restore_plan_refuses_a_partial_backup_and_the_override_plans_it() {
        // A partial whose captured entries are the real-shaped backup:
        // the default plan refuses it with the missing list; the
        // explicit override plans exactly what the file holds (plus
        // the `led.mode 0` tail).
        let mut partial = real_ordered_backup();
        partial.partial = Some(parsing::backup::PartialCapture {
            missing: vec!["led.fade".to_owned()],
        });

        let err = DefyKeyboard::backup_restore_plan(&partial).unwrap_err();

        assert_eq!(
            err.to_string(),
            "the backup is a partial capture: 1 command(s) missing: led.fade"
        );
        assert!(matches!(err, BackupValidationError::PartialCapture { .. }));

        let plan = DefyKeyboard::backup_restore_plan_allowing_partial(&partial).unwrap();

        assert_eq!(plan.last().unwrap().command, "led.mode");
        assert_eq!(plan.len(), real_ordered_backup().backup.len() + 1);
    }

    #[test]
    fn a_partial_refusal_with_no_missing_commands_words_the_empty_case() {
        // The empty-missing marker (a capture interrupted at the
        // identity probes) must not render "0 command(s) missing: "
        // with a dangling separator: the Display says why the list is
        // empty instead.
        let mut partial = real_ordered_backup();
        partial.partial = Some(parsing::backup::PartialCapture {
            missing: Vec::new(),
        });

        let err = DefyKeyboard::backup_restore_plan(&partial).unwrap_err();

        assert_eq!(
            err.to_string(),
            "the backup is a partial capture: the capture was interrupted \
             after every command, before the backup could be assembled \
             completely"
        );
    }
}
