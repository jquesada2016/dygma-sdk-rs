//! Provides the format-level model of the Dygma Raise 2: the
//! human-readable config types (`Raise2Keymap`, `Raise2Colormap`),
//! the physical [`LAYOUT`], and the offline restore-plan validation —
//! everything about the Raise 2 that needs no transport.
//!
//! There is deliberately **no `Raise2Keyboard` device type**: this
//! crate has no Raise 2 connection (no serial/HID transport for the
//! product yet — issue #24's unit 4), and a handle that cannot talk
//! would be a lie. The Raise 2-shaped data this module types is
//! produced and consumed offline: `config init --product raise2`
//! writes it, `backup restore --dry-run` validates a Raise 2 backup
//! against it, and `backup convert` produces it. A real
//! (device-writing) restore of a Raise 2 backup is not possible here —
//! the send path is and stays
//! [`DefyKeyboard::restore_backup`](crate::devices::defy::DefyKeyboard::restore_backup),
//! whose product guard refuses non-Defy backups; the cross-product
//! guard from issue #24 stands untouched.
//!
//! Like the Raise, the Raise 2 comes in two physical layouts, ANSI
//! and ISO — the descriptors' `info.keyboardType`, carried through
//! this crate as
//! [`KeyboardType`][crate::devices::conversions::KeyboardType]. The
//! layouts share every wire shape in this module; what differs is
//! which key codes and LED indices the factory default sits at, so
//! the layouts select *data* (the defaults), not *types*.
//!
//! ## Geometry (the evidence map)
//!
//! - **Keymap**: 10 layers × 80 wire positions — the same 5×16 wire
//!   matrix as the Defy and the Raise. The keyboard rows are
//!   identical to the Raise's in every artifact family (see the
//!   Raise module's evidence map): 73 modeled keys, seven unmodeled
//!   slots at wire positions 7–8, 23, 39–40, 55–56.
//! - **Colormap**: 10 layers × 176 LEDs — 69 keyboard LEDs
//!   (positions 0–68) followed by the underglow, which Bazecor's own
//!   Raise converter splits 53 left (69–121) + 54 right (122–175)
//!   (`raiseToRaise2.ts`'s fixed 176-target interpolation spans).
//!   One decimal per LED, the same wire form as every product's map.
//! - **Palette**: 16 colors × 4 RGBW channels — `RGBWMode: true`
//!   (the virtual descriptors carry it explicitly), the Defy's wire
//!   shape ([`Palette`][crate::focus_api::parsing::palette::Palette]).
//! - **USB identity**: vendor 13807, product 33 (ANSI) / 35 (ISO) in
//!   the `{device, virtual}` records Bazecor ships under
//!   `src/defaultBackups/`. Recorded, NOT replicated: the
//!   hardware-virtual *definitions* the app actually serves
//!   (`src/api/hardware-virtual/Raise2{ANSI,ISO}.ts`) spell product
//!   id 33 for BOTH layouts (0x0021) — the two artifact families
//!   disagree on the ISO product id, and this crate's descriptor
//!   transcription (the conversions module) follows the shipped
//!   records.
//! - **The two artifact families**: no real-device Raise 2 capture
//!   exists — the only Raise 2 artifacts in Bazecor are the
//!   hardware-virtual keyboards, in two copies that mostly agree but
//!   carry four recorded ISO-side discrepancies:
//!   1. the `{device, virtual}` ISO record's `colormap.map` holds
//!      1440 tokens (10 × 144) against its own
//!      5×29 = 145-LED `keyboardUnderglow` geometry AND against the
//!      176-LED layer both ANSI artifacts and the ISO *definition*
//!      carry — a recording artifact, misaligned layer by layer
//!      against the consistent data (the conversions unit records it
//!      as an OUTLIER expectation and the typed colormap here
//!      rejects its shape);
//!   2. the record's ISO `keymap.custom` drifts from the
//!      definition's at 28 positions of layer 3 — every difference
//!      a placeholder swap (`0` in one copy, `65535` transparent in
//!      the other; no real key differs; pinned exactly by the
//!      defaults tests);
//!   3. that record's underglow geometry is 5×29 with LED splits
//!      unlike the definition's 4×44 (which both Raise 2 layouts'
//!      colormaps are built on);
//!   4. the ISO product id (35 there, 33 in the definition) — above.
//!
//!   The definition pair (what the app's virtual-keyboard mode
//!   serves) is internally consistent for both layouts, and its ANSI
//!   copy agrees token for token with the shipped `{device, virtual}`
//!   ANSI record on every config command (pinned by the defaults
//!   tests).
//! - **Wireless**: the virtual descriptors carry `wireless: true`,
//!   and the Raise 2 virtual keyboards' wireless command responses
//!   agree value-for-value with the Defy wireless virtual's (the
//!   WS-D audit behind issue #24's "the Raise 2 wireless map is
//!   Defy-identical"). Transport for any of it is unit 4's work.
//! - **Superkeys and macros** are product-independent wire formats:
//!   the Defy module's
//!   [`SuperkeyMap`][crate::devices::defy::SuperkeyMap] (512-slot
//!   memory, 128-superkey firmware addressing limit) and
//!   [`MacroMap`][crate::devices::defy::MacroMap] (128-macro limit)
//!   apply to the Raise 2 unchanged (see the Raise module's evidence
//!   map for the wire forms the sources carry).

