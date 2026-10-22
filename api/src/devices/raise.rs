//! Provides the format-level model of the Dygma Raise (first
//! generation): the human-readable config types (`RaiseKeymap`,
//! `RaiseColormap`), the physical [`LAYOUT`], and the offline
//! restore-plan validation — everything about the Raise that needs no
//! transport.
//!
//! There is deliberately **no `RaiseKeyboard` device type**: this
//! crate has no Raise connection (no serial/HID transport for the
//! product yet — issue #24's unit 4), and a handle that cannot talk
//! would be a lie. The Raise-shaped data this module types is
//! produced and consumed offline: `config init --product raise`
//! writes it, `backup restore --dry-run` validates a captured Raise
//! backup against it, and `backup convert` produces it. A real
//! (device-writing) restore of a Raise backup is not possible here —
//! the send path is and stays
//! [`DefyKeyboard::restore_backup`](crate::devices::defy::DefyKeyboard::restore_backup),
//! whose product guard refuses non-Defy backups; the cross-product
//! guard from issue #24 stands untouched.
//!
//! The Raise comes in two physical layouts, ANSI and ISO — the
//! descriptors' `info.keyboardType`, carried through this crate as
//! [`KeyboardType`][crate::devices::conversions::KeyboardType]. The
//! layouts share every wire shape in this module (the descriptor
//! rows, the LED counts, the unmodeled slots); what differs is which
//! key codes the factory default sits at — the layouts' defaults
//! differ at exactly three layer-2 positions (17, 46, and 49, where
//! the ANSI and ISO data sit Superkey-1, Macro-1, and transparent
//! codes differently; pinned by the defaults tests) — so the layouts
//! select *data* (the defaults), not *types*.
//!
//! ## Geometry (the evidence map)
//!
//! - **Keymap**: 10 layers × 80 wire positions — the same 5×16 wire
//!   matrix as the Defy, layer-major, one u16 per position (both
//!   shipped Raise backups' `keymap.custom` entries are 800 tokens).
//! - **Physical keys**: 73. Three agreeing sources pin which of the
//!   80 wire positions name keys:
//!   1. the device descriptor's `keyboard.left`/`keyboard.right`
//!      rows — left rows of 7/7/7/7/8 keys at wire positions 0–6,
//!      16–22, 32–38, 48–54, 64–71, right rows of 7/8/7/7/8 keys at
//!      9–15, 24–31, 41–47, 57–63, 72–79 (byte-identical in all four
//!      Raise/Raise 2 artifact families: the shipped ANSI/ISO backups
//!      and the hardware-virtual `RaiseANSI`/`RaiseISO`
//!      definitions — and in Bazecor's `hardware-dygma-raise-{ansi,
//!      iso}` hardware definitions, whose `aFN(from, to)` rows
//!      expand to exactly these ranges, ANSI and ISO alike);
//!   2. the shipped backups' layer data: positions 7, 8, 23, 39, 40,
//!      55, and 56 — the descriptor gaps — NEVER carry a real key on
//!      any of the twenty layers (each carries `0` on layers 1–2 and
//!      `65535` transparent on layers 3–10 — the same pattern the
//!      Defy's and Sonsei's unmodeled slots show in their defaults).
//!      Four further positions the descriptor NAMES (22, 38, 57, 69)
//!      are also empty on all twenty layers — keys the factory
//!      default simply never maps; modeled-ness is the descriptor's
//!      fact, factory-emptiness the default's, and the LAYOUT models
//!      all of them. Every other position carries at least one real
//!      key code somewhere across the layers (pinned exactly by
//!      test);
//!   3. Bazecor's Raise keymap editor
//!      (`hardware-dygma-raise-ansi/components/Keymap-ANSI.jsx` and
//!      the ISO twin), whose `led_map` marks those same inner-column
//!      and gap columns `XX` (keyless) and whose per-half key LED
//!      count (`LEDS_LEFT_KEYS` 33, right keys 33–68) fills the
//!      descriptor's `ledsLeft`/`ledsRight` ranges.
//!
//!   The seven unmodeled positions are the Raise's analogue of the
//!   Defy's ten and the Sonsei's four. They serialize as `0` (what
//!   this crate's keymap serializer emits for padding).
//! - **Colormap**: 10 layers × 132 LEDs — 69 keyboard LEDs
//!   (positions 0–68, the descriptor's `keyboard.ledsLeft` 0–32 plus
//!   `ledsRight` 33–68) followed by the underglow, which Bazecor's
//!   own Raise converter splits 30 left (69–98) + 33 right (99–131)
//!   (`raiseToRaise2.ts`; the editor's `no_key_led_map` is exactly
//!   the 63 underglow LEDs 69–131, and the descriptor's
//!   `keyboardUnderglow` is the 6×22 = 132 grid). One decimal per
//!   LED, the same wire form as the Defy's and Sonsei's maps (the
//!   shipped backups' `colormap.map` entries are 1320 decimals, all
//!   0–15). Recorded, NOT replicated: the SHIPPED BACKUPS'
//!   descriptors' `keyboardUnderglow.ledsRight` arrays cover only
//!   99–130 — 32 entries, one short of the 33-LED right underglow
//!   every other source counts (the hardware-virtual definitions'
//!   arrays extend through 131 — `aFN(99, 132)` — so position 131 is
//!   named by no shipped-backup descriptor array; the same recorded
//!   quirk class as the Defy descriptor's unnamed positions
//!   176–177).
//! - **Palette**: 16 colors × 3 RGB channels — `RGBWMode: false`.
//!   The shipped ANSI backup's descriptor carries `RGBWMode: false`
//!   explicitly; the shipped ISO backup's descriptor OMITS the field
//!   (recorded as shipped — the data is the same 48-decimal RGB
//!   wire form on both, and Bazecor's RGB-mode palette handling is
//!   what `RGBWMode: false` spells). See
//!   [`RgbPalette`][crate::focus_api::parsing::palette::RgbPalette]
//!   for the wire type and the design call that keeps it beside the
//!   Defy's RGBW [`Palette`][crate::focus_api::parsing::palette::Palette].
//! - **USB identity**: vendor 4617, product 8705 (both shipped
//!   backups' descriptors; ANSI and ISO share it).
//! - **Superkeys and macros** are product-independent wire formats:
//!   the Defy module's
//!   [`SuperkeyMap`][crate::devices::defy::SuperkeyMap] (512-slot
//!   memory, 128-superkey firmware addressing limit) and
//!   [`MacroMap`][crate::devices::defy::MacroMap] (128-macro limit)
//!   apply to the Raise unchanged. Both shipped Raise backups'
//!   entries parse through them — the ANSI backup's `superkeys.map`
//!   carries the older padded wire form (one superkey, then stale
//!   bytes and `65535` padding to 512 tokens) and the ISO backup's
//!   the live-bytes form (7 tokens); both parse to the same single
//!   clipboard superkey, and the restore validation (through the
//!   shared `devices::plan` skeleton) uses these types for
//!   exactly that reason.

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

    /// Error when parsing a Raise keymap from a string slice.
    #[derive(Clone, Debug, Display, From, Error)]
    pub enum ParseKeymapError {
        /// The keymap data is not valid wire data.
        #[display("failed to parse keymap: {_0}")]
        MalformedData(parsing::keymap::ParseKeymapError),
        /// The keymap data is valid wire data but does not hold exactly
        /// the ten layers every Raise keymap holds.
        #[display("{_0}")]
        IncorrectLayerCount(KeymapDoesNotHave10LayersError),
    }

    /// Error returned when building a [`RaiseKeymap`] from anything
    /// but exactly 10 layers (the number every Raise keymap holds and
    /// every `keymap.custom` write carries).
    #[derive(Clone, Copy, Debug, Display, Error)]
    #[display("keymap does not have exactly 10 layers")]
    pub struct KeymapDoesNotHave10LayersError;

    /// Error when parsing a Raise colormap from a string slice.
    #[derive(Clone, Debug, Display, From, Error)]
    pub enum ParseColormapError {
        /// The colormap data is not valid wire data.
        #[display("failed to parse colormap map: {_0}")]
        MalformedData(parsing::colormap::ParseColormapError),
        /// The colormap data is valid wire data but does not hold
        /// exactly the ten layers every Raise colormap holds.
        #[display("{_0}")]
        IncorrectLayerCount(ColormapDoesNotHave10LayersError),
    }

    /// Error returned when building a [`RaiseColormap`] from anything
    /// but exactly 10 layers (the number every Raise colormap holds
    /// and every `colormap.map` write carries).
    #[derive(Clone, Copy, Debug, Display, Error)]
    #[display("colormap does not have exactly 10 layers")]
    pub struct ColormapDoesNotHave10LayersError;

    /// The Display message of a partial-capture refusal, shared with
    /// every product's validation — see
    /// [`parsing::backup::PartialCapture::refusal_display`], the
    /// wording's single source.
    pub(super) fn partial_capture_display(missing: &[String]) -> String {
        parsing::backup::PartialCapture {
            missing: missing.to_vec(),
        }
        .refusal_display()
    }

    /// Error returned when a backup fails Raise restore validation.
    ///
    /// Validation ([`backup_restore_plan`]) runs entirely offline:
    /// a backup either validates whole, or nothing leaves this module.
    /// The Raise sibling of the Defy's
    /// [`BackupValidationError`][defy-validation] — the cross-product
    /// guard mirrored: a backup whose descriptor names another product
    /// is refused here too, so the guard stands in every direction
    /// and conversion remains the explicit pre-step (a Raise backup
    /// destined for a Raise 2 goes through
    /// `convert_backup` first — see
    /// [`convert_backup`][crate::devices::conversions::convert_backup]).
    ///
    /// [defy-validation]:
    ///     crate::devices::defy::BackupValidationError
    #[derive(Clone, Debug, Display, Error)]
    pub enum BackupValidationError {
        /// The backup's device descriptor names a product other than
        /// the Raise. Restoring another product's backup to a Raise
        /// would write foreign data to it (and no Raise transport
        /// exists in this crate yet — see the [module docs](super)).
        #[display("the backup is for a `{product}`, not a Raise")]
        NotARaiseBackup {
            /// The product the backup's device descriptor names.
            product: String,
        },
        /// One of the backup's captured commands carries data this
        /// crate can prove is not valid Raise wire data. The keymap,
        /// superkeys, macros, palette, and colormap entries are each
        /// validated by parsing them against their wire formats (the
        /// map types also enforce the write shapes); captured commands
        /// without a Raise-shaped validator pass through verbatim,
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
    /// captured command this crate can validate against the Raise's
    /// wire shapes.
    #[derive(Clone, Debug, Display, Error)]
    pub enum BackupEntryError {
        /// The `keymap.custom` data is not a valid Raise keymap —
        /// either not valid wire data at all, or not the exactly ten
        /// layers every `keymap.custom` write carries (the wire parser
        /// itself accepts any layer count, so the [`RaiseKeymap`]
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
        /// The `colormap.map` data is not a valid Raise colormap —
        /// either not valid wire data at all, or not the exactly ten
        /// 132-LED layers every Raise `colormap.map` write carries.
        #[display("{_0}")]
        Colormap(ParseColormapError),
    }
}

