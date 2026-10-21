//! Cross-product backup conversion: rewriting a captured backup between
//! product wire shapes, with an explicit mapping report.
//!
//! A backup captured from one Dygma product cannot be sent to another
//! verbatim: the products differ in keymap geometry (the Defy keys off
//! a 5×16 wire matrix, 80 positions per layer; the Sonsei off a 5×12
//! matrix, 60 positions), in per-layer colormap size (the Defy has 70
//! keyboard LEDs plus 108 underglow LEDs, 178 per layer; the Sonsei has
//! 56 keyboard LEDs and no underglow), and in palette shape (the Defy
//! is RGBW, 4 bytes per color; the Sonsei is RGB, 3 bytes per color).
//! The keycodes themselves are firmware-common Kaleidoscope semantics
//! and transfer 1:1, as do the superkey and macro wire formats — what
//! differs between products is physical layout and format dimensions.
//!
//! The reference behavior is Bazecor's restore-time conversion
//! (`bazecor/src/api/backup/Backup.ts` `restoreBackup` and
//! `restoreVirtual`, with the mapping tables in
//! `bazecor/src/api/parsers/conversions/defyToSonsei.ts`): it converts
//! exactly three captured commands — `keymap.custom`, `colormap.map`,
//! and `palette` — leaves every other command (including
//! `superkeys.map` and `macros.map`) verbatim, and replaces the
//! backup's device descriptor with the target product's. Deviations
//! from Bazecor, all deliberate:
//!
//! - **Sonsei → Defy exists here.** Bazecor converts Defy → Sonsei
//!   only; a Sonsei backup restored onto a Defy is blind-restored,
//!   writing 60-position keymap layers to a device that reads 80 —
//!   this crate's
//!   [`backup_restore_plan`][crate::devices::defy::DefyKeyboard::backup_restore_plan]
//!   rejects such data instead. Conversion is the explicit pre-step
//!   that makes the restore *possible*; the guard stays. The inverse
//!   mapping is the exact inverse of the differentially-validated
//!   Defy → Sonsei table, and the color direction uses Bazecor's own
//!   `rgb2w` (its RGB → RGBW conversion, `bazecor/src/api/color`).
//! - **Invalid inputs fail with typed errors.** Bazecor's conversion
//!   chunks whatever token count the data holds (silently converting
//!   ragged layers) and crashes on a backup missing one of the three
//!   commands (`findIndex` returns `-1`); a duplicated converted
//!   command is silently reduced to its first entry, leaving the
//!   duplicate unconverted in the output. Here the layer shapes are
//!   checked at the boundary, and missing and duplicated commands are
//!   [`ConversionError::MissingCommand`] and
//!   [`ConversionError::DuplicateCommand`].
//! - **The mapping is reported, never guessed.** Every conversion
//!   returns a [`ConversionReport`] saying which target positions took
//!   a source value, which were defaulted (to
//!   [`DefaultValue::Transparent`] for keymaps — Bazecor's own
//!   `NO_KEY` — and [`DefaultValue::FirstPaletteColor`] for colormaps,
//!   its own convention), and which source positions nothing reads.
//!
//! The Raise pairs — Raise → Raise 2 and Raise 2 → Raise — add one
//! more dimension: the physical key layout. A Raise-generation
//! keyboard is built in ANSI and ISO variants whose Enter and left
//! Shift sit at different wire positions, so the keymap conversion
//! branches on the *target* keyboard's layout (an explicit
//! [`KeyboardType`] here; a `--keyboard-type` flag on the CLI), and
//! the Raise 2 → Raise colormap additionally on the *backup's* layout.
//! The transformations themselves are small: a three-position
//! thumb-cluster rotation, the conditional Enter/Shift swaps, an
//! underglow interpolation from the Raise's 132 LEDs per layer to the
//! Raise 2's 176 (and a plain truncation back the other way), and the
//! palette channel conversion — the Raise's wire palette is RGB, the
//! Raise 2's is RGBW — through the same Bazecor color math the Defy
//! and Sonsei pairs use. The Raise conversions also read the backup's
//! own descriptor for things the other pairs never read: the colormap
//! layer width comes from the descriptor's `keyboardUnderglow`
//! geometry (rows × columns), the way Bazecor's Raise conversions size
//! their parse, and the Raise 2 → Raise colormap reads the backup's
//! `keyboardType` as well.
//!
//! Raise-generation deviations from Bazecor, all deliberate:
//!
//! - **An unreadable backup layout fails the Raise 2 → Raise
//!   conversion.** The Raise 2 → Raise colormap's Shift swap is
//!   guarded by the *backup's* layout (`raise2ToRaise.ts`:
//!   `if (keyboardType === "ANSI" && backupKeyboardType === "ISO")`),
//!   and a missing or foreign `keyboardType` fails `=== "ISO"` — so
//!   Bazecor silently applies no Shift swap, assuming the backup was
//!   ANSI. Here a Raise 2 source whose descriptor does not spell
//!   `ANSI` or `ISO` exactly (the shipped spellings) fails with
//!   [`ConversionError::NoKeyboardType`] or
//!   [`ConversionError::UnknownKeyboardType`] — on every Raise 2 →
//!   Raise target, whatever its layout, though only the ANSI target's
//!   colormap reads the value. That uniform validation is itself a
//!   documented deviation: an ISO-target conversion of a garbled-
//!   layout backup is refused where Bazecor would complete.
//!   Correctness over bug-compatibility, and every real Raise/Raise2
//!   file carries the field. (The Raise → Raise 2 direction never
//!   reads the backup's layout — Bazecor's converter ignores its
//!   `backupKeyboardType` argument there — and neither does this
//!   crate.)
//! - **The Raise 2 ISO virtual's ragged colormap is rejected.** The
//!   shipped Raise2 ISO virtual keyboard's `colormap.map` holds 1440
//!   tokens (10 × 144) while its own descriptor geometry is 5 × 29 =
//!   145 per layer — the shipped evidence disagrees with itself.
//!   Bazecor chunks the ragged data and converts ten misaligned
//!   layers; here the exact-count shape check rejects it
//!   ([`ConversionError::InvalidEntry`]) instead of writing garbage
//!   into a keyboard — the same documented-outlier class as the
//!   wireless `channelHop` capture.
//! - **No Raise or Raise 2 hardware exists in this project's
//!   validation.** The Raise pairs are validated differentially only —
//!   against Bazecor's own conversion sources and the shipped backup
//!   and virtual-keyboard artifacts (pinned at Bazecor commit
//!   `ff9e10b` by the harness below); live validation is deferred to
//!   community hardware owners (issue #24's constraint, stated
//!   plainly).
//!
//! A backup converted *to* the Defy re-enters the keymap through
//! [`DefyKeymap`], the exactly-ten-layer type — the output is a valid
//! Defy `keymap.custom` write by construction, and the padding
//! positions normalize to `0`, the value this SDK's keymap serializer
//! has always emitted for padding. Such a
//! backup passes
//! [`DefyKeyboard::backup_restore_plan`] unchanged.
//!
//! The Defy → Sonsei and Raise directions are validated
//! differentially against Bazecor's own JavaScript: a Node harness
//! (`tools/bazecor-differential/`) executes Bazecor's conversion
//! source on the shipped default backups and records its output; the
//! tests here compare this module's output byte for byte.

use crate::{
    devices::defy::{DefyKeyboard, DefyKeymap, DefyKeymapLayer, DefyLayerData},
    focus_api::parsing::backup::DefyBackup,
    keycode_tables::KeyKind,
};
pub use error::*;
use std::array;

mod error {
    use super::*;