use crate::{
    devices::colormap_serde::colormap_layer_serde,
    devices::defy::{ParseMacroMapError, ParseSuperkeyMapError},
    focus_api::parsing,
    keycode_tables::KeyKind,
};
pub use error::*;
use std::{array, str::FromStr};

mod error {
    use super::*;

    /// Error when parsing a Raise 2 keymap from a string slice.
    #[derive(Clone, Debug, Display, From, Error)]
    pub enum ParseKeymapError {
        /// The keymap data is not valid wire data.
        #[display("failed to parse keymap: {_0}")]
        MalformedData(parsing::keymap::ParseKeymapError),
        /// The keymap data is valid wire data but does not hold exactly
        /// the ten layers every Raise 2 keymap holds.
        #[display("{_0}")]
        IncorrectLayerCount(KeymapDoesNotHave10LayersError),
    }

    /// Error returned when building a [`Raise2Keymap`] from anything
    /// but exactly 10 layers (the number every Raise 2 keymap holds
    /// and every `keymap.custom` write carries).
    #[derive(Clone, Copy, Debug, Display, Error)]
    #[display("keymap does not have exactly 10 layers")]
    pub struct KeymapDoesNotHave10LayersError;

    /// Error when parsing a Raise 2 colormap from a string slice.
    #[derive(Clone, Debug, Display, From, Error)]
    pub enum ParseColormapError {
        /// The colormap data is not valid wire data.
        #[display("failed to parse colormap map: {_0}")]
        MalformedData(parsing::colormap::ParseColormapError),
        /// The colormap data is valid wire data but does not hold
        /// exactly the ten layers every Raise 2 colormap holds.
        #[display("{_0}")]
        IncorrectLayerCount(ColormapDoesNotHave10LayersError),
    }

    /// Error returned when building a [`Raise2Colormap`] from anything
    /// but exactly 10 layers (the number every Raise 2 colormap holds
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

    /// Error returned when a backup fails Raise 2 restore validation.
    ///
    /// Validation ([`backup_restore_plan`]) runs entirely offline:
    /// a backup either validates whole, or nothing leaves this module.
    /// The Raise 2 sibling of the Defy's
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
        /// the Raise 2. Restoring another product's backup to a
        /// Raise 2 would write foreign data to it (and no Raise 2
        /// transport exists in this crate yet — see the [module
        /// docs](super)).
        #[display("the backup is for a `{product}`, not a Raise 2")]
        NotARaise2Backup {
            /// The product the backup's device descriptor names.
            product: String,
        },
        /// One of the backup's captured commands carries data this
        /// crate can prove is not valid Raise 2 wire data. The keymap,
        /// superkeys, macros, palette, and colormap entries are each
        /// validated by parsing them against their wire formats (the
        /// map types also enforce the write shapes); captured commands
        /// without a Raise 2-shaped validator pass through verbatim,
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
    /// captured command this crate can validate against the Raise 2's
    /// wire shapes.
    #[derive(Clone, Debug, Display, Error)]
    pub enum BackupEntryError {
        /// The `keymap.custom` data is not a valid Raise 2 keymap —
        /// either not valid wire data at all, or not the exactly ten
        /// layers every `keymap.custom` write carries (the wire parser
        /// itself accepts any layer count, so the [`Raise2Keymap`]
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
        /// The `palette` data is not a valid Defy-form palette —
        /// either not valid wire data at all, or not the exactly 64
        /// decimals (16 RGBW colors) a `RGBWMode: true` palette
        /// carries.
        #[display("{_0}")]
        Palette(parsing::palette::ParsePaletteError),
        /// The `colormap.map` data is not a valid Raise 2 colormap —
        /// either not valid wire data at all, or not the exactly ten
        /// 176-LED layers every Raise 2 `colormap.map` write carries.
        #[display("{_0}")]
        Colormap(ParseColormapError),
    }
}