/// Type alias for the raw Raise keymap layer data.
pub type RaiseLayerData = [KeyKind; KEYS_PER_LAYER];

/// Number of wire positions per Raise keymap layer (the 5×16 wire
/// matrix, the Defy's width). 73 of the 80 positions are physical
/// keys — see [`LAYOUT`] and the [module docs](self) for the seven
/// unmodeled slots.
pub const KEYS_PER_LAYER: usize = 80;

/// Number of layers in a Raise `keymap.custom` command.
///
/// This is the exact layer count of every Raise keymap: the device
/// stores and reports exactly this many, and [`RaiseKeymap`] can only
/// hold this many.
pub const KEYMAP_CUSTOM_COMMAND_LAYERS: usize = 10;

/// Number of LEDs per Raise colormap layer: 69 keyboard LEDs followed
/// by the 30-and-33 split underglow (see the [module docs](super)).
pub const LEDS_PER_LAYER: usize = KEYBOARD_LEDS + UNDERGLOW_LEFT + UNDERGLOW_RIGHT;

/// How many of one layer's LED positions are keyboard LEDs (positions
/// 0–68 of the layer).
pub const KEYBOARD_LEDS: usize = 69;

/// How many of one layer's LED positions are the left underglow
/// (positions 69–98 of the layer).
pub const UNDERGLOW_LEFT: usize = 30;