    /// Error returned by [`convert_backup`].
    #[derive(Clone, Debug, Display, Error, PartialEq, Eq)]
    pub enum ConversionError {
        /// The backup carries no device descriptor at all, so its
        /// product cannot be determined. (Bazecor's own conversion
        /// crashes on such a backup — it dereferences the descriptor
        /// unconditionally.)
        #[display("the backup carries no device descriptor, so its product cannot be determined")]
        MissingDescriptor,
        /// The backup's device descriptor carries no readable product
        /// name.
        #[display("the backup's device descriptor carries no product name")]
        NoProductName,
        /// The backup's device descriptor names a product this crate
        /// does not know.
        #[display("the backup's device descriptor names the unknown product `{product}`")]
        UnknownProduct {
            /// The product name the descriptor carries.
            product: String,
        },
        /// There is no conversion between the backup's product and the
        /// requested target. The supported pairs are Defy → Sonsei,
        /// Sonsei → Defy, Raise → Raise 2, and Raise 2 → Raise.
        #[display("there is no conversion from {from} to {to}")]
        UnsupportedConversion {
            /// The product the backup was captured from.
            from: Product,
            /// The requested target product.
            to: Product,
        },
        /// The Raise 2 → Raise conversion validates the backup's
        /// physical key layout whatever the target: on an ANSI target
        /// it decides the left-Shift LED swap (colormap 19 ↔ 20); on
        /// an ISO target nothing reads it, but the validation is
        /// uniform — a documented deviation, since an ISO-target
        /// conversion of a backup with a garbled layout is refused
        /// where Bazecor's `backupKeyboardType === "ISO"` check would
        /// silently treat it as ANSI and skip the swap. The backup's
        /// descriptor carries no `keyboardType` to validate.
        #[display(
            "the backup's device descriptor carries no `keyboardType`, which a Raise 2 → Raise conversion validates"
        )]
        NoKeyboardType,
        /// The backup's descriptor carries a `keyboardType` this crate
        /// does not know. Only the exact spellings `ANSI` and `ISO`
        /// — the ones every shipped Raise and Raise 2 descriptor
        /// carries — pass the validation, uniformly whatever the
        /// target (the ANSI-target colormap swap is the only reader);
        /// Bazecor's `backupKeyboardType === "ISO"` check would
        /// silently treat anything else as ANSI, skipping the Shift
        /// swap.
        #[display(
            "the backup's device descriptor carries the unknown `keyboardType` `{keyboard_type}`; a Raise 2 → Raise conversion accepts only `ANSI` or `ISO`"
        )]
        UnknownKeyboardType {
            /// The `keyboardType` value the descriptor carries.
            keyboard_type: String,
        },
        /// A Raise-generation conversion sizes its colormap parse from
        /// the backup descriptor's `keyboardUnderglow` geometry (the
        /// way Bazecor's Raise conversions do), and the descriptor
        /// carries no geometry that resolves to a usable width: the
        /// fields are missing or non-numeric, the product is zero,
        /// overflows a `u64`, or lands orders of magnitude past any
        /// keyboard's colormap layer.
        #[display(
            "the backup's device descriptor carries no usable `keyboardUnderglow` rows × columns geometry, which a Raise or Raise 2 conversion needs to size its colormap"
        )]
        MissingUnderglowGeometry,
        /// A Raise-generation backup's descriptor geometry resolves to
        /// a colormap layer narrower than the conversion's own tables
        /// read, so the descriptor and the captured data cannot both
        /// describe the same keyboard. Bazecor converts such input
        /// anyway — a Raise 2 → Raise layer comes out truncated, and
        /// the Raise → Raise 2 interpolator's `?: 15` default kicks in
        /// past the layer's end; indexing it here would read past the
        /// layer, so this crate refuses.
        #[display(
            "the backup descriptor's `keyboardUnderglow` geometry resolves to {claimed} LEDs per layer, but the conversion reads {needed}"
        )]
        UnderglowGeometryTooNarrow {
            /// The rows × columns product the descriptor claims.
            claimed: usize,
            /// The source LEDs per layer the conversion's colormap
            /// table reads.
            needed: usize,
        },
        /// The backup is already in the requested product's shape —
        /// there is nothing to convert.
        #[display("the backup is already a {product} backup; there is nothing to convert")]
        SameProduct {
            /// The backup's product, which equals the requested target.
            product: Product,
        },
        /// The backup does not carry one of the commands a conversion
        /// rewrites. Every supported conversion needs `keymap.custom`,
        /// `colormap.map`, and `palette` present.
        #[display("the backup carries no `{command}` entry; converting it requires one")]
        MissingCommand {
            /// The command whose entry is missing.
            command: &'static str,
        },
        /// The backup carries more than one captured entry for a
        /// command a conversion rewrites. Bazecor's conversion takes
        /// the first and silently leaves any later duplicate
        /// unconverted in the output — a stale duplicate would
        /// contradict the converted entry — so this crate refuses
        /// instead.
        #[display("the backup carries {count} `{command}` entries; converting needs exactly one")]
        DuplicateCommand {
            /// The command with more than one captured entry.
            command: &'static str,
            /// How many entries carry it.
            count: usize,
        },
        /// One of the converted entries does not hold the source
        /// product's wire shape.
        #[display("the `{command}` entry is not convertible: {source}")]
        InvalidEntry {
            /// The captured command whose data failed the shape check.
            command: &'static str,
            /// The shape failure.
            source: LayeredWireError,
        },
    }

    /// A backup entry's data does not hold exactly `layer_count` layers
    /// of `layer_size` whitespace-separated values in range.
    ///
    /// One type for every converted command — keymap, colormap, and
    /// palette data are all "layers of values", differing only in the
    /// dimensions and the value range.
    #[derive(Clone, Debug, Display, Error, PartialEq, Eq)]
    #[display("expected {layer_count} layers of {layer_size} values, {kind}")]
    pub struct LayeredWireError {
        /// How many values one layer holds (80 keymap positions, 178
        /// colormap LEDs, 4 RGBW channels, …).
        pub layer_size: usize,
        /// How many layers the data must hold (10 for a Defy or Sonsei
        /// keymap and colormap, 16 for a palette's colors).
        pub layer_count: usize,
        /// The largest value a position may carry (65535 for keymap
        /// codes, 255 for color bytes).
        pub max_value: u32,
        /// What exactly is wrong with the data.
        pub kind: LayeredWireErrorKind,
    }

    /// What exactly is wrong with a layered-wire entry's data.
    #[derive(Clone, Debug, Display, Error, PartialEq, Eq)]
    pub enum LayeredWireErrorKind {
        /// A token is not a decimal number.
        #[display("found `{token}` (not a number) at byte offset {offset}")]
        TokenNotANumber {
            /// Byte offset of the offending token within the data.
            offset: usize,
            /// The offending token.
            token: String,
        },
        /// A token is a number but exceeds the value range.
        #[display("found `{token}` at byte offset {offset}; values go up to {max}")]
        TokenOutOfRange {
            /// Byte offset of the offending token within the data.
            offset: usize,
            /// The offending token.
            token: String,
            /// The inclusive maximum.
            max: u32,
        },
        /// The data does not hold exactly `layers × layer_size` values.
        #[display("the data holds {found} values, not {expected}")]
        WrongValueCount {
            /// How many values the data holds.
            found: usize,
            /// How many it must hold (`layers × layer_size`).
            expected: usize,
        },
    }
}

/// A Dygma product a backup can be captured from or converted to.
///
/// Conversions exist between the Defy and the Sonsei (both
/// directions — the Sonsei → Defy direction is this crate's
/// extension) and between the Raise generations; the Raise
/// conversions also take the target keyboard's physical layout, which
/// [`ConversionTarget`] carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Product {
    /// The Dygma Defy (wired or wireless — one wire shape).
    Defy,
    /// The Dygma Sonsei.
    Sonsei,
    /// The Dygma Raise (first generation, ANSI or ISO).
    Raise,
    /// The Dygma Raise 2 (ANSI or ISO).
    Raise2,
}

impl Product {
    /// The product name as Bazecor's device descriptors spell it
    /// (`neuron.device.info.product`).
    pub fn name(self) -> &'static str {
        match self {
            Self::Defy => "Defy",
            Self::Sonsei => "Sonsei",
            Self::Raise => "Raise",
            Self::Raise2 => "Raise2",
        }
    }

    /// Reads the product a backup was captured from, out of its
    /// `neuron.device` descriptor — the same field Bazecor's restore
    /// switches its conversions on.
    pub fn from_descriptor(device: Option<&serde_json::Value>) -> Result<Self, ConversionError> {
        let Some(device) = device else {
            return Err(ConversionError::MissingDescriptor);
        };

        let product = device
            .get("info")
            .and_then(|info| info.get("product"))
            .and_then(serde_json::Value::as_str);

        match product {
            None => Err(ConversionError::NoProductName),
            Some(name) => Self::from_product_name(name),
        }
    }

    /// The product a descriptor's `info.product` name spells, or
    /// [`ConversionError::UnknownProduct`].
    fn from_product_name(name: &str) -> Result<Self, ConversionError> {
        match name {
            "Defy" => Ok(Self::Defy),
            "Sonsei" => Ok(Self::Sonsei),
            "Raise" => Ok(Self::Raise),
            "Raise2" => Ok(Self::Raise2),
            other => Err(ConversionError::UnknownProduct {
                product: other.to_owned(),
            }),
        }
    }
}

impl std::fmt::Display for Product {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// The physical key layout of a Raise-generation keyboard.
///
/// The Raise and Raise 2 are each built in two variants whose Enter
/// and left Shift sit at different wire positions (Bazecor's
/// converters swap wire positions 31 ↔ 47 and 48 ↔ 49 when the target
/// variant is ANSI). The variants are spelled `ANSI` and `ISO` in the
/// device descriptors' `info.keyboardType` — exactly that case, in
/// every shipped Raise and Raise 2 descriptor — so
/// [`KeyboardType::from_descriptor`] accepts only those spellings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyboardType {
    /// The ANSI variant (`keyboardType: "ANSI"`).
    Ansi,
    /// The ISO variant (`keyboardType: "ISO"`).
    Iso,
}

impl KeyboardType {
    /// The layout as the device descriptors spell it.
    pub fn name(self) -> &'static str {
        match self {
            Self::Ansi => "ANSI",
            Self::Iso => "ISO",
        }
    }

    /// Reads the physical layout out of a device descriptor's
    /// `info.keyboardType` — the backup-descriptor read Bazecor's
    /// Raise 2 → Raise conversion makes for its `backupKeyboardType`
    /// argument (the target's layout reaches Bazecor's converters from
    /// the connected device; here it is an explicit [`KeyboardType`],
    /// never a descriptor read). A missing or non-string value is
    /// [`ConversionError::NoKeyboardType`]; a value other than the
    /// exact spellings `ANSI`/`ISO` is
    /// [`ConversionError::UnknownKeyboardType`] — where Bazecor's
    /// `backupKeyboardType === "ISO"` check would silently treat both
    /// as ANSI, skipping the Shift swap.
    pub fn from_descriptor(device: Option<&serde_json::Value>) -> Result<Self, ConversionError> {
        let keyboard_type = device
            .and_then(|device| device.get("info"))
            .and_then(|info| info.get("keyboardType"))
            .and_then(serde_json::Value::as_str);

        match keyboard_type {
            None => Err(ConversionError::NoKeyboardType),
            Some("ANSI") => Ok(Self::Ansi),
            Some("ISO") => Ok(Self::Iso),
            Some(other) => Err(ConversionError::UnknownKeyboardType {
                keyboard_type: other.to_owned(),
            }),
        }
    }
}

impl std::fmt::Display for KeyboardType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// The product — and, for a Raise-generation target, the physical key
/// layout — a conversion builds for.
///
/// A Raise-generation conversion rewrites wire positions by layout,
/// so a Raise target is not fully named by its [`Product`]: the
/// layout is part of the target, carried here so a caller cannot ask
/// for "a Raise conversion" without choosing one. The Defy and the
/// Sonsei have a single layout each.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConversionTarget {
    /// The Dygma Defy.
    Defy,
    /// The Dygma Sonsei.
    Sonsei,
    /// The Dygma Raise, in the given layout.
    Raise(KeyboardType),
    /// The Dygma Raise 2, in the given layout.
    Raise2(KeyboardType),
}

impl ConversionTarget {
    /// The product this target names — what reports and errors are
    /// keyed on (the wire shapes and the descriptors differ by
    /// product; the layout is a conversion-internal dimension).
    pub fn product(self) -> Product {
        match self {
            Self::Defy => Product::Defy,
            Self::Sonsei => Product::Sonsei,
            Self::Raise(_) => Product::Raise,
            Self::Raise2(_) => Product::Raise2,
        }
    }
}

impl std::fmt::Display for ConversionTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Defy => f.write_str("Defy"),
            Self::Sonsei => f.write_str("Sonsei"),
            Self::Raise(keyboard_type) => write!(f, "Raise ({keyboard_type})"),
            Self::Raise2(keyboard_type) => write!(f, "Raise 2 ({keyboard_type})"),
        }
    }
}