/// Type alias for the raw Raise 2 keymap layer data.
pub type Raise2LayerData = [KeyKind; KEYS_PER_LAYER];

/// Number of wire positions per Raise 2 keymap layer (the 5×16 wire
/// matrix, the Defy's width). 73 of the 80 positions are physical
/// keys — see [`LAYOUT`] and the [module docs](self) for the seven
/// unmodeled slots.
pub const KEYS_PER_LAYER: usize = 80;

/// Number of layers in a Raise 2 `keymap.custom` command.
///
/// This is the exact layer count of every Raise 2 keymap: the device
/// stores and reports exactly this many, and [`Raise2Keymap`] can
/// only hold this many.
pub const KEYMAP_CUSTOM_COMMAND_LAYERS: usize = 10;

/// Number of LEDs per Raise 2 colormap layer: 69 keyboard LEDs
/// followed by the 53-and-54 split underglow (see the
/// [module docs](self)).
pub const LEDS_PER_LAYER: usize = KEYBOARD_LEDS + UNDERGLOW_LEFT + UNDERGLOW_RIGHT;

/// How many of one layer's LED positions are keyboard LEDs (positions
/// 0–68 of the layer).
pub const KEYBOARD_LEDS: usize = 69;

/// How many of one layer's LED positions are the left underglow
/// (positions 69–121 of the layer).
pub const UNDERGLOW_LEFT: usize = 53;

/// How many of one layer's LED positions are the right underglow
/// (positions 122–175 of the layer).
pub const UNDERGLOW_RIGHT: usize = 54;

/// Structure representing the physical layout of the Raise 2
/// keyboard: two halves, each four key rows and an 8-key thumb row —
/// the Raise's wire rows, which every Raise 2 artifact family shares
/// (see the [module docs](super)).
#[derive(Clone, Copy, Debug)]
pub struct Raise2Layout {
    /// The left half.
    pub left: Raise2LayoutLeft,
    /// The right half.
    pub right: Raise2LayoutRight,
}

/// The left half of the Raise 2 keyboard's physical layout: four
/// 7-key rows and an 8-key thumb row.
#[derive(Clone, Copy, Debug)]
pub struct Raise2LayoutLeft {
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

/// The right half of the Raise 2 keyboard's physical layout: an 8-key
/// second row, three 7-key rows, and an 8-key thumb row — one more
/// key in the second row than the left half carries, per the device
/// descriptor's own rows (7/8/7/7/8 against the left's 7/7/7/7/8).
#[derive(Clone, Copy, Debug)]
pub struct Raise2LayoutRight {
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

/// Constant providing the Raise 2 keymap layout.
///
/// The device descriptors' `keyboard.left`/`keyboard.right` rows —
/// identical to the Raise's in every artifact family, ANSI and ISO
/// alike — which the conversions unit transcribed verbatim and the
/// defaults tests pin.
pub const LAYOUT: &Raise2Layout = &Raise2Layout {
    left: Raise2LayoutLeft {
        row_1: [0, 1, 2, 3, 4, 5, 6],
        row_2: [16, 17, 18, 19, 20, 21, 22],
        row_3: [32, 33, 34, 35, 36, 37, 38],
        row_4: [48, 49, 50, 51, 52, 53, 54],
        thumb_row: [64, 65, 66, 67, 68, 69, 70, 71],
    },
    right: Raise2LayoutRight {
        row_1: [9, 10, 11, 12, 13, 14, 15],
        row_2: [24, 25, 26, 27, 28, 29, 30, 31],
        row_3: [41, 42, 43, 44, 45, 46, 47],
        row_4: [57, 58, 59, 60, 61, 62, 63],
        thumb_row: [72, 73, 74, 75, 76, 77, 78, 79],
    },
};

/// Full Raise 2 keymap.
///
/// A Raise 2 keymap holds exactly [`KEYMAP_CUSTOM_COMMAND_LAYERS`]
/// layers — the device stores exactly that many, and every
/// `keymap.custom` write carries exactly that many — so a
/// [`Raise2Keymap`] cannot hold a different number of layers by
/// construction. Building one from a count that is not known at
/// compile time (wire data, a JSON file) is fallible: it goes through
/// [`TryFrom<Vec<Raise2KeymapLayer>>`](TryFrom) and fails with
/// [`KeymapDoesNotHave10LayersError`] when the count is wrong.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Deref, DerefMut, From)]
pub struct Raise2Keymap {
    /// The keymap's ten layers, in wire order (layer 1 first).
    pub layers: [Raise2KeymapLayer; KEYMAP_CUSTOM_COMMAND_LAYERS],
}

impl TryFrom<Vec<Raise2KeymapLayer>> for Raise2Keymap {
    type Error = KeymapDoesNotHave10LayersError;

