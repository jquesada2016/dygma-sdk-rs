//! Provides the format-level model of the Dygma Sonsei: the
//! human-readable config types (`SonseiKeymap`, `SonseiColormap`), the
//! physical [`LAYOUT`], the product's default config data, and the
//! offline restore-plan validation — everything about the Sonsei that
//! needs no transport.
//!
//! There is deliberately **no `SonseiKeyboard` device type**: this
//! crate has no Sonsei connection (no serial/HID transport for the
//! product yet — issue #24's unit 4), and a handle that cannot talk
//! would be a lie. The Sonsei-shaped data this module types is
//! produced and consumed offline: `config init --product sonsei`
//! writes it, `backup restore --dry-run` validates a captured Sonsei
//! backup against it, and `backup convert` produces it. A real
//! (device-writing) restore of a Sonsei backup is not possible here —
//! the send path is and stays
//! [`DefyKeyboard::restore_backup`](crate::devices::defy::DefyKeyboard::restore_backup),
//! whose product guard refuses non-Defy backups; the cross-product
//! guard from issue #24 stands untouched.
//!
//! ## Geometry (the evidence map)
//!
//! - **Keymap**: 10 layers × 60 wire positions — a 5×12 wire matrix,
//!   contiguous `0..59`, layer-major, one u16 per position (the
//!   shipped Sonsei backup's `keymap.custom` is 600 tokens; the
//!   Defy → Sonsei conversion's 600-token output shape matches it,
//!   pinned by the conversions unit's differential tests).
//! - **Physical keys**: 56. Rows 1–4 are 12 keys each at wire
//!   positions 0–47. The bottom row is the thumb row: its left cluster
//!   sits at wire positions 48–51 and its right cluster at 56–59; wire
//!   positions 52–55 (matrix columns 4–7) name no key — the Sonsei's
//!   four unmodeled slots, the analogue of the Defy's ten. They
//!   serialize as `0` (what this crate's keymap serializer emits for
//!   padding), and the shipped backup's layer 1 carries `0` there.
//!   Three independent Bazecor/capture sources agree on the gap:
//!   Bazecor's own Sonsei keymap editor
//!   (`hardware-dygma-sonsei-wireless/components/Keymap.jsx`), whose
//!   `led_map` bottom row is `[48, 49, 50, 51, XX, XX, XX, XX, 52,
//!   53, 54, 55]` — the same four columns keyless, in Bazecor's own
//!   rendering; the Defy → Sonsei conversion table (`defyToSonsei.ts`
//!   `buildKeymapIndexMap`, byte-validated by the conversions unit);
//!   and the shipped backup's layer 1 (real keys at 48–51 and 56–59,
//!   `0`s at 52–55). The shipped device descriptor's `keyboard.left`
//!   bottom row carries eight entries, 48–55 — exactly the eight
//!   physical bottom-row keys' LED indices in the editor's `led_map` —
//!   so that row tracks the LED numbering (which the keyboard's 56
//!   LEDs fill contiguously), not the keymap wire positions (whose
//!   right cluster sits at 56–59 per the shipped capture); recorded,
//!   not replicated, like the descriptor-level outliers the
//!   conversions unit records. The Sonsei firmware sketch is not in
//!   the local Kaleidoscope checkout (only the Raise's device driver
//!   is), so no ground truth above Bazecor exists for this product.
//! - **Colormap**: 10 layers × 56 LEDs — keyboard only, no underglow
//!   (the descriptor's `keyboard.ledsLeft` is `0..55`,
//!   `ledsRight`/`keyboardUnderglow` are empty). One decimal per LED,
//!   the same wire form as the Defy's map (560 decimals in the shipped
//!   backup, all 0–15, every one a storable palette index — the
//!   nibble-packing constraint is the firmware's EEPROM storage, and
//!   the recorded wire never exceeds it).
//! - **Palette**: 16 colors × 3 RGB channels — `RGBWMode: false`,
//!   carried explicitly by BOTH of the descriptor's Bazecor sources,
//!   the hardware-virtual keyboard and the shipped backup (48 decimals
//!   on the wire; the shipped `palette` capture is exactly that; see
//!   [`RgbPalette`][crate::focus_api::parsing::palette::RgbPalette]
//!   for the wire type and the design call that keeps it beside the
//!   Defy's RGBW [`Palette`][crate::focus_api::parsing::palette::Palette]).
//! - **Descriptor** (`sonsei_device_descriptor` in the conversions
//!   module): vendor Dygma, product `Sonsei`, keyboardType `wireless`,
//!   USB 13807/49, `hasRF: false`, `sides: 1` — transcribed verbatim
//!   from the shipped backup by the conversions unit, where it lives
//!   because that is who rewrites descriptors.
//! - **Superkeys and macros** are product-independent wire formats:
//!   the Defy module's
//!   [`SuperkeyMap`][crate::devices::defy::SuperkeyMap] (512-slot
//!   memory, 128-superkey firmware addressing limit) and
//!   [`MacroMap`][crate::devices::defy::MacroMap] (128-macro limit)
//!   apply to the Sonsei unchanged — the shipped Sonsei backup's
//!   entries parse through them, and the restore validation (through
//!   the shared `devices::plan` skeleton) uses them for exactly
//!   that reason.