/// Keymap wire positions per layer, Defy (5 rows × 16 columns).
pub const DEFY_KEYMAP_LAYER_POSITIONS: usize = 80;

/// Keymap wire columns per row, Defy.
const DEFY_KEYMAP_COLUMNS: usize = 16;

/// Keymap wire positions per layer, Sonsei (5 rows × 12 columns).
pub const SONSEI_KEYMAP_LAYER_POSITIONS: usize = 60;

/// Keymap wire columns per row, Sonsei.
const SONSEI_KEYMAP_COLUMNS: usize = 12;

/// Colormap LED indices per layer, Defy (70 keyboard + 108 underglow).
pub const DEFY_COLOR_LAYER_LEDS: usize = 178;

/// Colormap LED indices per layer, Sonsei (56 keyboard, no underglow).
pub const SONSEI_COLOR_LAYER_LEDS: usize = 56;

/// Keymap wire positions per layer, Raise and Raise 2 (5 rows × 16
/// columns, same wire width as the Defy).
pub const RAISE_KEYMAP_LAYER_POSITIONS: usize = 80;

/// Colormap LED indices per layer, Raise: 69 keyboard LEDs plus a
/// 30-and-33 split underglow (Bazecor's own breakdown in
/// `raiseToRaise2.ts`). The same number Bazecor's Raise conversions
/// fall back to when a backup carries no descriptor geometry.
pub const RAISE_COLOR_LAYER_LEDS: usize =
    RAISE_KEYBOARD_LEDS + RAISE_UNDERGLOW_LEFT + RAISE_UNDERGLOW_RIGHT;

/// Colormap LED indices per layer, Raise 2: 69 keyboard LEDs plus a
/// 53-and-54 split underglow.
pub const RAISE2_COLOR_LAYER_LEDS: usize =
    RAISE_KEYBOARD_LEDS + RAISE2_UNDERGLOW_LEFT + RAISE2_UNDERGLOW_RIGHT;

/// Keyboard (non-underglow) LED indices per Raise-generation colormap
/// layer — identical on both generations.
const RAISE_KEYBOARD_LEDS: usize = 69;

/// Left-underglow LED count per Raise colormap layer.
const RAISE_UNDERGLOW_LEFT: usize = 30;

/// Right-underglow LED count per Raise colormap layer.
const RAISE_UNDERGLOW_RIGHT: usize = 33;

/// Left-underglow LED count per Raise 2 colormap layer.
const RAISE2_UNDERGLOW_LEFT: usize = 53;

/// Right-underglow LED count per Raise 2 colormap layer.
const RAISE2_UNDERGLOW_RIGHT: usize = 54;

/// Colors a `palette` response carries, on every product.
pub const PALETTE_COLORS: usize = 16;

/// The layers every converted keymap and colormap carries: the Defy
/// and the Sonsei both store exactly ten.
const CONVERTED_LAYERS: usize = 10;

/// The keycode a keymap position with no source carries after a
/// conversion: 65535, "transparent" — the fall-through-to-the-layer-
/// below code. This is Bazecor's own `NO_KEY` for converted positions.
pub const TRANSPARENT_KEY_CODE: u16 = 65535;

/// A conversion mapping table: for each *target* position, the
/// *source* position it takes its value from, or
/// [`NO_SOURCE_POSITION`] where the target has no equivalent key (or
/// LED) on the source product.
///
/// The Defy → Sonsei tables are transcriptions of Bazecor's
/// `defyToSonsei.ts` (`buildKeymapIndexMap` / `buildColormapIndexMap`);
/// the differentially-recorded expectations pin them byte for byte.
pub type ConversionTable = [i16];

/// Marker in a [`ConversionTable`] for a target position with no
/// source equivalent.
pub const NO_SOURCE_POSITION: i16 = -1;

/// Builds the Defy → Sonsei keymap position map.
///
/// Transcribed from Bazecor's `defyToSonsei.ts`
/// `buildKeymapIndexMap`: rows 0–3 map Sonsei columns 0–5 from Defy
/// columns 0–5 and Sonsei columns 6–11 from Defy columns 10–15
/// (dropping the Defy's inner-column keys at columns 6 and 9 and the
/// wire gaps at columns 7–8); the Sonsei thumb row maps columns 0–3
/// from the Defy's left thumb cluster top (wire positions 64–67) and
/// columns 8–11 from the right cluster top (76–79), with Sonsei
/// columns 4–7 having no Defy equivalent.
const fn defy_to_sonsei_keymap_table() -> [i16; SONSEI_KEYMAP_LAYER_POSITIONS] {
    let mut map = [NO_SOURCE_POSITION; SONSEI_KEYMAP_LAYER_POSITIONS];
    let mut row = 0;

    while row < 4 {
        let mut column = 0;

        while column < 6 {
            map[row * SONSEI_KEYMAP_COLUMNS + column] = (row * DEFY_KEYMAP_COLUMNS + column) as i16;
            column += 1;
        }

        while column < SONSEI_KEYMAP_COLUMNS {
            // Sonsei columns 6–11 ← Defy columns 10–15.
            map[row * SONSEI_KEYMAP_COLUMNS + column] =
                (row * DEFY_KEYMAP_COLUMNS + column + 4) as i16;
            column += 1;
        }

        row += 1;
    }

    // Thumb row (wire row 4): Sonsei 0–3 ← Defy 64–67, Sonsei 8–11 ←
    // Defy 76–79, Sonsei 4–7 stay without a source.
    let mut column = 0;

    while column < 4 {
        map[4 * SONSEI_KEYMAP_COLUMNS + column] = (64 + column) as i16;
        map[4 * SONSEI_KEYMAP_COLUMNS + 8 + column] = (76 + column) as i16;
        column += 1;
    }

    map
}

/// The Defy → Sonsei keymap position map (see
/// [`defy_to_sonsei_keymap_table`]).
const DEFY_TO_SONSEI_KEYMAP: [i16; SONSEI_KEYMAP_LAYER_POSITIONS] = defy_to_sonsei_keymap_table();

/// Builds the Defy → Sonsei colormap LED map, transcribed from
/// Bazecor's `defyToSonsei.ts` `buildColormapIndexMap`: each Sonsei
/// keyboard LED (0–55) takes the Defy LED its physical key sits at
/// (the Defy numbers keyboard LEDs 0–69 row-major with the wire gaps
/// collapsed; the right half runs right-to-left on the wire). The
/// Sonsei has no underglow, so nothing beyond the 56 keyboard LEDs
/// maps.
const fn defy_to_sonsei_colormap_table() -> [i16; SONSEI_COLOR_LAYER_LEDS] {
    let mut map = [NO_SOURCE_POSITION; SONSEI_COLOR_LAYER_LEDS];
    let mut column = 0;

    while column < 6 {
        // Row 0: Sonsei 0–5 ← Defy 0–5; Sonsei 6–11 ← Defy 40–35.
        map[column] = column as i16;
        map[6 + column] = (40 - column) as i16;
        // Row 1: Sonsei 12–17 ← Defy 7–12; Sonsei 18–23 ← Defy 47–42.
        map[12 + column] = (7 + column) as i16;
        map[18 + column] = (47 - column) as i16;
        // Row 2: Sonsei 24–29 ← Defy 14–19; Sonsei 30–35 ← Defy 54–49.
        map[24 + column] = (14 + column) as i16;
        map[30 + column] = (54 - column) as i16;
        // Row 3: Sonsei 36–41 ← Defy 21–26; Sonsei 42–47 ← Defy 61–56.
        map[36 + column] = (21 + column) as i16;
        map[42 + column] = (61 - column) as i16;
        column += 1;
    }

    // Thumb row: Sonsei 48–51 ← Defy 27–30 (left cluster top);
    // Sonsei 52–55 ← Defy 65–62 (right cluster top, wire order
    // reversed).
    let mut column = 0;

    while column < 4 {
        map[48 + column] = (27 + column) as i16;
        map[52 + column] = (65 - column) as i16;
        column += 1;
    }

    map
}

/// The Defy → Sonsei colormap LED map (see
/// [`defy_to_sonsei_colormap_table`]).
const DEFY_TO_SONSEI_COLORMAP: [i16; SONSEI_COLOR_LAYER_LEDS] = defy_to_sonsei_colormap_table();

/// Inverts a conversion table at compile time: for each source
/// position, the target position that reads it, or
/// [`NO_SOURCE_POSITION`] where no target does. The inverse of the
/// differentially-validated forward table is exact by construction.
const fn inverse_table<const SOURCES: usize, const TARGETS: usize>(
    forward: &[i16; TARGETS],
) -> [i16; SOURCES] {
    let mut inverse = [NO_SOURCE_POSITION; SOURCES];
    let mut target = 0;

    while target < TARGETS {
        let source = forward[target];

        if source != NO_SOURCE_POSITION {
            inverse[source as usize] = target as i16;
        }

        target += 1;
    }

    inverse
}

/// The Sonsei → Defy keymap position map: the exact inverse of
/// [`DEFY_TO_SONSEI_KEYMAP`]. Defy positions with no Sonsei source
/// (the inner-column keys, the bottom thumb rows, the wire gaps) are
/// [`NO_SOURCE_POSITION`] and convert to [`TRANSPARENT_KEY_CODE`].
const SONSEI_TO_DEFY_KEYMAP: [i16; DEFY_KEYMAP_LAYER_POSITIONS] = inverse_table::<
    DEFY_KEYMAP_LAYER_POSITIONS,
    SONSEI_KEYMAP_LAYER_POSITIONS,
>(&DEFY_TO_SONSEI_KEYMAP);

/// The Sonsei → Defy colormap LED map: the exact inverse of
/// [`DEFY_TO_SONSEI_COLORMAP`]. Every Defy underglow LED and the
/// Defy-only keyboard LEDs have no Sonsei source and convert to
/// palette color 0 (Bazecor's own default for converted colormaps).
const SONSEI_TO_DEFY_COLORMAP: [i16; DEFY_COLOR_LAYER_LEDS] =
    inverse_table::<DEFY_COLOR_LAYER_LEDS, SONSEI_COLOR_LAYER_LEDS>(&DEFY_TO_SONSEI_COLORMAP);