/// How many of one layer's LED positions are the right underglow
/// (positions 99–131 of the layer).
pub const UNDERGLOW_RIGHT: usize = 33;

/// Structure representing the physical layout of the Raise keyboard:
/// two halves, each four key rows and an 8-key thumb row.
///
/// Every entry is a wire position of the layer's 80; the seven
/// positions the layout never names (7–8, 23, 39–40, 55–56 — the
/// four key rows' matrix gaps) are the
/// unmodeled slots — see the [module docs](super).
#[derive(Clone, Copy, Debug)]
pub struct RaiseLayout {
    /// The left half.
    pub left: RaiseLayoutLeft,
    /// The right half.
    pub right: RaiseLayoutRight,
}

/// The left half of the Raise keyboard's physical layout: four 7-key
/// rows and an 8-key thumb row.
#[derive(Clone, Copy, Debug)]
pub struct RaiseLayoutLeft {
    /// Row 1 key indices.
    pub row_1: [u8; 7],
    /// Row 2 key indices.
    pub row_2: [u8; 7],
    /// Row 3 key indices.
    pub row_3: [u8; 7],
    /// Row 4 key indices.
    pub row_4: [u8; 7],
    /// The thumb row's 8 key indices.
    pub thumb_row: [u8; 8],
}

/// The right half of the Raise keyboard's physical layout: an 8-key
/// second row, three 7-key rows, and an 8-key thumb row — one more
/// key in the second row than the left half carries, per the device
/// descriptor's own rows (7/8/7/7/8 against the left's 7/7/7/7/8).
#[derive(Clone, Copy, Debug)]
pub struct RaiseLayoutRight {
    /// Row 1 key indices.
    pub row_1: [u8; 7],
    /// Row 2 key indices (eight).
    pub row_2: [u8; 8],
    /// Row 3 key indices.
    pub row_3: [u8; 7],
    /// Row 4 key indices.
    pub row_4: [u8; 7],
    /// The thumb row's 8 key indices.
    pub thumb_row: [u8; 8],
}

