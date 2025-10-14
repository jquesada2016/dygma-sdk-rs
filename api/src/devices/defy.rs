//! Provides the [`DefyKeyboard`] struct for programatically interacting with
//! the keyboard.

use crate::{
    focus_api::{CreateFocusApiError, FocusApi},
    parsing::{self, keymap::KeyKind},
};
use std::str::FromStr;

/// Type alias for the raw keymap data.
pub type DefyLayerData = [u16; 80];

const LAYOUT: &DefyLayout = &DefyLayout {
    left: LEFT_LAYOUT,
    right: RIGHT_LAYOUT,
};
const LEFT_LAYOUT: DefyLayoutLeft = DefyLayoutLeft {
    row_1: [0, 1, 2, 3, 4, 5, 6],
    row_2: [16, 17, 18, 19, 20, 21, 22],
    row_3: [32, 33, 34, 35, 36, 37, 38],
    row_4: [48, 49, 50, 51, 52, 53],
    thumb_cluster: DefyThumbClusterLayoutLeft {
        top: [64, 65, 66, 67],
        bottom: [71, 70, 69, 68],
    },
};
const RIGHT_LAYOUT: DefyLayoutRight = DefyLayoutRight {
    row_1: [9, 10, 11, 12, 13, 14, 15],
    row_2: [25, 26, 27, 28, 29, 30, 31],
    row_3: [41, 42, 43, 44, 45, 46, 47],
    row_4: [58, 59, 60, 61, 62, 63],
    thumb_cluster: DefyThumbClusterLayoutRight {
        top: [76, 77, 78, 79],
        bottom: [75, 74, 73, 72],
    },
};

/// Error returned when creating a handle to the keyboard.
#[derive(Debug, Display, From, Error)]
#[display("failed to create handle to the Dygma Defy keyboard: {_0}")]
pub struct CreateDefyKeyboardError(CreateFocusApiError);

/// Error when parsing a keymap from a string slice.
#[derive(Clone, Debug, Display, From, Error)]
#[display("failed to parse keymap: {_0}")]
pub struct ParseKeymapError(parsing::keymap::ParseKeymapError);

/// A handle to the Dygma Defy keyboard, allowing for programatic control.
#[derive(Debug, Deref, DerefMut)]
pub struct DefyKeyboard {
    focus_api: FocusApi,
}

impl DefyKeyboard {
    const PRODUCT_NAME: &str = "DEFY";
    const BAUD_RATE: u32 = 115_200;

    /// Creates a handle to the keyboard.
    pub async fn new() -> Result<Self, CreateDefyKeyboardError> {
        let focus_api = FocusApi::new(Self::PRODUCT_NAME, Self::BAUD_RATE).await?;

        Ok(Self { focus_api })
    }
}

#[derive(Clone, Copy, Debug)]
struct DefyLayout {
    left: DefyLayoutLeft,
    right: DefyLayoutRight,
}

#[derive(Clone, Copy, Debug)]
struct DefyLayoutLeft {
    row_1: [u8; 7],
    row_2: [u8; 7],
    row_3: [u8; 7],
    row_4: [u8; 6],
    thumb_cluster: DefyThumbClusterLayoutLeft,
}

#[derive(Clone, Copy, Debug)]
struct DefyThumbClusterLayoutLeft {
    top: [u8; 4],
    bottom: [u8; 4],
}

#[derive(Clone, Copy, Debug)]
struct DefyLayoutRight {
    row_1: [u8; 7],
    row_2: [u8; 7],
    row_3: [u8; 7],
    row_4: [u8; 6],
    thumb_cluster: DefyThumbClusterLayoutRight,
}

#[derive(Clone, Copy, Debug)]
struct DefyThumbClusterLayoutRight {
    top: [u8; 4],
    bottom: [u8; 4],
}

/// Full Defy keymap.
#[derive(Clone, Debug, Deref, DerefMut)]
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

/// A single human-readable Defy layer.
#[derive(Clone, Debug)]
pub struct DefyKeymapLayer {
    /// Left half of the keyboard.
    pub left: DefyKeymapLeft,
    /// Right half of the keyboard.
    pub right: DefyKeymapRight,
}

impl From<&DefyLayerData> for DefyKeymapLayer {
    fn from(layer_data: &DefyLayerData) -> Self {
        Self {
            left: DefyKeymapLeft::from(layer_data),
            right: DefyKeymapRight::from(layer_data),
        }
    }
}