/// Builds a Raise-generation keymap position map: the identity, with
/// the thumb-cluster rotation entries and — when the *target*
/// keyboard is ANSI — the Enter/Shift swaps written over it.
///
/// Transcribed from Bazecor's `raiseToRaise2.ts` /
/// `raise2ToRaise.ts` `convertKeymapRtoR2` / `convertKeymapR2toR`:
/// both rotate the three wire positions 69–71 (the thumb cluster's
/// order differs between the generations) and both then swap positions
/// 31 ↔ 47 (Enter) and 48 ↔ 49 (Shift) when the target `keyboardType`
/// is `"ANSI"` — the same involutive swaps in both directions. The
/// swaps read positions the rotation does not touch, so the composite
/// is one direct target → source table; positions 72–79 pass through
/// identity in both directions.
const fn raise_keymap_table(
    rotation: [(usize, usize); 3],
    keyboard_type: KeyboardType,
) -> [i16; RAISE_KEYMAP_LAYER_POSITIONS] {
    let mut table = [0i16; RAISE_KEYMAP_LAYER_POSITIONS];
    let mut position = 0;

    while position < RAISE_KEYMAP_LAYER_POSITIONS {
        table[position] = position as i16;
        position += 1;
    }

    let mut entry = 0;

    while entry < rotation.len() {
        let (target, source) = rotation[entry];
        table[target] = source as i16;
        entry += 1;
    }

    if matches!(keyboard_type, KeyboardType::Ansi) {
        table[31] = 47;
        table[47] = 31;
        table[48] = 49;
        table[49] = 48;
    }

    table
}

/// The Raise 2 → Raise keymap position map for an ANSI target:
/// thumb-cluster rotation new[69] = old[71], new[70] = old[69],
/// new[71] = old[70], plus the ANSI Enter/Shift swaps.
const RAISE2_TO_RAISE_KEYMAP_ANSI: [i16; RAISE_KEYMAP_LAYER_POSITIONS] =
    raise_keymap_table([(69, 71), (70, 69), (71, 70)], KeyboardType::Ansi);

/// The Raise 2 → Raise keymap position map for an ISO target: the
/// same rotation without the swaps.
const RAISE2_TO_RAISE_KEYMAP_ISO: [i16; RAISE_KEYMAP_LAYER_POSITIONS] =
    raise_keymap_table([(69, 71), (70, 69), (71, 70)], KeyboardType::Iso);

/// The Raise → Raise 2 keymap position map for an ANSI target: the
/// inverse rotation new[69] = old[70], new[70] = old[71],
/// new[71] = old[69], plus the ANSI Enter/Shift swaps.
const RAISE_TO_RAISE2_KEYMAP_ANSI: [i16; RAISE_KEYMAP_LAYER_POSITIONS] =
    raise_keymap_table([(69, 70), (70, 71), (71, 69)], KeyboardType::Ansi);

/// The Raise → Raise 2 keymap position map for an ISO target: the
/// inverse rotation without the swaps.
const RAISE_TO_RAISE2_KEYMAP_ISO: [i16; RAISE_KEYMAP_LAYER_POSITIONS] =
    raise_keymap_table([(69, 70), (70, 71), (71, 69)], KeyboardType::Iso);

/// Builds a Raise 2 → Raise colormap LED map: the Raise 2 layer
/// truncated to its first 132 LEDs (the Raise's layer size), plus —
/// when the *target* keyboard is ANSI — the LED swap standing in for
/// the Enter key's move (positions 40 ↔ 48), and — when the target is
/// ANSI *and* the backup's keyboard is ISO — the swap for the Shift
/// key's move (positions 19 ↔ 20).
///
/// Transcribed from Bazecor's `raise2ToRaise.ts`
/// `convertColormapR2toR`. The truncation is the whole story for an
/// ISO target: underglow LEDs beyond the Raise's 132-per-layer are
/// dropped (they surface in the report's `dropped_sources`).
const fn raise2_to_raise_colormap_table(
    keyboard_type: KeyboardType,
    source_keyboard_type: KeyboardType,
) -> [i16; RAISE_COLOR_LAYER_LEDS] {
    let mut table = [0i16; RAISE_COLOR_LAYER_LEDS];
    let mut position = 0;

    while position < RAISE_COLOR_LAYER_LEDS {
        table[position] = position as i16;
        position += 1;
    }

    if matches!(keyboard_type, KeyboardType::Ansi) {
        table[40] = 48;
        table[48] = 40;

        if matches!(source_keyboard_type, KeyboardType::Iso) {
            table[19] = 20;
            table[20] = 19;
        }
    }

    table
}

/// The Raise 2 → Raise colormap map for an ANSI target from an ANSI
/// backup: truncation plus the Enter swap.
const RAISE2_TO_RAISE_COLORMAP_ANSI_FROM_ANSI: [i16; RAISE_COLOR_LAYER_LEDS] =
    raise2_to_raise_colormap_table(KeyboardType::Ansi, KeyboardType::Ansi);

/// The Raise 2 → Raise colormap map for an ANSI target from an ISO
/// backup: truncation plus the Enter and Shift swaps.
const RAISE2_TO_RAISE_COLORMAP_ANSI_FROM_ISO: [i16; RAISE_COLOR_LAYER_LEDS] =
    raise2_to_raise_colormap_table(KeyboardType::Ansi, KeyboardType::Iso);

/// The Raise 2 → Raise colormap map for an ISO target: plain
/// truncation, whatever the backup's layout (Bazecor's converter
/// checks the target's `keyboardType` alone for the swaps).
const RAISE2_TO_RAISE_COLORMAP_ISO: [i16; RAISE_COLOR_LAYER_LEDS] =
    raise2_to_raise_colormap_table(KeyboardType::Iso, KeyboardType::Iso);

/// Builds the Raise → Raise 2 colormap LED map: the 69 keyboard LEDs
/// pass through identity, and the Raise's underglow stretches onto the
/// Raise 2's — 30 left LEDs spread over 53 targets, 33 right LEDs over
/// 54 — the interpolation Bazecor's `convertColormapRtoR2` runs
/// (`source = floor(i × 30 / 53)` left, `floor(i × 33 / 54)` right,
/// the same map Bazecor applies whatever both `keyboardType` arguments
/// carry — the function ignores them). Every source is read: each
/// stretch interval is wider than one target step, so nothing is
/// dropped and no target is defaulted. (Bazecor's `?: 15` fallback for
/// a missing source is unreachable at the exact 132-token layer shape
/// the Raise source parse enforces.)
///
/// The integer products are exact: Bazecor computes
/// `Math.floor((i / 53) * 30)` in floating point, but `i × 30 / 53`
/// sits at least 1/53 from any integer it is not exactly (and
/// `i × 33 / 54` at least 1/18), orders of magnitude beyond double
/// rounding error — the differentially-recorded expectations pin the
/// byte equality either way.
const fn raise_to_raise2_colormap_table() -> [i16; RAISE2_COLOR_LAYER_LEDS] {
    let mut table = [NO_SOURCE_POSITION; RAISE2_COLOR_LAYER_LEDS];
    let mut position = 0;

    while position < RAISE_KEYBOARD_LEDS {
        table[position] = position as i16;
        position += 1;
    }

    let mut index = 0;

    while index < RAISE2_UNDERGLOW_LEFT {
        table[RAISE_KEYBOARD_LEDS + index] =
            (RAISE_KEYBOARD_LEDS + index * RAISE_UNDERGLOW_LEFT / RAISE2_UNDERGLOW_LEFT) as i16;
        index += 1;
    }

    let mut index = 0;

    while index < RAISE2_UNDERGLOW_RIGHT {
        table[RAISE_KEYBOARD_LEDS + RAISE2_UNDERGLOW_LEFT + index] = (RAISE_KEYBOARD_LEDS
            + RAISE_UNDERGLOW_LEFT
            + index * RAISE_UNDERGLOW_RIGHT / RAISE2_UNDERGLOW_RIGHT)
            as i16;
        index += 1;
    }

    table
}

/// The Raise → Raise 2 colormap LED map (see
/// [`raise_to_raise2_colormap_table`]).
const RAISE_TO_RAISE2_COLORMAP: [i16; RAISE2_COLOR_LAYER_LEDS] = raise_to_raise2_colormap_table();

/// What a target position with no source receives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DefaultValue {
    /// Keymap code 65535, "transparent" (fall through to the layer
    /// below) — Bazecor's `NO_KEY` for converted keymap positions.
    Transparent,
    /// Palette color 0 — Bazecor's convention for converted colormap
    /// positions with no source LED.
    FirstPaletteColor,
}

impl DefaultValue {
    /// The wire value a defaulted position carries.
    pub fn value(self) -> u16 {
        match self {
            Self::Transparent => TRANSPARENT_KEY_CODE,
            Self::FirstPaletteColor => 0,
        }
    }
}

/// How a conversion maps positions of one layer, and exactly what it
/// mapped, defaulted, and dropped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PositionMapReport {
    /// How many layers carry this mapping (identical every layer by
    /// construction — the geometry does not vary per layer).
    pub layers: usize,
    /// Source positions per layer.
    pub source_positions: usize,
    /// Target positions per layer.
    pub target_positions: usize,
    /// The mapping table: target position → source position, or
    /// [`NO_SOURCE_POSITION`] where the target has no source.
    pub target_to_source: Vec<i16>,
    /// Source positions that no target position reads, ascending —
    /// the data these hold does not survive the conversion.
    pub dropped_sources: Vec<u16>,
    /// What defaulted positions receive.
    pub default_value: DefaultValue,
}

impl PositionMapReport {
    /// How many target positions per layer take a source value.
    pub fn mapped_per_layer(&self) -> usize {
        self.target_positions - self.defaulted_per_layer()
    }

    /// Which target positions per layer have no source, ascending.
    pub fn defaulted_positions(&self) -> Vec<u16> {
        self.target_to_source
            .iter()
            .enumerate()
            .filter(|(_, source)| **source == NO_SOURCE_POSITION)
            .map(|(target, _)| target as u16)
            .collect()
    }

    /// How many target positions per layer have no source.
    pub fn defaulted_per_layer(&self) -> usize {
        self.target_to_source
            .iter()
            .filter(|source| **source == NO_SOURCE_POSITION)
            .count()
    }
}