use crate::{
    devices::colormap_serde::{colormap_layer_serde, colormap_map_impls},
    devices::defy::{ParseMacroMapError, ParseSuperkeyMapError},
    focus_api::parsing,
    keycode_tables::KeyKind,
};
pub use error::*;
use std::{array, str::FromStr};

mod error {
    use super::*;

    /// Error when parsing a Sonsei keymap from a string slice.
    #[derive(Clone, Debug, Display, From, Error)]
    pub enum ParseKeymapError {
        /// The keymap data is not valid wire data.
        #[display("failed to parse keymap: {_0}")]
        MalformedData(parsing::keymap::ParseKeymapError),
        /// The keymap data is valid wire data but does not hold exactly
        /// the ten layers every Sonsei keymap holds.
        #[display("{_0}")]
        IncorrectLayerCount(KeymapDoesNotHave10LayersError),
    }

    /// Error returned when building a [`SonseiKeymap`] from anything
    /// but exactly 10 layers (the number every Sonsei keymap holds and
    /// every `keymap.custom` write carries).
    #[derive(Clone, Copy, Debug, Display, Error)]
    #[display("keymap does not have exactly 10 layers")]
    pub struct KeymapDoesNotHave10LayersError;

    /// Error when parsing a Sonsei colormap from a string slice.
    #[derive(Clone, Debug, Display, From, Error)]
    pub enum ParseColormapError {
        /// The colormap data is not valid wire data.
        #[display("failed to parse colormap map: {_0}")]
        MalformedData(parsing::colormap::ParseColormapError),
        /// The colormap data is valid wire data but does not hold
        /// exactly the ten layers every Sonsei colormap holds.
        #[display("{_0}")]
        IncorrectLayerCount(ColormapDoesNotHave10LayersError),
    }

    /// Error returned when building a [`SonseiColormap`] from anything
    /// but exactly 10 layers (the number every Sonsei colormap holds
    /// and every `colormap.map` write carries).
    #[derive(Clone, Copy, Debug, Display, Error)]
    #[display("colormap does not have exactly 10 layers")]
    pub struct ColormapDoesNotHave10LayersError;

    /// The Display message of a partial-capture refusal, shared with
    /// the Defy's validation — see [`parsing::backup::PartialCapture::
    /// refusal_display`], the wording's single source.
    pub(super) fn partial_capture_display(missing: &[String]) -> String {
        parsing::backup::PartialCapture {
            missing: missing.to_vec(),
        }
        .refusal_display()
    }