/// Left half human-readable Defy keymap.
#[derive(Clone, Debug)]
pub struct DefyKeymapLeft {
    /// Row 1.
    pub row_1: [String; 7],
    /// Row 2.
    pub row_2: [String; 7],
    /// Row 3.
    pub row_3: [String; 7],
    /// Row 4.
    pub row_4: [String; 6],
    /// Thumb cluster.
    pub thumb_cluster: DefyThumbclusterKeymapLeft,
}

impl From<&DefyLayerData> for DefyKeymapLeft {
    fn from(layer_data: &DefyLayerData) -> Self {
        let row_1 = LEFT_LAYOUT
            .row_1
            .map(|index| layer_data[index as usize])
            .map(KeyKind::from)
            .map(|key| key.to_string());

        let row_2 = LEFT_LAYOUT
            .row_2
            .map(|index| layer_data[index as usize])
            .map(KeyKind::from)
            .map(|key| key.to_string());

        let row_3 = LEFT_LAYOUT
            .row_3
            .map(|index| layer_data[index as usize])
            .map(KeyKind::from)
            .map(|key| key.to_string());

        let row_4 = LEFT_LAYOUT
            .row_4
            .map(|index| layer_data[index as usize])
            .map(KeyKind::from)
            .map(|key| key.to_string());

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
#[derive(Clone, Debug)]
pub struct DefyThumbclusterKeymapLeft {
    /// The top four keys of the thumb cluster, from left to right.
    pub top: [String; 4],
    /// The bottom four keys of the thumb cluster, from left to right.
    pub bottom: [String; 4],
}

impl From<&DefyLayerData> for DefyThumbclusterKeymapLeft {
    fn from(layer_data: &DefyLayerData) -> Self {
        let top = LEFT_LAYOUT
            .thumb_cluster
            .top
            .map(|index| layer_data[index as usize])
            .map(KeyKind::from)
            .map(|key| key.to_string());

        let bottom = LEFT_LAYOUT
            .thumb_cluster
            .bottom
            .map(|index| layer_data[index as usize])
            .map(KeyKind::from)
            .map(|key| key.to_string());

        Self { top, bottom }
    }
}

/// Right half human-readable Defy keymap.
#[derive(Clone, Debug)]
pub struct DefyKeymapRight {
    /// Row 1.
    pub row_1: [String; 7],
    /// Row 2.
    pub row_2: [String; 7],
    /// Row 3.
    pub row_3: [String; 7],
    /// Row 4.
    pub row_4: [String; 6],
    /// Thumb cluster.
    pub thumb_cluster: DefyThumbclusterKeymapRight,
}

impl From<&DefyLayerData> for DefyKeymapRight {
    fn from(layer_data: &DefyLayerData) -> Self {
        let row_1 = RIGHT_LAYOUT
            .row_1
            .map(|index| layer_data[index as usize])
            .map(KeyKind::from)
            .map(|key| key.to_string());

        let row_2 = RIGHT_LAYOUT
            .row_2
            .map(|index| layer_data[index as usize])
            .map(KeyKind::from)
            .map(|key| key.to_string());

        let row_3 = RIGHT_LAYOUT
            .row_3
            .map(|index| layer_data[index as usize])
            .map(KeyKind::from)
            .map(|key| key.to_string());

        let row_4 = RIGHT_LAYOUT
            .row_4
            .map(|index| layer_data[index as usize])
            .map(KeyKind::from)
            .map(|key| key.to_string());

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
#[derive(Clone, Debug)]
pub struct DefyThumbclusterKeymapRight {
    /// The top four keys of the thumb cluster, from left to right.
    pub top: [String; 4],
    /// The bottom four keys of the thumb cluster, from left to right.
    pub bottom: [String; 4],
}

impl From<&DefyLayerData> for DefyThumbclusterKeymapRight {
    fn from(layer_data: &DefyLayerData) -> Self {
        let top = RIGHT_LAYOUT
            .thumb_cluster
            .top
            .map(|index| layer_data[index as usize])
            .map(KeyKind::from)
            .map(|key| key.to_string());

        let bottom = RIGHT_LAYOUT
            .thumb_cluster
            .bottom
            .map(|index| layer_data[index as usize])
            .map(KeyKind::from)
            .map(|key| key.to_string());

        Self { top, bottom }
    }
}