/// How a conversion reshapes the palette's colors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorChannelConversion {
    /// RGBW → RGB: the white channel folds into the other three
    /// (channel becomes `min(255, white + channel)`, Bazecor's
    /// `rgbw2b`).
    RgbwToRgb,
    /// RGB → RGBW: the white channel carries the shared component
    /// (`white = min(r, g, b)`, subtracted from the channels —
    /// Bazecor's `rgb2w`).
    RgbToRgbw,
}

impl ColorChannelConversion {
    /// The channel count before the conversion.
    pub fn source_channels(self) -> usize {
        match self {
            Self::RgbwToRgb => 4,
            Self::RgbToRgbw => 3,
        }
    }

    /// The channel count after the conversion.
    pub fn target_channels(self) -> usize {
        match self {
            Self::RgbwToRgb => 3,
            Self::RgbToRgbw => 4,
        }
    }
}

impl std::fmt::Display for ColorChannelConversion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RgbwToRgb => f.write_str("RGBW → RGB (white channel folded in)"),
            Self::RgbToRgbw => f.write_str("RGB → RGBW (white channel split out)"),
        }
    }
}

/// What one conversion did, in full — the honest record of which data
/// moved 1:1, which was invented (defaulted), and which was dropped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConversionReport {
    /// The product the backup was captured from.
    pub from: Product,
    /// The product the backup was converted to.
    pub to: Product,
    /// How the keymap layers mapped.
    pub keymap: PositionMapReport,
    /// How the colormap layers mapped.
    pub colormap: PositionMapReport,
    /// How the palette colors converted.
    pub palette: ColorChannelConversion,
    /// How many captured commands other than the converted ones passed
    /// through verbatim.
    pub passthrough_commands: usize,
    /// Target keymap positions, among the report's defaulted ones,
    /// that the [`DefyKeymap`] type does not model (the wire-padding
    /// slots): their serialized value is `0` — the way a real Defy
    /// emits padding — not the [`DefaultValue::Transparent`] the
    /// mapping table defaults to. Empty when the conversion does not
    /// target the Defy. Carried so no consumer has to guess which
    /// defaulted positions the output rewrites.
    pub normalized_padding: Vec<u16>,
}

/// The commands a conversion rewrites. Every other captured command
/// passes through verbatim, exactly as in Bazecor's conversion.
const CONVERTED_COMMANDS: [&str; 3] = ["keymap.custom", "colormap.map", "palette"];

/// Everything a conversion pair needs, resolved once: the mapping
/// tables, the source and target shapes, the palette direction, and
/// the target descriptor. One value per *supported* pair — a product
/// geometry without a conversion has no [`ConversionRecipe`] to reach.
struct ConversionRecipe {
    /// Target position → source position, per layer.
    keymap_table: &'static ConversionTable,
    /// Target LED → source LED, per layer.
    colormap_table: &'static ConversionTable,
    /// How the palette's colors convert.
    palette: ColorChannelConversion,
    /// Keymap positions per layer, source and target.
    source_keymap_positions: usize,
    target_keymap_positions: usize,
    /// Colormap LEDs per layer, source — resolved per backup, see
    /// [`SourceColorLeds`] — and target.
    source_color_leds: SourceColorLeds,
    target_color_leds: usize,
    /// The device descriptor the converted backup's neuron object
    /// carries.
    target_descriptor: fn() -> serde_json::Value,
}

/// Where a conversion's source colormap layer width comes from.
///
/// A product constant for the Defy and Sonsei pairs: both their
/// directions have one layer shape each (178 and 56), and the Sonsei
/// descriptor carries no usable geometry at all (`rows: 0, columns:
/// 0` — deriving the width from it would break Sonsei → Defy). The
/// Raise generation is the one whose conversions Bazecor sizes from
/// the backup descriptor's `keyboardUnderglow` rows × columns — the
/// descriptor geometry *is* the Raise colormap layer — so the recipe
/// names the source and [`convert_backup`] resolves it per backup.
#[derive(Clone, Copy)]
enum SourceColorLeds {
    /// The width both directions of the pair use.
    Fixed(usize),
    /// `keyboardUnderglow.rows × keyboardUnderglow.columns` off the
    /// backup's own descriptor.
    DescriptorUnderglow,
}

/// The recipe for a conversion pair, or [`ConversionError::
/// UnsupportedConversion`] — the single place a pair is judged, so an
/// unsupported pair can only ever produce that error, never a
/// half-built conversion. The Raise 2 → Raise arm also validates the
/// backup's `keyboardType`, the field Bazecor's restore passes its
/// converters as `backupKeyboardType` — though only the ANSI-target
/// colormap table branches on it. The validation is uniform, whatever
/// the target: an ISO-target conversion of a backup with a garbled
/// layout is refused here, where Bazecor's converter would complete
/// without reading the argument — a documented deviation (uniform
/// validation).
fn recipe(
    from: Product,
    to: ConversionTarget,
    backup: &DefyBackup,
) -> Result<&'static ConversionRecipe, ConversionError> {
    match (from, to) {
        (Product::Defy, ConversionTarget::Sonsei) => Ok(&DEFY_TO_SONSEI),
        (Product::Sonsei, ConversionTarget::Defy) => Ok(&SONSEI_TO_DEFY),
        (Product::Raise, ConversionTarget::Raise2(keyboard_type)) => Ok(match keyboard_type {
            KeyboardType::Ansi => &RAISE_TO_RAISE2_ANSI,
            KeyboardType::Iso => &RAISE_TO_RAISE2_ISO,
        }),
        (Product::Raise2, ConversionTarget::Raise(keyboard_type)) => {
            let source_keyboard_type =
                KeyboardType::from_descriptor(backup.neuron.device.as_ref())?;

            Ok(match (keyboard_type, source_keyboard_type) {
                (KeyboardType::Ansi, KeyboardType::Ansi) => &RAISE2_TO_RAISE_ANSI_FROM_ANSI,
                (KeyboardType::Ansi, KeyboardType::Iso) => &RAISE2_TO_RAISE_ANSI_FROM_ISO,
                (KeyboardType::Iso, _) => &RAISE2_TO_RAISE_ISO,
            })
        }
        (from, to) => Err(ConversionError::UnsupportedConversion {
            from,
            to: to.product(),
        }),
    }
}

/// The Defy → Sonsei conversion recipe. The tables are the
/// differentially-validated forward tables; the geometry numbers
/// mirror the consts they are named after.
static DEFY_TO_SONSEI: ConversionRecipe = ConversionRecipe {
    keymap_table: &DEFY_TO_SONSEI_KEYMAP,
    colormap_table: &DEFY_TO_SONSEI_COLORMAP,
    palette: ColorChannelConversion::RgbwToRgb,
    source_keymap_positions: DEFY_KEYMAP_LAYER_POSITIONS,
    target_keymap_positions: SONSEI_KEYMAP_LAYER_POSITIONS,
    source_color_leds: SourceColorLeds::Fixed(DEFY_COLOR_LAYER_LEDS),
    target_color_leds: SONSEI_COLOR_LAYER_LEDS,
    target_descriptor: sonsei_device_descriptor,
};

/// The Sonsei → Defy conversion recipe: the exact inverses of the
/// forward tables, and Bazecor's `rgb2w` for the palette direction.
static SONSEI_TO_DEFY: ConversionRecipe = ConversionRecipe {
    keymap_table: &SONSEI_TO_DEFY_KEYMAP,
    colormap_table: &SONSEI_TO_DEFY_COLORMAP,
    palette: ColorChannelConversion::RgbToRgbw,
    source_keymap_positions: SONSEI_KEYMAP_LAYER_POSITIONS,
    target_keymap_positions: DEFY_KEYMAP_LAYER_POSITIONS,
    source_color_leds: SourceColorLeds::Fixed(SONSEI_COLOR_LAYER_LEDS),
    target_color_leds: DEFY_COLOR_LAYER_LEDS,
    target_descriptor: defy_target_descriptor,
};

/// The Raise → Raise 2 recipes, one per target layout: the
/// rotation-plus-swaps keymap tables, the underglow interpolation
/// (layout-blind, as Bazecor's converter is), RGB → RGBW through
/// Bazecor's `rgb2w`, and the Raise 2 target descriptor.
static RAISE_TO_RAISE2_ANSI: ConversionRecipe = ConversionRecipe {
    keymap_table: &RAISE_TO_RAISE2_KEYMAP_ANSI,
    colormap_table: &RAISE_TO_RAISE2_COLORMAP,
    palette: ColorChannelConversion::RgbToRgbw,
    source_keymap_positions: RAISE_KEYMAP_LAYER_POSITIONS,
    target_keymap_positions: RAISE_KEYMAP_LAYER_POSITIONS,
    source_color_leds: SourceColorLeds::DescriptorUnderglow,
    target_color_leds: RAISE2_COLOR_LAYER_LEDS,
    target_descriptor: || raise2_device_descriptor(KeyboardType::Ansi),
};

static RAISE_TO_RAISE2_ISO: ConversionRecipe = ConversionRecipe {
    keymap_table: &RAISE_TO_RAISE2_KEYMAP_ISO,
    colormap_table: &RAISE_TO_RAISE2_COLORMAP,
    palette: ColorChannelConversion::RgbToRgbw,
    source_keymap_positions: RAISE_KEYMAP_LAYER_POSITIONS,
    target_keymap_positions: RAISE_KEYMAP_LAYER_POSITIONS,
    source_color_leds: SourceColorLeds::DescriptorUnderglow,
    target_color_leds: RAISE2_COLOR_LAYER_LEDS,
    target_descriptor: || raise2_device_descriptor(KeyboardType::Iso),
};

/// The Raise 2 → Raise recipes, selected by target layout and — the
/// ANSI target only — the backup's layout (the colormap's Shift swap).
/// RGBW → RGB where the white channel folds into the display colors
/// (Bazecor's `parsePaletteRaw(data, true)` + `[r, g, b]`
/// serialization).
static RAISE2_TO_RAISE_ANSI_FROM_ANSI: ConversionRecipe = ConversionRecipe {
    keymap_table: &RAISE2_TO_RAISE_KEYMAP_ANSI,
    colormap_table: &RAISE2_TO_RAISE_COLORMAP_ANSI_FROM_ANSI,
    palette: ColorChannelConversion::RgbwToRgb,
    source_keymap_positions: RAISE_KEYMAP_LAYER_POSITIONS,
    target_keymap_positions: RAISE_KEYMAP_LAYER_POSITIONS,
    source_color_leds: SourceColorLeds::DescriptorUnderglow,
    target_color_leds: RAISE_COLOR_LAYER_LEDS,
    target_descriptor: || raise_device_descriptor(KeyboardType::Ansi),
};

