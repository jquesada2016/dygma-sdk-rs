//! Provides the [`DefyKeyboard`] struct for programatically interacting with
//! the keyboard.

use crate::{
    focus_api::parsing::{
        self,
        keymap::{Blank, KeyKind},
        superkeys::SuperkeyMap as RawSuperkeyMap,
    },
    focus_api::{
        CreateHidFoducApiError, FocusApiConnection, HidFocusApi, RunCommandError,
        SerialPortFocusApi,
    },
};
use itertools::Itertools;
use std::{array, str::FromStr};

/// Type alias for the raw keymap data.
pub type DefyLayerData = [u16; KEYS_PER_LAYER];

/// Data shape required for the "keymap.custom" command.
pub type DefyKeymapCustomData = [Option<u16>; KEYS_PER_LAYER * KEYMAP_CUSTOM_COMMAND_LAYERS];

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

/// Error returned when creating a handle to the keyboard.
#[derive(Debug, Display, From, Error)]
#[display("failed to create handle to the Dygma Defy keyboard: {_0}")]
pub struct CreateDefyKeyboardError(CreateHidFoducApiError);

/// Error when parsing a keymap from a string slice.
#[derive(Clone, Debug, Display, From, Error)]
#[display("failed to parse keymap: {_0}")]
pub struct ParseKeymapError(parsing::keymap::ParseKeymapError);

/// Error when parsing a superkey map from a string slice.
#[derive(Clone, Debug, Display, From, Error)]
#[display("failed to parse superkeys map: {_0}")]
pub struct ParseSuperkeyMapError(parsing::superkeys::ParseSuperkeyMapError);

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
    TooManySuperkeys(TooManySuperkeysError),
    /// Command failed to run.
    #[display("{_0}")]
    CommandFailed(RunCommandError),
}

/// Error returned when there are too many superkeys in a [`SuperkeyMap`]
/// and thus, creating the command data would overflow.
#[derive(Clone, Copy, Debug, Display, Error)]
pub struct TooManySuperkeysError;

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
        let data = keymap
            .to_keymap_custom_data()?
            .into_iter()
            .map(|key| key.unwrap_or_default())
            .join(" ");

        self.run_command(Self::KEYMAP_CUSTOM_COMMAND_NAME, Some(&data))
            .await?;

        Ok(())
    }

    /// Apply the superkeys map to the keyboard.
    pub async fn apply_superkeys(
        &mut self,
        superkeys: &SuperkeyMap,
    ) -> Result<(), ApplySuperkeyError> {
        let data = superkeys.to_superkey_map_data()?.into_iter().join(" ");

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
#[derive(Clone, Debug, Deref, DerefMut, Deserialize)]
pub struct DefyKeymap(pub Vec<DefyKeymapLayer>);

impl FromStr for DefyKeymap {
    type Err = ParseKeymapError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let layers = s
            .parse::<parsing::keymap::RawKeymap>()?
            .iter()
            .map(Into::into)
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
    ///
    /// Please refer to [`DefyKeymapLayer::to_keymap_data`] for more details as to
    /// why this function returns `Option<u16>` rathern than `u16`.
    pub fn to_keymap_custom_data(
        &self,
    ) -> Result<DefyKeymapCustomData, KeymapDoesNotHave10LayersError> {
        if self.0.len() != KEYMAP_CUSTOM_COMMAND_LAYERS {
            return Err(KeymapDoesNotHave10LayersError);
        };

        let data = self
            .0
            .iter()
            .flat_map(|layer| layer.to_keymap_data())
            .collect::<Vec<_>>()
            .try_into()
            .expect("`keymap.custom` command data has 800 entries");

        Ok(data)
    }
}

/// A single human-readable Defy layer.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
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
    /// **Note**: This function returns `Option<u16>`, rather than `u16`.
    /// This is done because a keymap layer must contain 80 keys, but the keyboard only
    /// has 70 keys. Therefore, there are 10 missing keys. You should therefore pick
    /// a difault placeholder key, usually `u16::MIN` or `u16::MAX`.
    pub fn to_keymap_data(&self) -> [Option<u16>; KEYS_PER_LAYER] {
        array::from_fn(|i| self.get_key_by_index(i as u8).map(Into::into))
    }
}