    /// Error returned when a backup fails Sonsei restore validation.
    ///
    /// Validation ([`backup_restore_plan`]) runs entirely offline:
    /// a backup either validates whole, or nothing leaves this module.
    /// The Sonsei sibling of the Defy's
    /// [`BackupValidationError`][defy-validation] — the cross-product
    /// guard mirrored: a backup whose descriptor names another product
    /// is refused here too, so the guard stands in both directions and
    /// conversion remains the explicit pre-step.
    ///
    /// [defy-validation]:
    ///     crate::devices::defy::BackupValidationError
    #[derive(Clone, Debug, Display, Error)]
    pub enum BackupValidationError {
        /// The backup's device descriptor names a product other than
        /// the Sonsei. Restoring another product's backup to a Sonsei
        /// would write foreign data to it (and no Sonsei transport
        /// exists in this crate yet — see the [module docs](super)).
        #[display("the backup is for a `{product}`, not a Sonsei")]
        NotASonseiBackup {
            /// The product the backup's device descriptor names.
            product: String,
        },
        /// One of the backup's captured commands carries data this
        /// crate can prove is not valid Sonsei wire data. The keymap,
        /// superkeys, macros, palette, and colormap entries are each
        /// validated by parsing them against their wire formats (the
        /// map types also enforce the write shapes); captured commands
        /// without a Sonsei-shaped validator pass through verbatim,
        /// exactly the way Bazecor's restore sends them.
        #[display("the `{command}` entry failed restore validation: {source}")]
        InvalidEntry {
            /// The captured command whose data failed validation.
            command: String,
            /// The parse failure.
            source: BackupEntryError,
        },
        /// The backup carries the partial-capture marker
        /// ([`parsing::backup::PartialCapture`]): it was saved from a
        /// capture that failed partway. Refused unless explicitly
        /// allowed (see [`backup_restore_plan_allowing_partial`]).
        #[display("{}", partial_capture_display(missing))]
        PartialCapture {
            /// The commands the capture list expects and the file does
            /// not hold — what restoring it would leave uncaptured.
            missing: Vec<String>,
        },
    }

    /// The parse failure behind
    /// [`BackupValidationError::InvalidEntry`]: one variant per
    /// captured command this crate can validate against the Sonsei's
    /// wire shapes.
    #[derive(Clone, Debug, Display, Error)]
    pub enum BackupEntryError {
        /// The `keymap.custom` data is not a valid Sonsei keymap —
        /// either not valid wire data at all, or not the exactly ten
        /// layers every `keymap.custom` write carries (the wire parser
        /// itself accepts any layer count, so the [`SonseiKeymap`]
        /// type enforces the count at construction).
        #[display("{_0}")]
        Keymap(ParseKeymapError),
        /// The `superkeys.map` data is not a valid superkeys map (the
        /// superkey wire format is product-independent).
        #[display("{_0}")]
        Superkeys(ParseSuperkeyMapError),
        /// The `macros.map` data is not a valid macros map (the macro
        /// wire format is product-independent).
        #[display("{_0}")]
        Macros(ParseMacroMapError),
        /// The `palette` data is not a valid RGB palette — either not
        /// valid wire data at all, or not the exactly 48 decimals (16
        /// RGB colors) a `RGBWMode: false` palette carries.
        #[display("{_0}")]
        Palette(parsing::palette::ParseRgbPaletteError),
        /// The `colormap.map` data is not a valid Sonsei colormap —
        /// either not valid wire data at all, or not the exactly ten
        /// 56-LED layers every Sonsei `colormap.map` write carries.
        #[display("{_0}")]
        Colormap(ParseColormapError),
    }
}

/// Type alias for the raw Sonsei keymap layer data.
pub type SonseiLayerData = [KeyKind; KEYS_PER_LAYER];

/// Number of wire positions per Sonsei keymap layer (the 5×12 wire
/// matrix). 56 of the 60 positions are physical keys — see [`LAYOUT`]
/// and the [module docs](self) for the four unmodeled slots.
pub const KEYS_PER_LAYER: usize = 60;

/// Number of layers in a Sonsei `keymap.custom` command.
///
/// This is the exact layer count of every Sonsei keymap: the device
/// stores and reports exactly this many, and [`SonseiKeymap`] can only
/// hold this many.
pub const KEYMAP_CUSTOM_COMMAND_LAYERS: usize = 10;

/// Number of LEDs per Sonsei colormap layer: keyboard only, no
/// underglow (the descriptor's `keyboard.ledsLeft` covers 0–55 and it
/// carries no underglow arrays).
pub const LEDS_PER_LAYER: usize = 56;

/// Structure representing the physical layout of the Sonsei keyboard:
/// one piece (the descriptor carries `sides: 1` and an empty
/// `keyboard.right`), four 12-key rows and an 8-key thumb row.
///
/// Every entry is a wire position of the layer's 60; the four
/// positions the layout never names (52–55, the thumb row's center)
/// are the unmodeled slots — see the [module docs](self).
#[derive(Clone, Copy, Debug)]
pub struct SonseiLayout {
    /// Row 1 key indices.
    pub row_1: [u8; 12],
    /// Row 2 key indices.
    pub row_2: [u8; 12],
    /// Row 3 key indices.
    pub row_3: [u8; 12],
    /// Row 4 key indices.
    pub row_4: [u8; 12],
    /// The thumb row: the bottom matrix row's 8 physical keys, wire
    /// positions 48–51 (left) and 56–59 (right); matrix columns 4–7
    /// between them name no key.
    pub thumb_row: SonseiThumbRowLayout,
}