static RAISE2_TO_RAISE_ANSI_FROM_ISO: ConversionRecipe = ConversionRecipe {
    keymap_table: &RAISE2_TO_RAISE_KEYMAP_ANSI,
    colormap_table: &RAISE2_TO_RAISE_COLORMAP_ANSI_FROM_ISO,
    palette: ColorChannelConversion::RgbwToRgb,
    source_keymap_positions: RAISE_KEYMAP_LAYER_POSITIONS,
    target_keymap_positions: RAISE_KEYMAP_LAYER_POSITIONS,
    source_color_leds: SourceColorLeds::DescriptorUnderglow,
    target_color_leds: RAISE_COLOR_LAYER_LEDS,
    target_descriptor: || raise_device_descriptor(KeyboardType::Ansi),
};

static RAISE2_TO_RAISE_ISO: ConversionRecipe = ConversionRecipe {
    keymap_table: &RAISE2_TO_RAISE_KEYMAP_ISO,
    colormap_table: &RAISE2_TO_RAISE_COLORMAP_ISO,
    palette: ColorChannelConversion::RgbwToRgb,
    source_keymap_positions: RAISE_KEYMAP_LAYER_POSITIONS,
    target_keymap_positions: RAISE_KEYMAP_LAYER_POSITIONS,
    source_color_leds: SourceColorLeds::DescriptorUnderglow,
    target_color_leds: RAISE_COLOR_LAYER_LEDS,
    target_descriptor: || raise_device_descriptor(KeyboardType::Iso),
};

/// The Defy descriptor a Defy-targeting conversion carries: the
/// descriptor [`DefyKeyboard::backup_device_descriptor`] builds, with
/// the `chipId` emptied — the converting process is offline and cannot
/// know the target Neuron's identity. The `wireless: true` hardcode is
/// the same story: restore flows (Bazecor's and this crate's plan
/// guard) match on `info.product` only, so the wireless flag is inert
/// in a converted descriptor; it is left at the shipped descriptor's
/// value rather than invented per-target.
fn defy_target_descriptor() -> serde_json::Value {
    DefyKeyboard::backup_device_descriptor(true, "")
}

/// Converts a captured backup between product wire shapes.
///
/// The source product comes from the backup's own device descriptor
/// (the field Bazecor's restore switches on); `to` is the requested
/// target — product *and*, for a Raise-generation target, the physical
/// key layout the swaps are keyed on. The product-shaped commands —
/// `keymap.custom`, `colormap.map`, and `palette` — are rewritten;
/// every other captured command passes through verbatim, exactly as in
/// Bazecor's conversion (the superkey and macro wire formats are
/// product-independent). Each converted command must be captured
/// exactly once — a duplicate would survive the conversion unconverted,
/// so [`ConversionError::DuplicateCommand`] refuses what Bazecor
/// silently accepts. The backup's device descriptor is replaced with
/// the target product's, as Bazecor replaces it with the connected
/// device's — offline, the identity fields (`chipId`) stay empty
/// because the target Neuron is not known.
///
/// A Raise-generation conversion also reads the backup's descriptor
/// for the colormap source width (the `keyboardUnderglow` geometry,
/// as Bazecor's Raise conversions do) and — Raise 2 → Raise — for the
/// backup's own layout, which the ANSI target's colormap table
/// branches on. Either read failing is a typed error
/// ([`ConversionError::MissingUnderglowGeometry`],
/// [`ConversionError::NoKeyboardType`],
/// [`ConversionError::UnknownKeyboardType`]).
///
/// When the target is the Defy, the converted keymap re-enters through
/// [`DefyKeymap`], the exactly-ten-layer type: the output is a valid
/// Defy `keymap.custom` write by construction, its padding positions
/// normalize to `0` — the value this SDK's keymap serializer has
/// always emitted for padding — and the converted
/// backup passes
/// [`DefyKeyboard::backup_restore_plan`] unchanged. The report's
/// [`ConversionReport::normalized_padding`] names exactly which
/// defaulted positions were rewritten this way.
///
/// On success the converted backup and its full
/// [`ConversionReport`] are returned; the report is the record of
/// which positions mapped 1:1, which were defaulted, and which source
/// positions were dropped.
pub fn convert_backup(
    backup: &DefyBackup,
    to: ConversionTarget,
) -> Result<(DefyBackup, ConversionReport), ConversionError> {
    let from = Product::from_descriptor(backup.neuron.device.as_ref())?;

    if from == to.product() {
        return Err(ConversionError::SameProduct {
            product: to.product(),
        });
    }

    let recipe = recipe(from, to, backup)?;

    let source_color_leds = match recipe.source_color_leds {
        SourceColorLeds::Fixed(leds) => leds,
        SourceColorLeds::DescriptorUnderglow => {
            let width = descriptor_underglow_width(backup.neuron.device.as_ref())?;

            // The runtime-resolved width is external input, and the
            // tables index the layers they are handed up to
            // [`table_source_span`] — a descriptor claiming fewer LEDs
            // per layer than that cannot be describing the data beside
            // it (self-consistent-but-wrong input: the shape checks
            // would pass, the remap would read past the layer's end).
            // Fixed widths cannot disagree — they are the same consts
            // the tables were built from, pinned by test.
            let span = table_source_span(recipe.colormap_table);

            if width < span {
                return Err(ConversionError::UnderglowGeometryTooNarrow {
                    claimed: width,
                    needed: span,
                });
            }

            width
        }
    };

    // The entry positions, captured up front: the conversion rewrites
    // the entries at the positions they were captured at, keeping the
    // capture order (Bazecor's semantics).
    let keymap_index = entry_index(backup, "keymap.custom")?;
    let colormap_index = entry_index(backup, "colormap.map")?;
    let palette_index = entry_index(backup, "palette")?;

    let source_keymap = parse_layered_values(
        &backup.backup[keymap_index].data,
        recipe.source_keymap_positions,
        CONVERTED_LAYERS,
        u16::MAX as u32,
    )
    .map_err(|source| ConversionError::InvalidEntry {
        command: "keymap.custom",
        source,
    })?;
    let source_colormap = parse_layered_values(
        &backup.backup[colormap_index].data,
        source_color_leds,
        CONVERTED_LAYERS,
        u8::MAX as u32,
    )
    .map_err(|source| ConversionError::InvalidEntry {
        command: "colormap.map",
        source,
    })?;
    let source_palette = parse_layered_values(
        &backup.backup[palette_index].data,
        recipe.palette.source_channels(),
        PALETTE_COLORS,
        u8::MAX as u32,
    )
    .map_err(|source| ConversionError::InvalidEntry {
        command: "palette",
        source,
    })?;

    let target_keymap: Vec<Vec<u16>> = source_keymap
        .iter()
        .map(|layer| remap(layer, recipe.keymap_table, DefaultValue::Transparent))
        .collect();
    let target_colormap: Vec<Vec<u16>> = source_colormap
        .iter()
        .map(|layer| {
            remap(
                layer,
                recipe.colormap_table,
                DefaultValue::FirstPaletteColor,
            )
        })
        .collect();
    let target_palette: Vec<Vec<u16>> = source_palette
        .iter()
        .map(|color| convert_color(color, recipe.palette))
        .collect();

    // A Defy target carries its keymap through the exactly-ten-layer
    // type: the write shape is proven, and the padding positions the
    // type does not model normalize to `0` — the value this SDK's
    // keymap serializer has always emitted for padding. The unmodeled
    // positions are read off the type itself
    // (`to_keymap_data` answers `None` exactly there), so the report
    // can name which defaulted positions the output rewrites.
    let normalized_padding = if to.product() == Product::Defy {
        let unmodeled = DefyKeymapLayer::new_cleared_to(KeyKind::from(0))
            .to_keymap_data()
            .iter()
            .enumerate()
            .filter(|(_, key)| key.is_none())
            .map(|(index, _)| index as u16)
            .collect::<Vec<_>>();

        Some(unmodeled)
    } else {
        None
    };

    let target_keymap_data = if to.product() == Product::Defy {
        let layers: Vec<DefyKeymapLayer> = target_keymap
            .iter()
            .map(|layer| {
                let codes: DefyLayerData = array::from_fn(|index| KeyKind::from(layer[index]));

                DefyKeymapLayer::from(&codes)
            })
            .collect();

        DefyKeymap::try_from(layers)
            .expect("the keymap is built with exactly ten layers")
            .to_keymap_custom_data()
    } else {
        join_layers(&target_keymap)
    };

    let mut converted = backup.clone();

    converted.backup[keymap_index].data = target_keymap_data;
    converted.backup[colormap_index].data = join_layers(&target_colormap);
    converted.backup[palette_index].data = join_layers(&target_palette);

    converted.neuron.device = Some((recipe.target_descriptor)());

    let report = ConversionReport {
        from,
        to: to.product(),
        keymap: map_report(
            recipe.keymap_table,
            CONVERTED_LAYERS,
            recipe.source_keymap_positions,
            recipe.target_keymap_positions,
            DefaultValue::Transparent,
        ),
        colormap: map_report(
            recipe.colormap_table,
            CONVERTED_LAYERS,
            source_color_leds,
            recipe.target_color_leds,
            DefaultValue::FirstPaletteColor,
        ),
        palette: recipe.palette,
        passthrough_commands: converted
            .backup
            .iter()
            .filter(|entry| !CONVERTED_COMMANDS.contains(&entry.command.as_str()))
            .count(),
        normalized_padding: normalized_padding.unwrap_or_default(),
    };

    Ok((converted, report))
}

/// Converts a Defy-shaped backup into Sonsei wire shape.
///
/// The Sonsei output is validated as far as this crate can (the layer
/// shapes are exact by construction); there is no Sonsei device model
/// here to type the whole file against — the Defy → Sonsei direction
/// is instead validated differentially against Bazecor's own
/// conversion output (see the [module docs](self)).
pub fn convert_defy_backup_to_sonsei(
    backup: &DefyBackup,
) -> Result<(DefyBackup, ConversionReport), ConversionError> {
    convert_backup(backup, ConversionTarget::Sonsei)
}