    fn try_from(layers: Vec<Raise2KeymapLayer>) -> Result<Self, Self::Error> {
        let layers = layers
            .try_into()
            .map_err(|_| KeymapDoesNotHave10LayersError)?;

        Ok(Self { layers })
    }
}

impl FromStr for Raise2Keymap {
    type Err = ParseKeymapError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let layers: Vec<Raise2KeymapLayer> = s
            .parse::<parsing::keymap::Keymap<{ KEYS_PER_LAYER }>>()?
            .iter()
            .map(Raise2KeymapLayer::from)
            .collect();

        Ok(Self::try_from(layers)?)
    }
}

impl serde::Serialize for Raise2Keymap {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.layers
            .iter()
            .copied()
            .enumerate()
            .map(|(i, layer)| Raise2KeymapLayer {
                layer_number: i as u8 + 1,
                ..layer
            })
            .collect::<Vec<_>>()
            .serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for Raise2Keymap {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let layers = <Vec<Raise2KeymapLayer> as serde::Deserialize>::deserialize(deserializer)?;

        // Checked at the JSON boundary like `FromStr` checks it at the
        // wire boundary: a config file holding a different number of
        // layers cannot come into existence as a [`Raise2Keymap`]
        // either, so every consumer of the type can rely on the write
        // shape.
        Self::try_from(layers).map_err(serde::de::Error::custom)
    }
}

impl Raise2Keymap {
    /// Converts this keymap into a form suitable for sending over to
    /// the keyboard as the data of a `keymap.custom` command.
    ///
    /// Infallible by construction: a [`Raise2Keymap`] holds exactly
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

/// A single human-readable Raise 2 layer.
#[derive(Clone, Copy, Debug, Eq, Serialize, Deserialize)]
pub struct Raise2KeymapLayer {
    /// A human-readable label for knowing what layer you're editing in
    /// the JSON file.
    ///
    /// **Note**: This number is only a UX aid, and is not taken
    /// into account when deserializing from the config file.
    #[serde(skip_deserializing)]
    pub layer_number: u8,
    /// Left half of the keyboard.
    pub left: Raise2KeymapLeft,
    /// Right half of the keyboard.
    pub right: Raise2KeymapRight,
}

impl From<&Raise2LayerData> for Raise2KeymapLayer {
    fn from(layer_data: &Raise2LayerData) -> Self {
        Self {
            layer_number: 0,
            left: Raise2KeymapLeft::from(layer_data),
            right: Raise2KeymapRight::from(layer_data),
        }
    }
}

impl PartialEq for Raise2KeymapLayer {
    fn eq(&self, other: &Self) -> bool {
        self.left == other.left && self.right == other.right
    }
}

impl std::hash::Hash for Raise2KeymapLayer {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.left.hash(state);
        self.right.hash(state);
    }
}

impl Raise2KeymapLayer {
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
        let keys: Raise2LayerData = [key; KEYS_PER_LAYER];

        Raise2KeymapLayer::from(&keys)
    }
}

/// The Raise 2 keyboard's left half: four 7-key rows and an 8-key
/// thumb row (wire positions 0–6, 16–22, 32–38, 48–54, 64–71).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Raise2KeymapLeft {
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