/// Left half human-readable Defy keymap.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
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

        let row_1 = left_layout
            .row_1
            .map(|index| layer_data[index as usize])
            .map(KeyKind::from);

        let row_2 = left_layout
            .row_2
            .map(|index| layer_data[index as usize])
            .map(KeyKind::from);

        let row_3 = left_layout
            .row_3
            .map(|index| layer_data[index as usize])
            .map(KeyKind::from);

        let row_4 = left_layout
            .row_4
            .map(|index| layer_data[index as usize])
            .map(KeyKind::from);

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
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
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
            .map(|index| layer_data[index as usize])
            .map(KeyKind::from);

        let bottom = left_layout
            .thumb_cluster
            .bottom
            .map(|index| layer_data[index as usize])
            .map(KeyKind::from);

        Self { top, bottom }
    }
}

/// Right half human-readable Defy keymap.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
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

        let row_1 = right_layout
            .row_1
            .map(|index| layer_data[index as usize])
            .map(KeyKind::from);

        let row_2 = right_layout
            .row_2
            .map(|index| layer_data[index as usize])
            .map(KeyKind::from);

        let row_3 = right_layout
            .row_3
            .map(|index| layer_data[index as usize])
            .map(KeyKind::from);

        let row_4 = right_layout
            .row_4
            .map(|index| layer_data[index as usize])
            .map(KeyKind::from);

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
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
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
            .map(|index| layer_data[index as usize])
            .map(KeyKind::from);

        let bottom = right_layout
            .thumb_cluster
            .bottom
            .map(|index| layer_data[index as usize])
            .map(KeyKind::from);

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
                macro_number: i as u8 + 1,
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
            .parse::<RawSuperkeyMap>()?
            .0
            .into_iter()
            .enumerate()
            .map(|(i, key)| Superkey {
                macro_number: i as u8 + 1,
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

impl SuperkeyMap {
    /// Converts this type into a form suitable for sending to the keyboard.
    pub fn to_superkey_map_data(&self) -> Result<[u16; 512], TooManySuperkeysError> {
        let mut data = self
            .0
            .iter()
            .flat_map(|key| key.to_superkey_map_data())
            .collect::<Vec<_>>();

        data.push(0);

        if data.len() > 512 {
            return Err(TooManySuperkeysError);
        }

        data.resize(512, u16::MAX);

        Ok(data.try_into().expect("size should be exactly 512"))
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
    pub macro_number: u8,
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

impl Superkey {
    /// Converts this type into a form suitable for sending to the keyboard.
    pub fn to_superkey_map_data(&self) -> [u16; 6] {
        let Self {
            macro_number: _,
            tap,
            hold,
            tap_hold,
            double_tap,
            double_tap_hold,
        } = self;

        let action_to_u16 = |key| match key {
            Some(KeyKind::Blank(Blank::NoKey)) => 1,
            Some(key) => key.into(),
            None => 1,
        };

        let tap = action_to_u16(*tap);
        let hold = action_to_u16(*hold);
        let tap_hold = action_to_u16(*tap_hold);
        let double_tap = action_to_u16(*double_tap);
        let double_tap_hold = action_to_u16(*double_tap_hold);

        [tap, hold, tap_hold, double_tap, double_tap_hold, 0]
    }
}

#[cfg(test)]
mod tests {
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

                        key.unwrap_or_default()
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        assert_eq!(res, layer_data);
    }

    #[test]
    fn superkeys_round_trips_from_str() {
        let map = SUPERKEY_DATA.parse::<SuperkeyMap>().unwrap();

        let str_data = map.to_superkey_map_data().unwrap().into_iter().join(" ");

        assert_eq!(format!("{str_data} "), SUPERKEY_DATA);
    }
}