/// Thumb row layout of the Sonsei keyboard.
#[derive(Clone, Copy, Debug)]
pub struct SonseiThumbRowLayout {
    /// The left cluster's four keys, from left to right (wire
    /// positions 48–51).
    pub left: [u8; 4],
    /// The right cluster's four keys, from left to right (wire
    /// positions 56–59).
    pub right: [u8; 4],
}

/// Constant providing the Sonsei keymap layout.
pub const LAYOUT: &SonseiLayout = &SonseiLayout {
    row_1: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
    row_2: [12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23],
    row_3: [24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35],
    row_4: [36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47],
    thumb_row: SonseiThumbRowLayout {
        left: [48, 49, 50, 51],
        right: [56, 57, 58, 59],
    },
};

/// Full Sonsei keymap.
///
/// A Sonsei keymap holds exactly [`KEYMAP_CUSTOM_COMMAND_LAYERS`]
/// layers — the device stores exactly that many, and every
/// `keymap.custom` write carries exactly that many — so a
/// [`SonseiKeymap`] cannot hold a different number of layers by
/// construction. Building one from a count that is not known at
/// compile time (wire data, a JSON file) is fallible: it goes through
/// [`TryFrom<Vec<SonseiKeymapLayer>>`](TryFrom) and fails with
/// [`KeymapDoesNotHave10LayersError`] when the count is wrong.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Deref, DerefMut, From)]
pub struct SonseiKeymap {
    /// The keymap's ten layers, in wire order (layer 1 first).
    pub layers: [SonseiKeymapLayer; KEYMAP_CUSTOM_COMMAND_LAYERS],
}

impl TryFrom<Vec<SonseiKeymapLayer>> for SonseiKeymap {
    type Error = KeymapDoesNotHave10LayersError;

    fn try_from(layers: Vec<SonseiKeymapLayer>) -> Result<Self, Self::Error> {
        let layers = layers
            .try_into()
            .map_err(|_| KeymapDoesNotHave10LayersError)?;

        Ok(Self { layers })
    }
}

impl FromStr for SonseiKeymap {
    type Err = ParseKeymapError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let layers: Vec<SonseiKeymapLayer> = s
            .parse::<parsing::keymap::Keymap<{ KEYS_PER_LAYER }>>()?
            .iter()
            .map(SonseiKeymapLayer::from)
            .collect();

        Ok(Self::try_from(layers)?)
    }
}

impl serde::Serialize for SonseiKeymap {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.layers
            .iter()
            .copied()
            .enumerate()
            .map(|(i, layer)| SonseiKeymapLayer {
                layer_number: i as u8 + 1,
                ..layer
            })
            .collect::<Vec<_>>()
            .serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for SonseiKeymap {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let layers = <Vec<SonseiKeymapLayer> as serde::Deserialize>::deserialize(deserializer)?;

        // Checked at the JSON boundary like `FromStr` checks it at the
        // wire boundary: a config file holding a different number of
        // layers cannot come into existence as a [`SonseiKeymap`]
        // either, so every consumer of the type can rely on the write
        // shape.
        Self::try_from(layers).map_err(serde::de::Error::custom)
    }
}

impl SonseiKeymap {
    /// Converts this keymap into a form suitable for sending over to
    /// the keyboard as the data of a `keymap.custom` command.
    ///
    /// Infallible by construction: a [`SonseiKeymap`] holds exactly
    /// the ten layers the command requires. The four unmodeled wire
    /// positions per layer serialize as `0` — the value this crate's
    /// keymap serializer has always emitted for padding.
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
}