/// Constant providing the Raise keymap layout.
///
/// The device descriptors' `keyboard.left`/`keyboard.right` rows,
/// cross-checked against the shipped backups' layer data and
/// Bazecor's keymap editor — see the [module docs](super) for the
/// evidence.
pub const LAYOUT: &RaiseLayout = &RaiseLayout {
    left: RaiseLayoutLeft {
        row_1: [0, 1, 2, 3, 4, 5, 6],
        row_2: [16, 17, 18, 19, 20, 21, 22],
        row_3: [32, 33, 34, 35, 36, 37, 38],
        row_4: [48, 49, 50, 51, 52, 53, 54],
        thumb_row: [64, 65, 66, 67, 68, 69, 70, 71],
    },
    right: RaiseLayoutRight {
        row_1: [9, 10, 11, 12, 13, 14, 15],
        row_2: [24, 25, 26, 27, 28, 29, 30, 31],
        row_3: [41, 42, 43, 44, 45, 46, 47],
        row_4: [57, 58, 59, 60, 61, 62, 63],
        thumb_row: [72, 73, 74, 75, 76, 77, 78, 79],
    },
};

/// Full Raise keymap.
///
/// A Raise keymap holds exactly [`KEYMAP_CUSTOM_COMMAND_LAYERS`]
/// layers — the device stores exactly that many, and every
/// `keymap.custom` write carries exactly that many — so a
/// [`RaiseKeymap`] cannot hold a different number of layers by
/// construction. Building one from a count that is not known at
/// compile time (wire data, a JSON file) is fallible: it goes through
/// [`TryFrom<Vec<RaiseKeymapLayer>>`](TryFrom) and fails with
/// [`KeymapDoesNotHave10LayersError`] when the count is wrong.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Deref, DerefMut, From)]
pub struct RaiseKeymap {
    /// The keymap's ten layers, in wire order (layer 1 first).
    pub layers: [RaiseKeymapLayer; KEYMAP_CUSTOM_COMMAND_LAYERS],
}

