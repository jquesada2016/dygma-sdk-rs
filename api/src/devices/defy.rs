//! Provides the [`DefyKeyboard`] struct for programatically interacting with
//! the keyboard.

use crate::{
    focus_api::{
        CreateHidFocusApiError, FocusApiConnection, GetCommandsError, HidFocusApi, RunCommandError,
        SerialPortFocusApi, parsing,
    },
    keycode_tables::KeyKind,
};
pub use error::*;
use std::{array, str::FromStr};

mod error {
    use super::*;

    /// Error returned when creating a handle to the keyboard.
    #[derive(Debug, Display, From, Error)]
    #[display("failed to create handle to the Dygma Defy keyboard: {_0}")]
    pub struct CreateDefyKeyboardError(CreateHidFocusApiError);

    /// Error when parsing a keymap from a string slice.
    #[derive(Clone, Debug, Display, From, Error)]
    #[display("failed to parse keymap: {_0}")]
    pub struct ParseKeymapError(parsing::keymap::ParseKeymapError);

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

    /// Error returned when there are not exactly 10 layers in a [`DefyKeymap`] necessary for
    /// creating the command data.
    #[derive(Clone, Copy, Debug, Display, Error)]
    #[display("keymap does not have exactly 10 layers")]
    pub struct KeymapDoesNotHave10LayersError;

    /// Error returned from [`DefyKeyboard::apply_custom_keymap`].
    #[derive(Debug, Display, From, Error)]
    pub enum ApplyCustomKeymapError {
        /// 10 layers are required, but this keymap has a different number of them.
        #[display("{_0}")]
        IncorrectNumberOfLayers(KeymapDoesNotHave10LayersError),
        /// Command failed to run.
        #[display("{_0}")]
        CommandFailed(RunCommandError),
    }

    /// Error returned from [`DefyKeyboard::get_custom_keymap`].
    #[derive(Debug, Display, From, Error)]
    pub enum GetCustomKeymapError {
        /// Failed to run command.
        #[display("{_0}")]
        CommandFailed(RunCommandError),
        /// Keymap returned by the keyboard failed to parse.
        KeymapParsingFailure(ParseKeymapError),
    }

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
        /// The map holds more macros than the firmware can address.
        #[display("{_0}")]
        MacroMapTooLarge(MacroMapTooLargeError),
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
    /// `mouse.*`, `macros.memory`, `eeprom.contents`).
    #[derive(Debug, Display, From, Error)]
    pub enum GetSettingError {
        /// Failed to run the command.
        #[display("{_0}")]
        CommandFailed(RunCommandError),
        /// The response could not be parsed.
        #[display("{_0}")]
        ParsingFailure(parsing::settings::ParseSettingsError),
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
        /// capture list could not be computed.
        #[display("failed to list the device's commands: {_0}")]
        ListCommands(GetCommandsError),
        /// Capturing the response of one of the backup's commands
        /// failed.
        #[display("failed to capture the `{command}` response: {source}")]
        Command {
            /// The command whose response could not be captured. The
            /// capture stopped here; everything listed before it was
            /// captured and is lost with this error.
            command: String,
            /// The error the command failed with.
            source: RunCommandError,
        },
        /// Reading the Neuron's `hardware.chip_id` failed.
        #[display("failed to read the chip id: {_0}")]
        ChipId(GetSettingError),
        /// Reading the `version` response failed.
        #[display("failed to read the firmware version: {_0}")]
        Version(RunCommandError),
        /// Reading `hardware.wireless` failed.
        #[display("failed to read whether this is the wireless variant: {_0}")]
        Wireless(GetSettingError),
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
        /// Tried to clear a layer that does not exist.
        #[display("the layer does not exist in this keymap")]
        LayerDoesNotExist,
    }
}

/// Type alias for the raw keymap data.
pub type DefyLayerData = [KeyKind; KEYS_PER_LAYER];

/// Number of keys per layer.
pub const KEYS_PER_LAYER: usize = 80;