/// A single human-readable Sonsei layer.
#[derive(Clone, Copy, Debug, Eq, Serialize, Deserialize)]
pub struct SonseiKeymapLayer {
    /// A human-readable label for knowing what layer you're editing in
    /// the JSON file.
    ///
    /// **Note**: This number is only a UX aid, and is not taken
    /// into account when deserializing from the config file.
    #[serde(skip_deserializing)]
    pub layer_number: u8,
    /// Row 1.
    pub row_1: [KeyKind; 12],
    /// Row 2.
    pub row_2: [KeyKind; 12],
    /// Row 3.
    pub row_3: [KeyKind; 12],
    /// Row 4.
    pub row_4: [KeyKind; 12],
    /// The thumb row: the bottom matrix row's 8 physical keys.
    pub thumb_row: SonseiThumbRowKeymap,
}

impl From<&SonseiLayerData> for SonseiKeymapLayer {
    fn from(layer_data: &SonseiLayerData) -> Self {
        Self {
            layer_number: 0,
            row_1: LAYOUT.row_1.map(|index| layer_data[index as usize]),
            row_2: LAYOUT.row_2.map(|index| layer_data[index as usize]),
            row_3: LAYOUT.row_3.map(|index| layer_data[index as usize]),
            row_4: LAYOUT.row_4.map(|index| layer_data[index as usize]),
            thumb_row: SonseiThumbRowKeymap::from(layer_data),
        }
    }
}

impl PartialEq for SonseiKeymapLayer {
    fn eq(&self, other: &Self) -> bool {
        self.row_1 == other.row_1
            && self.row_2 == other.row_2
            && self.row_3 == other.row_3
            && self.row_4 == other.row_4
            && self.thumb_row == other.thumb_row
    }
}

impl std::hash::Hash for SonseiKeymapLayer {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.row_1.hash(state);
        self.row_2.hash(state);
        self.row_3.hash(state);
        self.row_4.hash(state);
        self.thumb_row.hash(state);
    }
}

impl SonseiKeymapLayer {
    /// Get's the corresponding key given the key offset.
    ///
    /// The key offset is an unsigned integer in the range `0..60`.
    /// Please refer to the [`LAYOUT`] constant for getting
    /// the key offset of a specific key.
    ///
    /// Note that keys are in `0..60`, but the keyboard only has
    /// 56 keys, therefore, some indices will return `None`, even
    /// though a keymap will contain a key code. In these cases, you
    /// can use either `u16::MIN` or `u16::MAX`, as it is only a padded
    /// placeholder.
    fn get_key_by_index(&self, index: u8) -> Option<KeyKind> {
        if index >= KEYS_PER_LAYER as u8 {
            return None;
        }

        macro_rules! get_index {
            ($(($($path:ident),*)),* $(,)?) => {
                None
                  $(
                    .or_else(|| {
                        LAYOUT
                            $(.$path)*
                            .iter()
                            .copied()
                            .position(|key_index| key_index == index)
                            .map(|i| self$(.$path)*[i])
                    })
                  )*
            };
        }

        get_index! {
            (row_1),
            (row_2),
            (row_3),
            (row_4),
            (thumb_row, left),
            (thumb_row, right),
        }
    }

    /// Converts this layer into a form suitable for using with keymap
    /// commands.
    ///
    /// **Note**: This function returns [`Option<KeyKind>`], rather
    /// than [`KeyKind`]. This is done because a keymap layer must
    /// contain 60 keys, but the keyboard only has 56 keys. Therefore,
    /// there are 4 missing keys (the thumb row's center, wire
    /// positions 52–55). You should therefore pick a placeholder key,
    /// usually `u16::MIN` or `u16::MAX`.
    pub fn to_keymap_data(&self) -> [Option<KeyKind>; KEYS_PER_LAYER] {
        array::from_fn(|i| self.get_key_by_index(i as u8))
    }

    /// Clears the entire layer to the provided key.
    pub fn new_cleared_to(key: KeyKind) -> Self {
        let keys: SonseiLayerData = [key; KEYS_PER_LAYER];

        SonseiKeymapLayer::from(&keys)
    }
}

/// The Sonsei thumb row's human-readable keymap: the bottom matrix
/// row's 8 physical keys, in two clusters of four.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SonseiThumbRowKeymap {
    /// The left cluster's four keys, from left to right (wire
    /// positions 48–51).
    pub left: [KeyKind; 4],
    /// The right cluster's four keys, from left to right (wire
    /// positions 56–59).
    pub right: [KeyKind; 4],
}