impl From<&Raise2LayerData> for Raise2KeymapLeft {
    fn from(layer_data: &Raise2LayerData) -> Self {
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

/// The Raise 2 keyboard's right half: an 8-key second row, three
/// 7-key rows, and an 8-key thumb row (wire positions 9–15, 24–31,
/// 41–47, 57–63, 72–79 — the right half's second row carries one
/// more key than the left's, per the device descriptor's own rows).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Raise2KeymapRight {
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

impl From<&Raise2LayerData> for Raise2KeymapRight {
    fn from(layer_data: &Raise2LayerData) -> Self {
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

/// Full Raise 2 colormap: the palette indices painting every LED, on
/// every layer.
///
/// A Raise 2 colormap holds exactly [`KEYMAP_CUSTOM_COMMAND_LAYERS`]
/// layers — the device stores and reports exactly that many, and every
/// `colormap.map` write carries them — so a [`Raise2Colormap`] cannot
/// hold a different number of layers by construction. Building one
/// from a count that is not known at compile time (wire data, a JSON
/// file) is fallible: it goes through
/// [`TryFrom<Vec<Raise2ColormapLayer>>`](TryFrom) and fails with
/// [`ColormapDoesNotHave10LayersError`] when the count is wrong.
///
/// Each layer splits at the hardware's LED boundaries: the first
/// [`KEYBOARD_LEDS`] positions paint the keyboard's keys, then
/// [`UNDERGLOW_LEFT`] the left underglow and [`UNDERGLOW_RIGHT`] the
/// right one.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Deref, DerefMut, From)]
pub struct Raise2Colormap {
    /// The colormap's ten layers, in wire order (layer 1 first).
    pub layers: [Raise2ColormapLayer; KEYMAP_CUSTOM_COMMAND_LAYERS],
}

impl TryFrom<Vec<Raise2ColormapLayer>> for Raise2Colormap {
    type Error = ColormapDoesNotHave10LayersError;

    fn try_from(layers: Vec<Raise2ColormapLayer>) -> Result<Self, Self::Error> {
        let layers = layers
            .try_into()
            .map_err(|_| ColormapDoesNotHave10LayersError)?;

        Ok(Self { layers })
    }
}

impl FromStr for Raise2Colormap {
    type Err = ParseColormapError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let layers: Vec<Raise2ColormapLayer> = s
            .parse::<parsing::colormap::Colormap<{ LEDS_PER_LAYER }>>()?
            .iter()
            .map(Raise2ColormapLayer::from)
            .collect();

        Ok(Self::try_from(layers)?)
    }
}

impl serde::Serialize for Raise2Colormap {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.layers
            .iter()
            .copied()
            .enumerate()
            .map(|(i, layer)| Raise2ColormapLayer {
                layer_number: i as u8 + 1,
                ..layer
            })
            .collect::<Vec<_>>()
            .serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for Raise2Colormap {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let layers = <Vec<Raise2ColormapLayer> as serde::Deserialize>::deserialize(deserializer)?;

        // Checked at the JSON boundary like `FromStr` checks it at the
        // wire boundary: a config file holding a different number of
        // layers cannot come into existence as a [`Raise2Colormap`]
        // either, so every consumer of the type can rely on the write
        // shape.
        Self::try_from(layers).map_err(serde::de::Error::custom)
    }
}

impl Raise2Colormap {
    /// Converts this colormap into a form suitable for sending over to
    /// the keyboard as the data of a `colormap.map` command.
    ///
    /// Infallible by construction: a [`Raise2Colormap`] holds exactly
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

/// A single human-readable Raise 2 colormap layer.
///
/// Serialized as a JSON object of four fields; `layer_number` is a
/// write-only UX aid like the keymap layers' (a file carrying one has
/// it ignored on the way in), and the three LED arrays are
/// length-checked on the way in — exactly 69 keyboard, 53
/// left-underglow, and 54 right-underglow indices. The serde impls
/// come from the shared `colormap_layer_serde!` macro (69 lands past
/// the derive's array support).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Raise2ColormapLayer {
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
    /// (positions 69–121 of the layer).
    pub underglow_left: [parsing::colormap::PaletteIndex; UNDERGLOW_LEFT],
    /// The right underglow LEDs' palette indices, in wire order
    /// (positions 122–175 of the layer).
    pub underglow_right: [parsing::colormap::PaletteIndex; UNDERGLOW_RIGHT],
}

colormap_layer_serde! {
    Raise2ColormapLayer,
    keyboard => Keyboard [KEYBOARD_LEDS] "keyboard",
    underglow_left => UnderglowLeft [UNDERGLOW_LEFT] "left-underglow",
    underglow_right => UnderglowRight [UNDERGLOW_RIGHT] "right-underglow",
}

impl From<&parsing::colormap::ColormapLayer<{ LEDS_PER_LAYER }>> for Raise2ColormapLayer {
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

impl From<&Raise2ColormapLayer> for parsing::colormap::ColormapLayer<{ LEDS_PER_LAYER }> {
    fn from(layer: &Raise2ColormapLayer) -> Self {
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

/// Computes the exact command sequence a Raise 2 restore would send
/// for a backup, validating the backup first — **without any
/// connection** (no Raise 2 transport exists; see the
/// [module docs](self) for what that means).
///
/// The Raise 2 sibling of
/// [`DefyKeyboard::backup_restore_plan`][defy-plan]: the same
/// product-dispatched plan semantics (the order is
/// [`restore_command_sequence`][seq]'s, product-agnostic; the `led.mode 0`
/// tail Bazecor appends is added the same way), with every
/// product-shaped entry validated through the Raise 2 types — an
/// 800-token `keymap.custom` through [`Raise2Keymap`], a 1760-token
/// `colormap.map` through [`Raise2Colormap`], a 64-decimal `palette`
/// through the Defy-form
/// [`Palette`][crate::focus_api::parsing::palette::Palette] — and the
/// product-independent maps through the shared
/// [`SuperkeyMap`][crate::devices::defy::SuperkeyMap]/
/// [`MacroMap`][crate::devices::defy::MacroMap] types. The backup's
/// device descriptor, when it carries one, must name the Raise 2: the
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
/// `led.mode 0` tail — and [`Raise2Shapes`] names the Raise 2's own
/// pieces: the product name, the refusals' wording, and the
/// Raise 2-shaped entries.
///
/// [core]: plan::restore_plan
fn backup_restore_plan_with(
    backup: &parsing::backup::DefyBackup,
    allow_partial: bool,
) -> Result<Vec<parsing::backup::BackupCommand>, BackupValidationError> {
    super::plan::restore_plan::<Raise2Shapes>(backup, allow_partial)
}

/// The Raise 2's restore-validation shapes: the pieces
/// [`plan::restore_plan`](super::plan::restore_plan) wires together —
/// the product name the guard matches, the refusals' wording, and the
/// Raise 2-shaped entries (the keymap, palette, and colormap; the
/// superkeys and macros maps are the skeleton's).
struct Raise2Shapes;

impl super::plan::ProductRestoreShapes for Raise2Shapes {
    type ValidationError = BackupValidationError;
    type EntryError = BackupEntryError;
    const EXPECTED_PRODUCT: &'static str = "Raise2";

    fn wrong_product(product: String) -> BackupValidationError {
        BackupValidationError::NotARaise2Backup { product }
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
            // [`Raise2Keymap`] cannot be constructed from anything
            // else, so the parse here is the whole validation.
            KEYMAP_CUSTOM_COMMAND_NAME => Some(
                Raise2Keymap::from_str(data)
                    .map(|_| ())
                    .map_err(BackupEntryError::Keymap),
            ),
            // A `RGBWMode: true` palette write is exactly 16 RGBW
            // colors; the type cannot be constructed from anything
            // else.
            PALETTE_COMMAND_NAME => Some(
                parsing::palette::Palette::from_str(data)
                    .map(|_| ())
                    .map_err(BackupEntryError::Palette),
            ),
            // A `colormap.map` write carries exactly the ten 176-LED
            // layers; [`Raise2Colormap`] cannot be constructed from
            // anything else.
            COLORMAP_MAP_COMMAND_NAME => Some(
                Raise2Colormap::from_str(data)
                    .map(|_| ())
                    .map_err(BackupEntryError::Colormap),
            ),
            _ => None,
        }
    }
}

/// The captured Focus commands the Raise 2 restore validation knows —
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