impl TryFrom<Vec<RaiseKeymapLayer>> for RaiseKeymap {
    type Error = KeymapDoesNotHave10LayersError;

    fn try_from(layers: Vec<RaiseKeymapLayer>) -> Result<Self, Self::Error> {
        let layers = layers
            .try_into()
            .map_err(|_| KeymapDoesNotHave10LayersError)?;

        Ok(Self { layers })
    }
}

impl FromStr for RaiseKeymap {
    type Err = ParseKeymapError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let layers: Vec<RaiseKeymapLayer> = s
            .parse::<parsing::keymap::Keymap<{ KEYS_PER_LAYER }>>()?
            .iter()
            .map(RaiseKeymapLayer::from)
            .collect();

        Ok(Self::try_from(layers)?)
    }
}

impl serde::Serialize for RaiseKeymap {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.layers
            .iter()
            .copied()
            .enumerate()
            .map(|(i, layer)| RaiseKeymapLayer {
                layer_number: i as u8 + 1,
                ..layer
            })
            .collect::<Vec<_>>()
            .serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for RaiseKeymap {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let layers = <Vec<RaiseKeymapLayer> as serde::Deserialize>::deserialize(deserializer)?;

        // Checked at the JSON boundary like `FromStr` checks it at the
        // wire boundary: a config file holding a different number of
        // layers cannot come into existence as a [`RaiseKeymap`]
        // either, so every consumer of the type can rely on the write
        // shape.
        Self::try_from(layers).map_err(serde::de::Error::custom)
    }
}

impl RaiseKeymap {
    /// Converts this keymap into a form suitable for sending over to
    /// the keyboard as the data of a `keymap.custom` command.
    ///
    /// Infallible by construction: a [`RaiseKeymap`] holds exactly
    /// the ten layers the command requires. The seven unmodeled wire
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

/// A single human-readable Raise layer.
#[derive(Clone, Copy, Debug, Eq, Serialize, Deserialize)]
pub struct RaiseKeymapLayer {
    /// A human-readable label for knowing what layer you're editing in
    /// the JSON file.
    ///
    /// **Note**: This number is only a UX aid, and is not taken
    /// into account when deserializing from the config file.
    #[serde(skip_deserializing)]
    pub layer_number: u8,
    /// Left half of the keyboard.
    pub left: RaiseKeymapLeft,
    /// Right half of the keyboard.
    pub right: RaiseKeymapRight,
}

impl From<&RaiseLayerData> for RaiseKeymapLayer {
    fn from(layer_data: &RaiseLayerData) -> Self {
        Self {
            layer_number: 0,
            left: RaiseKeymapLeft::from(layer_data),
            right: RaiseKeymapRight::from(layer_data),
        }
    }
}

impl PartialEq for RaiseKeymapLayer {
    fn eq(&self, other: &Self) -> bool {
        self.left == other.left && self.right == other.right
    }
}

impl std::hash::Hash for RaiseKeymapLayer {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.left.hash(state);
        self.right.hash(state);
    }
}