impl From<&SonseiLayerData> for SonseiThumbRowKeymap {
    fn from(layer_data: &SonseiLayerData) -> Self {
        let thumb_row = &LAYOUT.thumb_row;

        let left = thumb_row.left.map(|index| layer_data[index as usize]);
        let right = thumb_row.right.map(|index| layer_data[index as usize]);

        Self { left, right }
    }
}

/// Full Sonsei colormap: the palette indices painting every LED, on
/// every layer.
///
/// A Sonsei colormap holds exactly [`KEYMAP_CUSTOM_COMMAND_LAYERS`]
/// layers — the device stores and reports exactly that many, and every
/// `colormap.map` write carries them — so a [`SonseiColormap`] cannot
/// hold a different number of layers by construction. Building one
/// from a count that is not known at compile time (wire data, a JSON
/// file) is fallible: it goes through
/// [`TryFrom<Vec<SonseiColormapLayer>>`](TryFrom) and fails with
/// [`ColormapDoesNotHave10LayersError`] when the count is wrong.
///
/// Each layer paints the keyboard's 56 LEDs; the Sonsei has no
/// underglow (see the [module docs](self)).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Deref, DerefMut, From)]
pub struct SonseiColormap {
    /// The colormap's ten layers, in wire order (layer 1 first).
    pub layers: [SonseiColormapLayer; KEYMAP_CUSTOM_COMMAND_LAYERS],
}

colormap_map_impls! {
    SonseiColormap,
    SonseiColormapLayer,
    parsing::colormap::Colormap<{ LEDS_PER_LAYER }>,
    layer_count_error: ColormapDoesNotHave10LayersError,
    parse_error: ParseColormapError,
}

/// A single human-readable Sonsei colormap layer.
///
/// Serialized as a JSON object of two fields; `layer_number` is a
/// write-only UX aid like the keymap layers' (a file carrying one has
/// it ignored on the way in), and the `keyboard` array is
/// length-checked on the way in — exactly 56 indices. The serde impls
/// come from the shared `colormap_layer_serde!` macro (56 lands past
/// the derive's array support).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SonseiColormapLayer {
    /// A human-readable label for knowing what layer you're editing in
    /// the JSON file.
    ///
    /// **Note**: This number is only a UX aid, and is not taken
    /// into account when deserializing from the config file.
    pub layer_number: u8,
    /// The keyboard LEDs' palette indices, in wire order (all 56 of
    /// the layer — the Sonsei has no underglow).
    pub keyboard: [parsing::colormap::PaletteIndex; LEDS_PER_LAYER],
}

colormap_layer_serde! {
    SonseiColormapLayer,
    keyboard => Keyboard [LEDS_PER_LAYER] "keyboard",
}

impl From<&parsing::colormap::ColormapLayer<{ LEDS_PER_LAYER }>> for SonseiColormapLayer {
    fn from(layer: &parsing::colormap::ColormapLayer<{ LEDS_PER_LAYER }>) -> Self {
        Self {
            layer_number: 0,
            keyboard: layer.0,
        }
    }
}

impl From<&SonseiColormapLayer> for parsing::colormap::ColormapLayer<{ LEDS_PER_LAYER }> {
    fn from(layer: &SonseiColormapLayer) -> Self {
        Self(layer.keyboard)
    }
}

/// The captured Focus commands the Sonsei restore validation knows —
/// the product-shaped ones. (The product-independent `superkeys.map`
/// and `macros.map` arms live in the shared
/// [`plan`](super::plan) skeleton.) Private: the command names are
/// the firmware's and identical across products, and nothing outside
/// this module names them.
const KEYMAP_CUSTOM_COMMAND_NAME: &str = "keymap.custom";
const PALETTE_COMMAND_NAME: &str = "palette";
const COLORMAP_MAP_COMMAND_NAME: &str = "colormap.map";