/// Number of layers in a `keymap.custom` command.
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
    const PRODUCT_NAME: &str = "DEFY";
    const BAUD_RATE: u32 = 115_200;

    const KEYMAP_CUSTOM_COMMAND_NAME: &str = "keymap.custom";
    const SUPERKEY_MAP_COMMAND_NAME: &str = "superkeys.map";
    const MACRO_MAP_COMMAND_NAME: &str = "macros.map";
    const VERSION_COMMAND_NAME: &str = "version";
    const HARDWARE_CHIP_ID_COMMAND_NAME: &str = "hardware.chip_id";
    const HARDWARE_WIRELESS_COMMAND_NAME: &str = "hardware.wireless";
    const HARDWARE_LAYOUT_COMMAND_NAME: &str = "hardware.layout";
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

    /// The memory size of the superkey map.
    pub const SUPERKEY_MEMORY_SIZE: usize = 512;

    /// The fallback memory size of the macro map, used when the
    /// `macros.memory` probe is missing or untrustworthy. Use
    /// [`DefyKeyboard::macro_memory_size`] for the real per-device
    /// value.
    pub const MACRO_MEMORY_SIZE: usize = parsing::settings::MacroMemorySize::FALLBACK as usize;

    /// Creates a handle to the keyboard.
    pub async fn new() -> Result<Self, CreateDefyKeyboardError> {
        let sp_focus_api_res = SerialPortFocusApi::new(Self::PRODUCT_NAME, Self::BAUD_RATE)
            .await
            .map(Into::into);
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
        let data = keymap.to_keymap_custom_data()?;

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
    /// Fails with [`ApplyMacroError::MacroMapTooLarge`] before touching
    /// the device when the map holds more than
    /// [`MacroMap::MAX_MACROS`] macros, since the firmware could never
    /// trigger them.
    pub async fn apply_macros(&mut self, macros: &MacroMap) -> Result<(), ApplyMacroError> {
        MacroMap::check_size(macros.0.len()).map_err(ApplyMacroError::MacroMapTooLarge)?;

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
    /// The file's neuron object carries the chip id and the Defy
    /// device descriptor, but no names yet: layer, macro, and superkey
    /// names are app-side in Bazecor, and this crate has no name store
    /// (see [`parsing::backup::BackupNeuron`]).
    pub async fn create_backup(
        &mut self,
    ) -> Result<parsing::backup::DefyBackup, CreateBackupError> {
        let help_commands = self
            .available_commands()
            .await
            .map_err(CreateBackupError::ListCommands)?;
        let capture_list = parsing::backup::backup_capture_commands(&help_commands);

        // Sequential on purpose: each command is a device round-trip,
        // and the capture order is the `help` order the format keeps.
        let mut captured = Vec::with_capacity(capture_list.len());
        for command in capture_list {
            let data = self.run_command(&command, None).await.map_err(|source| {
                CreateBackupError::Command {
                    command: command.clone(),
                    source,
                }
            })?;

            captured.push(parsing::backup::BackupCommand::new(command, data));
        }

        let chip_id = self
            .get_chip_id()
            .await
            .map_err(CreateBackupError::ChipId)?;
        let version_response = self
            .run_command(Self::VERSION_COMMAND_NAME, None)
            .await
            .map_err(CreateBackupError::Version)?;
        let wireless = self
            .is_wireless()
            .await
            .map_err(CreateBackupError::Wireless)?;

        Ok(parsing::backup::DefyBackup::capture(
            chip_id.as_str(),
            parsing::backup::BackupVersions::from_version_response(&version_response),
            captured,
            Self::backup_device_descriptor(wireless, chip_id.as_str()),
        ))
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
    pub fn backup_device_descriptor(wireless: bool, chip_id: &str) -> serde_json::Value {
        let mut descriptor = backup_device_descriptor_template();

        descriptor["wireless"] = wireless.into();
        descriptor["chipId"] = chip_id.into();

        descriptor
    }

    /// Runs a single-value settings command and parses its response.
    async fn get_setting<T>(&mut self, command: &str) -> Result<T, GetSettingError>
    where
        T: std::str::FromStr<Err = parsing::settings::ParseSettingsError>,
    {
        let res = self
            .run_command(command, None)
            .await
            .map_err(GetSettingError::CommandFailed)?;

        T::from_str(&res).map_err(GetSettingError::ParsingFailure)
    }

    /// Runs a settings command with the given write data.
    async fn set_setting(&mut self, command: &str, data: &str) -> Result<(), SetSettingError> {
        self.run_command(command, Some(data)).await?;

        Ok(())
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
#[derive(Clone, Debug, PartialEq, Eq, Hash, From, Deref, DerefMut, Deserialize)]
pub struct DefyKeymap(pub Vec<DefyKeymapLayer>);

impl FromStr for DefyKeymap {
    type Err = ParseKeymapError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let layers = s
            .parse::<parsing::keymap::Keymap>()?
            .iter()
            .map(DefyKeymapLayer::from)
            .collect();

        Ok(Self(layers))
    }
}

impl serde::Serialize for DefyKeymap {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0
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

impl DefyKeymap {
    /// Converts this keymap into a form suitable for sending over to the keyboard
    /// as the data of a `keymap.custom` command.
    pub fn to_keymap_custom_data(&self) -> Result<String, KeymapDoesNotHave10LayersError> {
        if self.0.len() != KEYMAP_CUSTOM_COMMAND_LAYERS {
            return Err(KeymapDoesNotHave10LayersError);
        };

        let data = self
            .0
            .iter()
            .map(|layer| {
                layer
                    .to_keymap_data()
                    .map(|opt| opt.unwrap_or(KeyKind::from(0)))
            })
            .collect::<Vec<_>>();

        let keymap = parsing::keymap::Keymap(data);

        Ok(keymap.to_string())
    }

    /// Clears the layer to the provided key.
    pub fn clear_layer_to(&mut self, layer: usize, key: KeyKind) -> Result<(), ClearLayerError> {
        if layer == 0 {
            return Err(ClearLayerError::ZeroIndexProvided);
        }

        let layer = layer - 1;

        if self.len() <= layer {
            return Err(ClearLayerError::LayerDoesNotExist);
        }

        self[layer] = DefyKeymapLayer::new_cleared_to(key);

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
    /// The key offset is an unsigned integer between 0 and 80 exclusive. Please refer
    /// to the [`LAYOUT`] constant for getting the key offset of a specific key.
    ///
    /// Note that keys are between 0 and 80, but the keyboard only has 70 keys, therefore,
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
/// A map holds at most [`SuperkeyMap::MAX_SUPERKEYS`] superkeys. The
/// invariant is enforced wherever a map comes into existence: parsing
/// from wire data, deserializing from JSON, and serializing all reject
/// oversized maps, so converting a [`SuperkeyMap`] to command data
/// (offline or on [`DefyKeyboard::apply_superkeys`]) can never emit a
/// map the firmware could not address.
#[derive(Clone, Debug, Default, Deref, DerefMut, PartialEq, Eq)]
pub struct SuperkeyMap(pub Vec<Superkey>);

impl SuperkeyMap {
    /// The maximum number of superkeys a [`SuperkeyMap`] can hold.
    ///
    /// The firmware triggers superkeys through a keycode range exactly
    /// 128 codes wide (`DYNAMIC_SUPER_FIRST` to `DYNAMIC_SUPER_LAST` in
    /// the firmware's `Kaleidoscope-Ranges.h`), so superkeys beyond that
    /// count could never be triggered.
    pub const MAX_SUPERKEYS: usize = 128;

    /// Checks that a map holding `superkeys` superkeys does not hold
    /// more than [`MAX_SUPERKEYS`](Self::MAX_SUPERKEYS).
    fn check_size(superkeys: usize) -> Result<(), SuperkeyMapTooLargeError> {
        if superkeys > Self::MAX_SUPERKEYS {
            return Err(SuperkeyMapTooLargeError);
        }

        Ok(())
    }
}

impl serde::Serialize for SuperkeyMap {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // Checked before the numbering below: an unchecked position past
        // 255 would overflow the 1-based `u8` numbering.
        Self::check_size(self.0.len()).map_err(serde::ser::Error::custom)?;

        self.0
            .iter()
            .copied()
            .enumerate()
            .map(|(i, key)| Superkey {
                // Cannot wrap: the size check bounds the position to
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
        let map = Self(<Vec<Superkey> as serde::Deserialize>::deserialize(
            deserializer,
        )?);

        // Checked at the JSON boundary like `FromStr` checks it at the
        // wire boundary: an oversized map cannot come into existence
        // from a config file either, so the CLI's offline
        // `superkeys to-command-data` path fails at read time instead of
        // emitting apply-ready command data for a map the firmware
        // could never address.
        Self::check_size(map.0.len()).map_err(serde::de::Error::custom)?;

        Ok(map)
    }
}

impl FromStr for SuperkeyMap {
    type Err = ParseSuperkeyMapError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let wire_map = s.parse::<parsing::superkeys::SuperkeyMap>()?;

        // Checked before the numbering below: the wire parse itself
        // imposes no count cap, so an unchecked position past 255 would
        // overflow the 1-based `u8` numbering.
        Self::check_size(wire_map.0.len())?;

        let map = Self(
            wire_map
                .0
                .into_iter()
                .enumerate()
                .map(|(i, key)| Superkey {
                    // Cannot wrap: the size check above bounds the
                    // position to `MAX_SUPERKEYS` (128), well within `u8`.
                    superkey_number: i as u8 + 1,
                    tap: key.tap,
                    hold: key.hold,
                    tap_hold: key.tap_hold,
                    double_tap: key.double_tap,
                    double_tap_hold: key.double_tap_hold,
                })
                .collect(),
        );

        Ok(map)
    }
}

impl From<&SuperkeyMap> for parsing::superkeys::SuperkeyMap {
    fn from(map: &SuperkeyMap) -> Self {
        Self(map.0.iter().copied().map(Into::into).collect())
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
/// A map holds at most [`MacroMap::MAX_MACROS`] macros. The invariant is
/// enforced wherever a map comes into existence: parsing from wire data,
/// deserializing from JSON, and serializing all reject oversized maps,
/// so converting a [`MacroMap`] to command data (offline or on
/// [`DefyKeyboard::apply_macros`]) can never emit a map the firmware
/// could not address.
#[derive(Clone, Debug, Default, Deref, DerefMut, PartialEq, Eq)]
pub struct MacroMap(pub Vec<Macro>);

impl MacroMap {
    /// The maximum number of macros a [`MacroMap`] can hold.
    ///
    /// The firmware triggers macros through a keycode range exactly 128
    /// codes wide (`DYNAMIC_MACRO_FIRST` to `DYNAMIC_MACRO_LAST` in the
    /// firmware's `Kaleidoscope-Ranges.h`), so macros beyond that count
    /// could never be triggered.
    pub const MAX_MACROS: usize = 128;

    /// Checks that a map holding `macros` macros does not hold more
    /// than [`MAX_MACROS`](Self::MAX_MACROS).
    fn check_size(macros: usize) -> Result<(), MacroMapTooLargeError> {
        if macros > Self::MAX_MACROS {
            return Err(MacroMapTooLargeError);
        }

        Ok(())
    }
}

impl serde::Serialize for MacroMap {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        Self::check_size(self.0.len()).map_err(serde::ser::Error::custom)?;

        self.0
            .iter()
            .enumerate()
            .map(|(i, m)| Macro {
                // Cannot wrap: the size check bounds the position to
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
        let map = Self(<Vec<Macro> as serde::Deserialize>::deserialize(
            deserializer,
        )?);

        // Checked at the JSON boundary like `FromStr` checks it at the
        // wire boundary: an oversized map cannot come into existence
        // from a config file either, so the CLI's offline
        // `macros to-command-data` path fails at read time instead of
        // emitting apply-ready command data for a map the firmware
        // could never address.
        Self::check_size(map.0.len()).map_err(serde::de::Error::custom)?;

        Ok(map)
    }
}

impl FromStr for MacroMap {
    type Err = ParseMacroMapError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let wire_map = s.parse::<parsing::macros::MacroMap>()?;

        // Checked before the numbering below: the wire parse itself
        // imposes no count cap, so an unchecked position past 255 would
        // overflow the 1-based `u8` numbering.
        Self::check_size(wire_map.0.len())?;

        let map = Self(
            wire_map
                .0
                .into_iter()
                .enumerate()
                .map(|(i, macro_data)| Macro {
                    // Cannot wrap: the size check above bounds the
                    // position to `MAX_MACROS` (128), well within `u8`.
                    macro_number: i as u8 + 1,
                    actions: macro_data.actions.into_iter().map(Into::into).collect(),
                })
                .collect(),
        );

        Ok(map)
    }
}

impl From<&MacroMap> for parsing::macros::MacroMap {
    fn from(map: &MacroMap) -> Self {
        Self(
            map.0
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

#[cfg(test)]
mod tests {
    use itertools::Itertools;

    use crate::keycode_tables::Blank;

    use super::*;

    const KEYMAP_DATA: &str = "41 30 31 32 33 34 0 0 0 0 35 36 37 38 39 0 43 20 26 8 21 23 0 0 0 0 28 24 12 18 19 0 57 4 22 7 9 10 17152 0 0 0 11 13 14 15 51 52 53980 29 27 6 25 5 0 0 0 0 17 16 54 55 56 0 53853 17452 44 49467 49209 226 227 0 0 231 76 49209 52028 44 49162 230 41 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 43 85 95 96 97 87 0 0 0 0 75 74 82 77 0 0 0 84 92 93 94 86 83 0 0 0 78 80 81 79 70 0 0 46 89 90 91 99 0 0 0 0 0 0 0 0 0 0 0 0 98 65535 65535 65535 0 0 0 0 0 65535 65535 65535 65535 0 0 58 59 60 61 62 63 65535 65535 64 65 66 67 68 69 0 0 0 0 22710 22709 23785 0 65535 65535 0 0 23663 0 0 65535 0 0 0 22713 22711 22733 23785 0 65535 65535 0 0 23664 20866 20865 0 0 0 0 0 0 0 19682 65535 65535 65535 65535 0 0 0 0 0 0 0 65535 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 65535 65535 0 0 0 0 0 0 0 0 53 2079 2080 2081 2101 0 65535 65535 0 2083 2095 2096 2093 2094 0 0 2078 56 2102 2103 2082 0 65535 65535 0 2084 2086 2087 45 46 0 0 0 0 49 2097 0 65535 65535 65535 65535 0 47 48 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 65535 0 0 0 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 ";
    const SUPERKEY_DATA: &str = "262 281 1 1 1 0 0 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 ";

    fn defy_keymap_layers<R>(layers: R) -> (Vec<u16>, DefyKeymap)
    where
        R: std::ops::RangeBounds<usize>,
    {
        use std::ops::Bound;

        let start = match layers.start_bound() {
            Bound::Included(i) => *i,
            Bound::Unbounded => 0,
            _ => unreachable!(),
        };
        let end = match layers.end_bound() {
            Bound::Included(i) => i + 1,
            Bound::Excluded(i) => *i,
            Bound::Unbounded => 10,
        };

        let layers = end - start;

        let layer_data = KEYMAP_DATA
            .split(' ')
            .skip(start * KEYS_PER_LAYER)
            .take(layers * KEYS_PER_LAYER);

        let keymap = layer_data.clone().join(" ").parse::<DefyKeymap>().unwrap();

        assert_eq!(keymap.0.len(), layers);

        let layer_data = layer_data.map(|s| s.parse().unwrap()).collect();

        (layer_data, keymap)
    }

    #[test]
    fn keymap_round_trips_from_str() {
        let (mut layer_data, keymap) = defy_keymap_layers(..);

        let res = keymap
            .0
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

        assert_eq!(map.0[0].macro_number, 1);
        assert_eq!(map.0[1].macro_number, 2);

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

        let actions: Vec<_> = parsed.0.iter().map(|m| m.actions.clone()).collect();
        let expected: Vec<_> = map.0.iter().map(|m| m.actions.clone()).collect();

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

        assert_eq!(map.0.len(), MacroMap::MAX_MACROS);
        assert_eq!(map.0.first().unwrap().macro_number, 1);
        assert_eq!(map.0.last().unwrap().macro_number, 128);

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
    fn macros_past_the_firmware_limit_are_rejected_when_serializing() {
        // 257 macros is well past both the firmware's 128-macro
        // addressing space and the old `u8` numbering wrap at position
        // 256 (numbers went ...254, 255, 0, 1...). A hand-written JSON
        // like this must fail to serialize, not be renumbered into a
        // plausible-looking file.
        let map = MacroMap(
            std::iter::repeat_n(
                Macro {
                    macro_number: 0,
                    actions: vec![MacroAction::Wait(1000)],
                },
                257,
            )
            .collect(),
        );

        assert!(serde_json::to_string(&map).is_err());
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

        assert_eq!(map.0.len(), MacroMap::MAX_MACROS);
        assert_eq!(map.0.first().unwrap().macro_number, 0);
        assert_eq!(map.0.last().unwrap().actions, vec![MacroAction::Wait(1000)]);

        // The accepted map round-trips back out through serialization.
        assert_eq!(
            serde_json::from_str::<MacroMap>(&serde_json::to_string(&map).unwrap())
                .unwrap()
                .0,
            map.0
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

        assert_eq!(map.0.len(), SuperkeyMap::MAX_SUPERKEYS);
        assert_eq!(map.0.first().unwrap().superkey_number, 1);
        assert_eq!(map.0.last().unwrap().superkey_number, 128);

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
    fn superkeys_past_the_firmware_limit_are_rejected_when_serializing() {
        // 257 superkeys is well past both the firmware's 128-superkey
        // addressing space and the `u8` numbering overflow at position
        // 256. A hand-written JSON like this must fail to serialize
        // — before the numbering — not panic or wrap into a
        // plausible-looking file.
        let map = SuperkeyMap(
            std::iter::repeat_n(
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
            .collect(),
        );

        assert!(serde_json::to_string(&map).is_err());
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

        assert_eq!(map.0.len(), SuperkeyMap::MAX_SUPERKEYS);
        assert_eq!(map.0.first().unwrap().superkey_number, 0);
        assert_eq!(map.0.last().unwrap().tap, None);

        // Numbering runs to exactly the firmware's limit when the
        // accepted map is serialized back out.
        let json = serde_json::to_string(&map).unwrap();

        assert!(json.contains(r#""superkey_number":128"#));
        assert!(!json.contains(r#""superkey_number":129"#));

        // The accepted map round-trips back out through serialization.
        assert_eq!(serde_json::from_str::<SuperkeyMap>(&json).unwrap().0, map.0);
    }
}