impl RaiseKeymapLayer {
    /// Get's the corresponding key given the key offset.
    ///
    /// The key offset is an unsigned integer in the range `0..80`.
    /// Please refer to the [`LAYOUT`] constant for getting
    /// the key offset of a specific key.
    ///
    /// Note that keys are in `0..80`, but the keyboard only has
    /// 73 keys, therefore, some indices will return `None`, even
    /// though a keymap will contain a key code. In these cases, you
    /// can use either `u16::MIN` or `u16::MAX`, as it is only a padded
    /// placeholder.
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
                (thumb_row),
            }
        };

        let right = get_index! {
            right: {
                (row_1),
                (row_2),
                (row_3),
                (row_4),
                (thumb_row),
            }
        };

        left.or(right)
    }

    /// Converts this layer into a form suitable for using with keymap
    /// commands.
    ///
    /// **Note**: This function returns [`Option<KeyKind>`], rather
    /// than [`KeyKind`]. This is done because a keymap layer must
    /// contain 80 keys, but the keyboard only has 73 keys. Therefore,
    /// there are 7 missing keys (the four key rows' matrix gaps,
    /// wire positions 7–8, 23, 39–40, 55–56).
    /// You should therefore pick a placeholder key, usually `u16::MIN`
    /// or `u16::MAX`.
    pub fn to_keymap_data(&self) -> [Option<KeyKind>; KEYS_PER_LAYER] {
        array::from_fn(|i| self.get_key_by_index(i as u8))
    }

    /// Clears the entire layer to the provided key.
    pub fn new_cleared_to(key: KeyKind) -> Self {
        let keys: RaiseLayerData = [key; KEYS_PER_LAYER];

        RaiseKeymapLayer::from(&keys)
    }
}

/// The Raise keyboard's left half: four 7-key rows and an 8-key thumb
/// row (wire positions 0–6, 16–22, 32–38, 48–54, 64–71).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RaiseKeymapLeft {
    /// Row 1.
    pub row_1: [KeyKind; 7],
    /// Row 2.
    pub row_2: [KeyKind; 7],
    /// Row 3.
    pub row_3: [KeyKind; 7],
    /// Row 4.
    pub row_4: [KeyKind; 7],
    /// The thumb row's eight keys, from left to right (wire positions
    /// 64–71).
    pub thumb_row: [KeyKind; 8],
}

impl From<&RaiseLayerData> for RaiseKeymapLeft {
    fn from(layer_data: &RaiseLayerData) -> Self {
        let left = &LAYOUT.left;

        Self {
            row_1: left.row_1.map(|index| layer_data[index as usize]),
            row_2: left.row_2.map(|index| layer_data[index as usize]),
            row_3: left.row_3.map(|index| layer_data[index as usize]),
            row_4: left.row_4.map(|index| layer_data[index as usize]),
            thumb_row: left.thumb_row.map(|index| layer_data[index as usize]),
        }
    }
}

/// The Raise keyboard's right half: an 8-key second row, three 7-key
/// rows, and an 8-key thumb row (wire positions 9–15, 24–31, 41–47,
/// 57–63, 72–79 — the right half's second row carries one more key
/// than the left's, per the device descriptor's own rows).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RaiseKeymapRight {
    /// Row 1.
    pub row_1: [KeyKind; 7],
    /// Row 2 (eight keys — the descriptor's right rows are
    /// 7/8/7/7/8 against the left's 7/7/7/7/8).
    pub row_2: [KeyKind; 8],
    /// Row 3.
    pub row_3: [KeyKind; 7],
    /// Row 4.
    pub row_4: [KeyKind; 7],
    /// The thumb row's eight keys, from left to right (wire positions
    /// 72–79).
    pub thumb_row: [KeyKind; 8],
}

impl From<&RaiseLayerData> for RaiseKeymapRight {
    fn from(layer_data: &RaiseLayerData) -> Self {
        let right = &LAYOUT.right;

        Self {
            row_1: right.row_1.map(|index| layer_data[index as usize]),
            row_2: right.row_2.map(|index| layer_data[index as usize]),
            row_3: right.row_3.map(|index| layer_data[index as usize]),
            row_4: right.row_4.map(|index| layer_data[index as usize]),
            thumb_row: right.thumb_row.map(|index| layer_data[index as usize]),
        }
    }
}