/// Computes the exact command sequence a Sonsei restore would send for
/// a backup, validating the backup first — **without any connection**
/// (no Sonsei transport exists; see the [module docs](self) for what
/// that means).
///
/// The Sonsei sibling of
/// [`DefyKeyboard::backup_restore_plan`][defy-plan]: the same
/// product-dispatched plan semantics (the order is
/// [`restore_command_sequence`][seq]'s, product-agnostic; the `led.mode 0`
/// tail Bazecor appends is added the same way), with every
/// product-shaped entry validated through the Sonsei types — a
/// 600-token `keymap.custom` through [`SonseiKeymap`], a 560-token
/// `colormap.map` through [`SonseiColormap`], a 48-decimal `palette`
/// through
/// [`RgbPalette`][crate::focus_api::parsing::palette::RgbPalette] —
/// and the product-independent maps through the shared
/// [`SuperkeyMap`][crate::devices::defy::SuperkeyMap]/
/// [`MacroMap`][crate::devices::defy::MacroMap] types. The backup's
/// device descriptor,
/// when it carries one, must name the Sonsei: the cross-product guard
/// stands in this direction too, so conversion remains the explicit
/// pre-step between products.
///
/// A backup carrying the partial-capture marker is refused; see
/// [`backup_restore_plan_allowing_partial`] for the explicit override.
///
/// [defy-plan]: crate::devices::defy::DefyKeyboard::backup_restore_plan
/// [seq]: crate::focus_api::parsing::backup::restore_command_sequence
pub fn backup_restore_plan(
    backup: &parsing::backup::DefyBackup,
) -> Result<Vec<parsing::backup::BackupCommand>, BackupValidationError> {
    backup_restore_plan_with(backup, false)
}

/// [`backup_restore_plan`] with the partial-capture refusal lifted: an
/// explicitly `--allow-partial`-style restore of a marked backup gets
/// its plan computed like any other, after acknowledging it restores
/// only part of a captured state. Everything else — the product guard,
/// the per-entry wire validation, the ordering, the `led.mode 0` tail
/// — is identical.
pub fn backup_restore_plan_allowing_partial(
    backup: &parsing::backup::DefyBackup,
) -> Result<Vec<parsing::backup::BackupCommand>, BackupValidationError> {
    backup_restore_plan_with(backup, true)
}

/// The plan computation behind the refusal pair: `allow_partial` says
/// whether a backup carrying the partial-capture marker is planned
/// anyway.
///
/// The skeleton ([`plan::restore_plan`][core]) holds the
/// product-independent shape — the partial refusal, the product
/// guard, the shared superkeys/macros entries, the plan order, the
/// `led.mode 0` tail — and [`SonseiShapes`] names the Sonsei's own
/// pieces: the product name, the refusals' wording, and the
/// Sonsei-shaped entries.
///
/// [core]: plan::restore_plan
fn backup_restore_plan_with(
    backup: &parsing::backup::DefyBackup,
    allow_partial: bool,
) -> Result<Vec<parsing::backup::BackupCommand>, BackupValidationError> {
    super::plan::restore_plan::<SonseiShapes>(backup, allow_partial)
}

/// The Sonsei's restore-validation shapes: the pieces
/// [`plan::restore_plan`](super::plan::restore_plan) wires together —
/// the product name the guard matches, the refusals' wording, and the
/// Sonsei-shaped entries (the keymap, palette, and colormap; the
/// superkeys and macros maps are the skeleton's).
struct SonseiShapes;

impl super::plan::ProductRestoreShapes for SonseiShapes {
    type ValidationError = BackupValidationError;
    type EntryError = BackupEntryError;
    const EXPECTED_PRODUCT: &'static str = "Sonsei";

    fn wrong_product(product: String) -> BackupValidationError {
        BackupValidationError::NotASonseiBackup { product }
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
            // [`SonseiKeymap`] cannot be constructed from anything
            // else, so the parse here is the whole validation.
            KEYMAP_CUSTOM_COMMAND_NAME => Some(
                SonseiKeymap::from_str(data)
                    .map(|_| ())
                    .map_err(BackupEntryError::Keymap),
            ),
            // A `RGBWMode: false` palette write is exactly 16 RGB
            // colors; the type cannot be constructed from anything
            // else.
            PALETTE_COMMAND_NAME => Some(
                parsing::palette::RgbPalette::from_str(data)
                    .map(|_| ())
                    .map_err(BackupEntryError::Palette),
            ),
            // A `colormap.map` write carries exactly the ten 56-LED
            // layers; [`SonseiColormap`] cannot be constructed from
            // anything else.
            COLORMAP_MAP_COMMAND_NAME => Some(
                SonseiColormap::from_str(data)
                    .map(|_| ())
                    .map_err(BackupEntryError::Colormap),
            ),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