/// Converts a Sonsei-shaped backup into Defy wire shape.
///
/// The converted keymap is built through [`DefyKeymap`] — the
/// exactly-ten-layer type — so the output is a valid Defy
/// `keymap.custom` write by construction and passes
/// [`DefyKeyboard::backup_restore_plan`].
pub fn convert_sonsei_backup_to_defy(
    backup: &DefyBackup,
) -> Result<(DefyBackup, ConversionReport), ConversionError> {
    convert_backup(backup, ConversionTarget::Defy)
}

/// Converts a Raise-shaped backup into Raise 2 wire shape, for a
/// Raise 2 keyboard of the given physical layout — the `keyboardType`
/// Bazecor's restore passes from the connected device
/// (`Backup.ts` `convertRaiseToRaise2`).
///
/// The Raise 2 output is validated as far as this crate can (the
/// layer shapes are exact by construction); there is no Raise 2 device
/// model here to type the whole file against — the direction is
/// instead validated differentially against Bazecor's own conversion
/// output (see the [module docs](self)).
pub fn convert_raise_backup_to_raise2(
    backup: &DefyBackup,
    keyboard_type: KeyboardType,
) -> Result<(DefyBackup, ConversionReport), ConversionError> {
    convert_backup(backup, ConversionTarget::Raise2(keyboard_type))
}

/// Converts a Raise 2-shaped backup into Raise wire shape, for a
/// Raise keyboard of the given physical layout — the `keyboardType`
/// Bazecor's restore passes from the connected device
/// (`Backup.ts` `convertRaise2ToRaise`).
///
/// The colormap source width comes from the backup descriptor's
/// `keyboardUnderglow` geometry, so a Raise 2 source whose descriptor
/// disagrees with its own data (the shipped Raise 2 ISO virtual's
/// ragged colormap) fails the shape check instead of converting
/// misaligned layers — see the [module docs](self).
pub fn convert_raise2_backup_to_raise(
    backup: &DefyBackup,
    keyboard_type: KeyboardType,
) -> Result<(DefyBackup, ConversionReport), ConversionError> {
    convert_backup(backup, ConversionTarget::Raise(keyboard_type))
}

/// The position of the captured entry for `command`, which must be
/// exactly one: a later duplicate would survive the conversion
/// unconverted (Bazecor's `findIndex` takes the first and silently
/// leaves it), so a backup with one is refused.
fn entry_index(backup: &DefyBackup, command: &'static str) -> Result<usize, ConversionError> {
    let positions: Vec<usize> = backup
        .backup
        .iter()
        .enumerate()
        .filter(|(_, entry)| entry.command == command)
        .map(|(index, _)| index)
        .collect();

    match positions.as_slice() {
        [] => Err(ConversionError::MissingCommand { command }),
        [index] => Ok(*index),
        _ => Err(ConversionError::DuplicateCommand {
            command,
            count: positions.len(),
        }),
    }
}

/// The largest `keyboardUnderglow` rows × columns product this crate
/// resolves: hundreds of times past the largest real colormap layer
/// (the Defy's 178), and small enough that every count computed from
/// it stays far inside a `usize`. A claim beyond it is as unusable as
/// an unreadable one — no keyboard has a colormap layer that size.
const UNDERGLOW_WIDTH_LIMIT: u64 = 65_536;

/// Reads a Raise-generation colormap source width out of the backup
/// descriptor's `keyboardUnderglow` geometry — exactly what Bazecor's
/// Raise conversions parse their colormap at
/// (`Backup.ts`: `bkpDev.keyboardUnderglow.rows *
/// bkpDev.keyboardUnderglow.columns`). A descriptor whose geometry
/// does not resolve to a usable width — the fields missing or
/// non-numeric, the product zero, overflowing a `u64`, or landing past
/// the width limit — is [`ConversionError::MissingUnderglowGeometry`]
/// here. Bazecor has no working answer in that spot, and which failure
/// it hits depends on the defect, not on which restore path runs: with
/// the `keyboardUnderglow` object absent, both `restoreBackup`'s
/// converters and `restoreVirtual` throw `TypeError` on
/// `undefined.rows` (the converters' `: 132` / `: 176` fallbacks are
/// for a *descriptor-less* backup — unreachable through
/// `restoreBackup`, whose dispatch dereferences
/// `backup.neuron.device.info.product` before ever calling them); with
/// the object present but its fields unreadable, both multiply
/// `undefined × undefined` into `NaN`, and `parseColormapRaw`'s
/// chunker then buckets every token under the array's `"NaN"` property
/// — the array's length stays 0, the later `.map` visits nothing, and
/// the serialized colormap is an empty string: silent total loss.
fn descriptor_underglow_width(
    device: Option<&serde_json::Value>,
) -> Result<usize, ConversionError> {
    let geometry = device
        .and_then(|device| device.get("keyboardUnderglow"))
        .and_then(|underglow| {
            let rows = underglow.get("rows").and_then(serde_json::Value::as_u64)?;
            let columns = underglow
                .get("columns")
                .and_then(serde_json::Value::as_u64)?;

            // The product is external input: an overflow is a broken
            // geometry claim, not a crash.
            rows.checked_mul(columns)
        });

    match geometry {
        Some(width) if width > 0 && width <= UNDERGLOW_WIDTH_LIMIT => Ok(width as usize),
        _ => Err(ConversionError::MissingUnderglowGeometry),
    }
}

/// The number of source positions a conversion table reads: its
/// highest referenced position plus one. The Raise-pair colormap
/// tables read 132 per layer (the full Raise layer — the truncation
/// tables keep every position 0–131, the interpolation table stretches
/// from every one); [`convert_backup`] guards a runtime-resolved
/// descriptor width against this span, which is what makes
/// [`remap`]'s indexing safe on external input.
fn table_source_span(table: &ConversionTable) -> usize {
    table
        .iter()
        .filter(|source| **source != NO_SOURCE_POSITION)
        .map(|source| *source as usize + 1)
        .max()
        .unwrap_or(0)
}

/// Parses whitespace-separated decimal values shaped as `layer_count`
/// layers of `layer_size`, every value at most `max_value`.
///
/// This is the conversion layer's own shape check for *foreign*
/// product data (a Sonsei 60-position layer is not a Focus wire format
/// this crate parses anywhere else) — the equivalent of Bazecor's
/// `parseKeymapRaw`/`parseColormapRaw`/`parsePaletteRaw` chunking,
/// with the exact-count and range checks Bazecor leaves out.
fn parse_layered_values(
    data: &str,
    layer_size: usize,
    layer_count: usize,
    max_value: u32,
) -> Result<Vec<Vec<u16>>, LayeredWireError> {
    let error = |kind| LayeredWireError {
        layer_size,
        layer_count,
        max_value,
        kind,
    };

    let expected = layer_count * layer_size;
    let mut layers = Vec::with_capacity(layer_count);
    // No capacity hint on the in-flight layer: `layer_size` is
    // external input here (the Raise pairs resolve it from the backup
    // descriptor), and preallocating it would turn a crafted absurd
    // geometry into an allocation abort before a single token is
    // examined. The vectors hold at most 178 elements in every real
    // case; amortized growth is the cheap side of that trade.
    let mut current: Vec<u16> = Vec::new();
    let mut searched_to = 0;

    for token in data.split_whitespace() {
        // The token's real byte offset: `split_whitespace` skips
        // separator runs, so search forward from the previous token's
        // end. The search finds the token at (or within a separator
        // run of) the cursor, so this stays linear in practice.
        let offset = data[searched_to..]
            .find(token)
            .map(|relative| searched_to + relative)
            .unwrap_or(searched_to);

        let value: u32 = token.parse().map_err(|_| {
            error(LayeredWireErrorKind::TokenNotANumber {
                offset,
                token: token.to_owned(),
            })
        })?;

        if value > max_value {
            return Err(error(LayeredWireErrorKind::TokenOutOfRange {
                offset,
                token: token.to_owned(),
                max: max_value,
            }));
        }

        current.push(value as u16);

        if current.len() == layer_size {
            layers.push(std::mem::take(&mut current));
        }

        searched_to = offset + token.len();
    }

    // Complete layers close as soon as their last token is seen, so
    // an exact count leaves nothing in flight: the data either holds
    // exactly `expected` values or it fails here.
    let found = layers.len() * layer_size + current.len();

    if found != expected {
        return Err(error(LayeredWireErrorKind::WrongValueCount {
            found,
            expected,
        }));
    }

    Ok(layers)
}

/// Applies a conversion table to one layer: each target position takes
/// its source position's value, or `default_value` where the table has
/// no source. The table's source positions index `layer` directly. For
/// the fixed-geometry recipes that holds by construction (compile-time
/// inversion of the differentially-validated forward tables) and by
/// test; for the Raise pairs' descriptor-derived widths,
/// [`convert_backup`] guards the resolve point — a descriptor claiming
/// fewer LEDs per layer than the table reads
/// ([`table_source_span`]) is rejected with
/// [`ConversionError::UnderglowGeometryTooNarrow`] before any layer is
/// remapped.
fn remap(layer: &[u16], table: &ConversionTable, default_value: DefaultValue) -> Vec<u16> {
    table
        .iter()
        .map(|source| {
            if *source == NO_SOURCE_POSITION {
                default_value.value()
            } else {
                layer[*source as usize]
            }
        })
        .collect()
}

/// Converts one palette color between RGBW and RGB, with Bazecor's
/// exact channel math (`rgbw2b` / `rgb2w` in `bazecor/src/api/color`).
fn convert_color(color: &[u16], direction: ColorChannelConversion) -> Vec<u16> {
    match direction {
        ColorChannelConversion::RgbwToRgb => {
            let [r, g, b, w]: [u16; 4] = color.try_into().unwrap_or_else(|_| {
                unreachable!("a parsed palette color carries exactly 4 RGBW channels")
            });

            // rgbw2b: each channel gains the white component, clamped
            // to the byte range (Bazecor clamps with the same effect
            // through `sanitizeIntensity`).
            [
                (w.saturating_add(r)).min(u16::from(u8::MAX)),
                (w.saturating_add(g)).min(u16::from(u8::MAX)),
                (w.saturating_add(b)).min(u16::from(u8::MAX)),
            ]
            .into_iter()
            .collect()
        }
        ColorChannelConversion::RgbToRgbw => {
            let [r, g, b]: [u16; 3] = color.try_into().unwrap_or_else(|_| {
                unreachable!("a parsed palette color carries exactly 3 RGB channels")
            });

            // rgb2w: the white channel carries the shared component.
            let white = r.min(g).min(b);

            [r - white, g - white, b - white, white]
                .into_iter()
                .collect()
        }
    }
}

