//! Provides the [`DefyKeyboard`] struct for programatically interacting with
//! the keyboard.

use crate::{
    focus_api::{
        CreateHidFocusApiError, FocusApiConnection, HidFocusApi, RunCommandError,
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
    #[display("failed to parse superkeys map: {_0}")]
    pub struct ParseSuperkeyMapError(parsing::superkeys::ParseSuperkeyMapError);

    /// Error when parsing a macro map from a string slice.
    #[derive(Clone, Debug, Display, From, Error)]
    #[display("failed to parse macros map: {_0}")]
    pub struct ParseMacroMapError(parsing::macros::ParseMacroMapError);

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
    #[derive(Debug, Display, From, Error)]
    pub enum ApplyMacroError {
        /// Too many macros were in the map to fit in the macro memory.
        #[display("{_0}")]
        TooManyMacros(parsing::macros::TooManyMacrosError),
        /// A key code was too large for a single-byte macro action.
        #[display("{_0}")]
        KeyCodeTooLarge(parsing::macros::KeyCodeTooLargeError),
        /// Command failed to run.
        #[display("{_0}")]
        CommandFailed(RunCommandError),
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

    /// The memory size of the superkey map.
    pub const SUPERKEY_MEMORY_SIZE: usize = 512;

    /// The memory size of the macro map.
    pub const MACRO_MEMORY_SIZE: usize = 2048;

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
    pub async fn apply_macros(&mut self, macros: &MacroMap) -> Result<(), ApplyMacroError> {
        let data = parsing::macros::MacroMap::from(macros)
            .to_command_data::<{ Self::MACRO_MEMORY_SIZE }>()?;

        self.run_command(Self::MACRO_MAP_COMMAND_NAME, Some(&data))
            .await?;

        Ok(())
    }
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
#[derive(Clone, Debug, Default, Deref, DerefMut, PartialEq, Eq, Deserialize)]
pub struct SuperkeyMap(pub Vec<Superkey>);

impl serde::Serialize for SuperkeyMap {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0
            .iter()
            .copied()
            .enumerate()
            .map(|(i, key)| Superkey {
                superkey_number: i as u8 + 1,
                ..key
            })
            .collect::<Vec<_>>()
            .serialize(serializer)
    }
}

impl FromStr for SuperkeyMap {
    type Err = ParseSuperkeyMapError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let map = s
            .parse::<parsing::superkeys::SuperkeyMap>()?
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
            .collect();

        Ok(Self(map))
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
#[derive(Clone, Debug, Default, Deref, DerefMut, PartialEq, Eq, Deserialize)]
pub struct MacroMap(pub Vec<Macro>);

impl serde::Serialize for MacroMap {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0
            .iter()
            .enumerate()
            .map(|(i, m)| Macro {
                macro_number: i as u8 + 1,
                actions: m.actions.clone(),
            })
            .collect::<Vec<_>>()
            .serialize(serializer)
    }
}

impl FromStr for MacroMap {
    type Err = ParseMacroMapError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let map = s
            .parse::<parsing::macros::MacroMap>()?
            .0
            .into_iter()
            .enumerate()
            .map(|(i, macro_data)| Macro {
                macro_number: i as u8 + 1,
                actions: macro_data.actions.into_iter().map(Into::into).collect(),
            })
            .collect();

        Ok(Self(map))
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
            .to_command_data::<19>()
            .unwrap();

        assert_eq!(format!("{str_data} "), data);
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
}