/// Full Raise colormap: the palette indices painting every LED, on
/// every layer.
///
/// A Raise colormap holds exactly [`KEYMAP_CUSTOM_COMMAND_LAYERS`]
/// layers — the device stores and reports exactly that many, and every
/// `colormap.map` write carries them — so a [`RaiseColormap`] cannot
/// hold a different number of layers by construction. Building one
/// from a count that is not known at compile time (wire data, a JSON
/// file) is fallible: it goes through
/// [`TryFrom<Vec<RaiseColormapLayer>>`](TryFrom) and fails with
/// [`ColormapDoesNotHave10LayersError`] when the count is wrong.
///
/// Each layer splits at the hardware's LED boundaries: the first
/// [`KEYBOARD_LEDS`] positions paint the keyboard's keys, then
/// [`UNDERGLOW_LEFT`] the left underglow and [`UNDERGLOW_RIGHT`] the
/// right one.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Deref, DerefMut, From)]
pub struct RaiseColormap {
    /// The colormap's ten layers, in wire order (layer 1 first).
    pub layers: [RaiseColormapLayer; KEYMAP_CUSTOM_COMMAND_LAYERS],
}

colormap_map_impls! {
    RaiseColormap,
    RaiseColormapLayer,
    parsing::colormap::Colormap<{ LEDS_PER_LAYER }>,
    layer_count_error: ColormapDoesNotHave10LayersError,
    parse_error: ParseColormapError,
}

/// A single human-readable Raise colormap layer.
///
/// Serialized as a JSON object of four fields; `layer_number` is a
/// write-only UX aid like the keymap layers' (a file carrying one has
/// it ignored on the way in), and the three LED arrays are
/// length-checked on the way in — exactly 69 keyboard, 30
/// left-underglow, and 33 right-underglow indices. The serde impls
/// come from the shared `colormap_layer_serde!` macro (69 lands past
/// the derive's array support).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RaiseColormapLayer {
    /// A human-readable label for knowing what layer you're editing in
    /// the JSON file.
    ///
    /// **Note**: This number is only a UX aid, and is not taken
    /// into account when deserializing from the config file.
    pub layer_number: u8,
    /// The keyboard LEDs' palette indices, in wire order (positions
    /// 0–68 of the layer).
    pub keyboard: [parsing::colormap::PaletteIndex; KEYBOARD_LEDS],
    /// The left underglow LEDs' palette indices, in wire order
    /// (positions 69–98 of the layer).
    pub underglow_left: [parsing::colormap::PaletteIndex; UNDERGLOW_LEFT],
    /// The right underglow LEDs' palette indices, in wire order
    /// (positions 99–131 of the layer).
    pub underglow_right: [parsing::colormap::PaletteIndex; UNDERGLOW_RIGHT],
}

colormap_layer_serde! {
    RaiseColormapLayer,
    keyboard => Keyboard [KEYBOARD_LEDS] "keyboard",
    underglow_left => UnderglowLeft [UNDERGLOW_LEFT] "left-underglow",
    underglow_right => UnderglowRight [UNDERGLOW_RIGHT] "right-underglow",
}

impl From<&parsing::colormap::ColormapLayer<{ LEDS_PER_LAYER }>> for RaiseColormapLayer {
    fn from(layer: &parsing::colormap::ColormapLayer<{ LEDS_PER_LAYER }>) -> Self {
        let leds = layer.0;

        let (keyboard, rest) = leds.split_at(KEYBOARD_LEDS);
        let (underglow_left, underglow_right) = rest.split_at(UNDERGLOW_LEFT);

        Self {
            layer_number: 0,
            keyboard: keyboard.try_into().expect(
                "the wire layer's keyboard span is exactly KEYBOARD_LEDS by the split above",
            ),
            underglow_left: underglow_left.try_into().expect(
                "the wire layer's left-underglow span is exactly UNDERGLOW_LEFT by the split above",
            ),
            underglow_right: underglow_right.try_into().expect(
                "the wire layer's right-underglow span is exactly UNDERGLOW_RIGHT by the split above",
            ),
        }
    }
}