/// Joins converted layers back into one wire data string: the layers'
/// values, space-separated, no trailing separator — the same shape
/// Bazecor's conversions serialize (`.flat().map(String).join(" ")`).
fn join_layers(layers: &[Vec<u16>]) -> String {
    layers
        .iter()
        .flatten()
        .map(u16::to_string)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Builds the mapping report for a conversion table.
fn map_report(
    table: &ConversionTable,
    layers: usize,
    source_positions: usize,
    target_positions: usize,
    default_value: DefaultValue,
) -> PositionMapReport {
    let mut referenced = vec![false; source_positions];

    for source in table {
        if *source != NO_SOURCE_POSITION {
            referenced[*source as usize] = true;
        }
    }

    PositionMapReport {
        layers,
        source_positions,
        target_positions,
        target_to_source: table.to_vec(),
        dropped_sources: referenced
            .iter()
            .enumerate()
            .filter(|(_, referenced)| !*referenced)
            .map(|(position, _)| position as u16)
            .collect(),
        default_value,
    }
}

/// The Sonsei device descriptor a converted backup's neuron object
/// carries: the shipped Sonsei backup's descriptor
/// (`bazecor/src/defaultBackups/Sonsei.json`, `neuron.device`) with the
/// `chipId` emptied — the converting process is offline and cannot
/// know the Neuron the converted file will land on. Restore flows
/// match on `info.product`, which is what the replacement is for.
pub fn sonsei_device_descriptor() -> serde_json::Value {
    serde_json::json!({
        "info": {
            "vendor": "Dygma",
            "product": "Sonsei",
            "keyboardType": "wireless",
            "displayName": "Dygma Sonsei",
            "urls": [
                { "name": "Homepage", "url": "https://www.dygma.com/sonsei/" }
            ]
        },
        "usb": { "vendorId": 13807, "productId": 49 },
        "keyboard": {
            "rows": 5,
            "columns": 12,
            "left": [
                [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
                [12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23],
                [24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35],
                [36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47],
                [48, 49, 50, 51, 52, 53, 54, 55]
            ],
            "right": [],
            "ledsLeft": (0..56).collect::<Vec<i32>>(),
            "ledsRight": []
        },
        "keyboardUnderglow": { "rows": 0, "columns": 0, "ledsLeft": [], "ledsRight": [] },
        "RGBWMode": false,
        "hasRF": false,
        "sides": 1,
        "components": {},
        "instructions": {
            "en": {
                "updateInstructions": "To update the firmware, the keyboard needs a special reset. When the countdown starts, press and hold the Escape key. Soon after the countdown finished, the Neuron's light should start a blue pulsing pattern, and the flashing will proceed. At this point, you should release the Escape key."
            }
        },
        "chipId": ""
    })
}

/// The Raise device descriptor a Raise-targeting conversion carries:
/// the shipped Raise backup's `neuron.device` for the given layout
/// (`bazecor/src/defaultBackups/RaiseANSI.json` /
/// `RaiseISO.json` — real Neuron backups) with the `chipId` emptied —
/// the converting process is offline and cannot know the target
/// Neuron's identity. The two shipped descriptors differ beyond the
/// layout: only the ANSI one carries an `RGBWMode` key at all, and
/// both are transcribed exactly as shipped (apart from the chip id).
///
/// Restore flows match on `info.product`, which is what the
/// replacement is for.
pub fn raise_device_descriptor(keyboard_type: KeyboardType) -> serde_json::Value {
    let mut descriptor = serde_json::json!({
        "info": {
            "vendor": "Dygma",
            "product": "Raise",
            "keyboardType": keyboard_type.name(),
            "displayName": match keyboard_type {
                KeyboardType::Ansi => "Dygma Raise ANSI",
                KeyboardType::Iso => "Dygma Raise ISO",
            },
            "urls": [
                { "name": "Homepage", "url": "https://www.dygma.com/raise/" }
            ]
        },
        "usb": { "vendorId": 4617, "productId": 8705 },
        "keyboard": {
            "rows": 5,
            "columns": 16,
            "left": [
                [0, 1, 2, 3, 4, 5, 6],
                [16, 17, 18, 19, 20, 21, 22],
                [32, 33, 34, 35, 36, 37, 38],
                [48, 49, 50, 51, 52, 53, 54],
                [64, 65, 66, 67, 68, 69, 70, 71]
            ],
            "right": [
                [9, 10, 11, 12, 13, 14, 15],
                [24, 25, 26, 27, 28, 29, 30, 31],
                [41, 42, 43, 44, 45, 46, 47],
                [57, 58, 59, 60, 61, 62, 63],
                [72, 73, 74, 75, 76, 77, 78, 79]
            ],
            "ledsLeft": (0..=32).collect::<Vec<i32>>(),
            "ledsRight": (33..=68).collect::<Vec<i32>>()
        },
        "keyboardUnderglow": {
            "rows": 6,
            "columns": 22,
            "ledsLeft": (69..=98).collect::<Vec<i32>>(),
            "ledsRight": (99..=130).collect::<Vec<i32>>()
        },
        "components": {},
        "instructions": raise_update_instructions(),
        "wireless": false,
        "chipId": ""
    });

    // The shipped Raise ANSI backup carries `RGBWMode: false`; the ISO
    // backup carries no `RGBWMode` key at all. Verbatim means verbatim.
    if matches!(keyboard_type, KeyboardType::Ansi) {
        descriptor["RGBWMode"] = serde_json::json!(false);
    }

    descriptor
}

/// The Raise 2 device descriptor a Raise 2-targeting conversion
/// carries: the shipped hardware-virtual keyboard definition's
/// `device` object for the given layout
/// (`bazecor/src/defaultBackups/Raise2ANSI.json` /
/// `Raise2ISO.json` — not backups, the app's simulated factory-state
/// devices; the only Raise 2 artifacts Bazecor ships). Transcribed
/// verbatim, including the virtual-recording fields (`path`,
/// `filePath`) the shipped objects carry; they carry no `chipId` to
/// empty — the offline-conversion identity rule has nothing to clear.
pub fn raise2_device_descriptor(keyboard_type: KeyboardType) -> serde_json::Value {
    let keyboard_leds: (Vec<i32>, Vec<i32>) = match keyboard_type {
        // The ANSI virtual's keyboard LED split runs one LED earlier
        // than the ISO virtual's — shipped as-is on both sides.
        KeyboardType::Ansi => ((0..=31).collect(), (32..=67).collect()),
        KeyboardType::Iso => ((0..=32).collect(), (33..=68).collect()),
    };
    let underglow = match keyboard_type {
        KeyboardType::Ansi => (
            4,
            44,
            (69..=120).collect::<Vec<i32>>(),
            (121..=174).collect::<Vec<i32>>(),
        ),
        KeyboardType::Iso => (
            5,
            29,
            (69..=104).collect::<Vec<i32>>(),
            (105..=143).collect::<Vec<i32>>(),
        ),
    };

    serde_json::json!({
        "info": {
            "vendor": "Dygma",
            "product": "Raise2",
            "keyboardType": keyboard_type.name(),
            "displayName": match keyboard_type {
                KeyboardType::Ansi => "Dygma Raise 2 ANSI",
                KeyboardType::Iso => "Dygma Raise2 ISO",
            },
            "urls": [
                { "name": "Homepage", "url": "https://www.dygma.com/raise2/" }
            ]
        },
        "usb": {
            "vendorId": 13807,
            "productId": match keyboard_type {
                KeyboardType::Ansi => 33,
                KeyboardType::Iso => 35,
            }
        },
        "keyboard": {
            "rows": 5,
            "columns": 16,
            "left": [
                [0, 1, 2, 3, 4, 5, 6],
                [16, 17, 18, 19, 20, 21, 22],
                [32, 33, 34, 35, 36, 37, 38],
                [48, 49, 50, 51, 52, 53, 54],
                [64, 65, 66, 67, 68, 69, 70, 71]
            ],
            "right": [
                [9, 10, 11, 12, 13, 14, 15],
                [24, 25, 26, 27, 28, 29, 30, 31],
                [41, 42, 43, 44, 45, 46, 47],
                [57, 58, 59, 60, 61, 62, 63],
                [72, 73, 74, 75, 76, 77, 78, 79]
            ],
            "ledsLeft": keyboard_leds.0,
            "ledsRight": keyboard_leds.1
        },
        "keyboardUnderglow": {
            "rows": underglow.0,
            "columns": underglow.1,
            "ledsLeft": underglow.2,
            "ledsRight": underglow.3
        },
        "RGBWMode": true,
        "bootloader": false,
        "wireless": true,
        "components": match keyboard_type {
            KeyboardType::Ansi => serde_json::json!({}),
            KeyboardType::Iso => serde_json::json!({ "keymap": {} }),
        },
        "instructions": raise_update_instructions(),
        "path": "VIRTUAL",
        "filePath": match keyboard_type {
            KeyboardType::Ansi => "/Users/minue/Google Drive/Bazecor/VirtualRaise2ANSI.json",
            KeyboardType::Iso => "/Users/minue/Google Drive/Bazecor/VirtualRaise2ISO.json",
        }
    })
}

/// The firmware-update instructions text every shipped Raise and
/// Raise 2 descriptor carries (identical across the four).
fn raise_update_instructions() -> serde_json::Value {
    serde_json::json!({
        "en": {
            "updateInstructions": "To update the firmware, the keyboard needs a special reset. When the countdown starts, press and hold the Escape key. Soon after the countdown finished, the Neuron's light should start a blue pulsing pattern, and the flashing will proceed. At this point, you should release the Escape key."
        }
    })
}

#[cfg(test)]
mod tests;