impl From<&RaiseColormapLayer> for parsing::colormap::ColormapLayer<{ LEDS_PER_LAYER }> {
    fn from(layer: &RaiseColormapLayer) -> Self {
        let leds: [parsing::colormap::PaletteIndex; LEDS_PER_LAYER] = layer
            .keyboard
            .into_iter()
            .chain(layer.underglow_left)
            .chain(layer.underglow_right)
            .collect::<Vec<_>>()
            .try_into()
            .expect("the three spans chain to exactly LEDS_PER_LAYER by construction");

        Self(leds)
    }
}

/// Computes the exact command sequence a Raise restore would send for
/// a backup, validating the backup first — **without any connection**
/// (no Raise transport exists; see the [module docs](super) for what
/// that means).
///
/// The Raise sibling of
/// [`DefyKeyboard::backup_restore_plan`][defy-plan]: the same
/// product-dispatched plan semantics (the order is
/// [`restore_command_sequence`][seq]'s, product-agnostic; the `led.mode 0`
/// tail Bazecor appends is added the same way), with every
/// product-shaped entry validated through the Raise types — an
/// 800-token `keymap.custom` through [`RaiseKeymap`], a 1320-token
/// `colormap.map` through [`RaiseColormap`], a 48-decimal `palette`
/// through
/// [`RgbPalette`][crate::focus_api::parsing::palette::RgbPalette] —
/// and the product-independent maps through the shared
/// [`SuperkeyMap`][crate::devices::defy::SuperkeyMap]/
/// [`MacroMap`][crate::devices::defy::MacroMap] types. The backup's
/// device descriptor, when it carries one, must name the Raise: the
/// cross-product guard stands in this direction too, so conversion
/// remains the explicit pre-step between products.
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
/// `led.mode 0` tail — and [`RaiseShapes`] names the Raise's own
/// pieces: the product name, the refusals' wording, and the
/// Raise-shaped entries.
///
/// [core]: plan::restore_plan
fn backup_restore_plan_with(
    backup: &parsing::backup::DefyBackup,
    allow_partial: bool,
) -> Result<Vec<parsing::backup::BackupCommand>, BackupValidationError> {
    super::plan::restore_plan::<RaiseShapes>(backup, allow_partial)
}

/// The Raise's restore-validation shapes: the pieces
/// [`plan::restore_plan`](super::plan::restore_plan) wires together —
/// the product name the guard matches, the refusals' wording, and the
/// Raise-shaped entries (the keymap, palette, and colormap; the
/// superkeys and macros maps are the skeleton's).
struct RaiseShapes;

impl super::plan::ProductRestoreShapes for RaiseShapes {
    type ValidationError = BackupValidationError;
    type EntryError = BackupEntryError;
    const EXPECTED_PRODUCT: &'static str = "Raise";

    fn wrong_product(product: String) -> BackupValidationError {
        BackupValidationError::NotARaiseBackup { product }
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
            // [`RaiseKeymap`] cannot be constructed from anything
            // else, so the parse here is the whole validation.
            KEYMAP_CUSTOM_COMMAND_NAME => Some(
                RaiseKeymap::from_str(data)
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
            // A `colormap.map` write carries exactly the ten 132-LED
            // layers; [`RaiseColormap`] cannot be constructed from
            // anything else.
            COLORMAP_MAP_COMMAND_NAME => Some(
                RaiseColormap::from_str(data)
                    .map(|_| ())
                    .map_err(BackupEntryError::Colormap),
            ),
            _ => None,
        }
    }
}

/// The captured Focus commands the Raise restore validation knows —
/// the product-shaped ones. (The product-independent `superkeys.map`
/// and `macros.map` arms live in the shared `devices::plan`
/// skeleton.) Private: the command names are the firmware's and
/// identical across products, and nothing outside this module names
/// them.
const KEYMAP_CUSTOM_COMMAND_NAME: &str = "keymap.custom";
const PALETTE_COMMAND_NAME: &str = "palette";
const COLORMAP_MAP_COMMAND_NAME: &str = "colormap.map";

#[cfg(test)]
mod tests;
