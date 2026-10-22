//! Per-product default configurations: the config a keyboard of a
//! given product ships with, before any user edits — the data
//! `config init` writes as the human-readable JSON files.
//!
//! This is the SDK's replacement for Bazecor's *virtual keyboard*
//! mode (its offline `restoreVirtual` flow): rather than simulating a
//! device, it hands the user that device's default config as editable
//! files — `keymap.json`, `superkeys.json`, `macros.json`,
//! `palette.json`, and `colormap.json` — which the existing apply
//! paths accept as-is.
//!
//! ## Where a default comes from (the evidence map)
//!
//! There is no firmware-form default to consult: the Defy firmware
//! sketch is not in the local Kaleidoscope checkout (only the Raise's
//! device driver is), and the shipped firmware carries its defaults
//! compiled in — `keymap.default` answers them, but a Defy sketch to
//! read is missing. Per the ground-truth hierarchy (firmware >
//! Bazecor > docs), Bazecor is the best available source, and it holds
//! TWO independent forms of the defaults:
//!
//! 1. **The hardware-virtual keyboards**
//!    (`bazecor/src/api/hardware-virtual/{DefyWired,DefyWireless,
//!    SonseiWireless,RaiseANSI,RaiseISO,Raise2ANSI,Raise2ISO}.ts`) —
//!    the app's simulated factory-state devices, carrying a full
//!    command surface (`keymap.custom`, `superkeys.map`, `macros.map`,
//!    `palette`, `colormap.map`, and the settings).
//! 2. **The shipped default backups**
//!    (`bazecor/src/defaultBackups/*.json`) — captures from real
//!    devices, shipped with the app for its restore-a-default flow.
//!
//! For the Defy and the Sonsei the two sources agree **token for
//! token** on every recorded config command — machine-checked at
//! record time by `tools/bazecor-differential/record-defaults.mjs`,
//! which records both into
//! `tools/bazecor-differential/expectations/default-configs.json`
//! (pinned to the audited Bazecor commit `ff9e10b`). The defaults
//! below cite that agreement; a Rust test re-checks it from the
//! committed fixtures.
//!
//! Known outliers in Bazecor's data, recorded in the expectations
//! file but deliberately NOT replicated here:
//!
//! - The shipped Defy **wired** backup's `keymap.custom` differs from
//!   both Defy virtual keyboards and the wireless backup at wire
//!   positions 72–73 of layer 0 — right-thumb-cluster keys — where it
//!   carries `0 0` and the others carry `54109 54108`, the
//!   bluetooth-pairing and battery-status keys. A lone older capture;
//!   the default used here is the three-sources-agree form.
//! - The virtual keyboards' `keymap.default` entries (the simulated
//!   firmware-side factory layer) are internally inconsistent:
//!   DefyWired's is a 160-token fragment and the Sonsei's is an
//!   800-token Defy-shaped keymap on a 60-keys-per-layer product —
//!   copy-paste drift in Bazecor's definitions. The virtual
//!   keyboards' `keymap.custom` (what a new device actually exposes
//!   as its current config) is the consistent, backup-corroborated
//!   default, and that is what this module ships.
//! - The **settings** values the virtual keyboards carry have a
//!   thinner evidence class: backups capture no settings, so only the
//!   simulation holds them. The numbers differ per product — the Defy
//!   virtuals carry qukeys 250/80, superkeys timings
//!   500/250/20/200/20, LED brightness 153, mouse 20/2/20/2/2/100/127,
//!   idle timeouts 300/300, while the Sonsei virtual's set (qukeys
//!   200/100, superkeys 500/144/20/236/80, LED brightness 102/76,
//!   idle timeouts 600/300, and more) is recorded alongside in the
//!   expectations file. None are modeled here — this SDK has no
//!   settings file format; settings live on the device
//!   (`settings get`/`set`).
//! - The default **palette** and **colormap** are modeled as typed
//!   files too (`palette.json`, `colormap.json`) — WS-C gave them
//!   human-readable formats. The embedded strings are the recorded
//!   Bazecor data verbatim (the palette's trailing separator space
//!   included), the types parse them, and the serializers emit the
//!   same tokens without the trailing separator the device attaches
//!   to reads.
//!
//! Per-product status:
//!
//! - **Defy** — full default config, typed
//!   ([`DefaultConfig::Defy`]), differentially validated against the
//!   recorded expectations (the tests compare this module's output
//!   byte for byte, at the data level, with the recorded Bazecor
//!   defaults).
//! - **Sonsei** — full default config, typed
//!   ([`DefaultConfig::Sonsei`]): one wireless variant; four default
//!   superkeys named by the shipped backup's own store entries —
//!   `⌫/L3`, `↵/L2`, ` 💡/Alt`, `⏯️/L3`, each pairing one tap action
//!   with one hold action (the store names are the backup's own
//!   evidence; the table-decoded wire values are pinned separately in
//!   the tests) — and two default macros (typing "Hello" and "Hola").
//!   The
//!   evidence is the same dual-source agreement as the Defy's, pinned
//!   by the same tests. One known outlier recorded NOT replicated:
//!   the SonseiWireless virtual's `keymap.default` entry is an
//!   800-token Defy-shaped keymap on a 60-key product (copy-paste
//!   drift, listed above); the backup-corroborated `keymap.custom` is
//!   the default shipped here.
//! - **Raise** — full default config, typed per physical layout
//!   ([`DefaultConfig::Raise`]; ANSI and ISO are separate variants —
//!   the layouts' defaults differ at three layer-2 positions).
//!   This is the DUAL-SOURCE evidence, resolved by the unit that
//!   modeled the product: the hardware-virtual definitions and the
//!   SHIPPED BACKUPS (real captures) agree on every config command.
//!   The keymaps, palettes, and colormaps agree token for token
//!   (layout for layout); the superkeys and macros agree at the
//!   PARSED-content level — the sources' streams carry stale EEPROM
//!   bytes and padding after the maps' terminators (both hardware-
//!   virtual definitions, for both maps; the shipped ANSI backup's
//!   macros entry token-identical to its definition's (2048 tokens;
//!   the definition's string carries a trailing separator the
//!   backup's lacks) while its superkeys entry then runs pure
//!   `65535` padding (the live form first); the shipped ISO backup's
//!   entries the live forms plain) — and the parsers,
//!   Bazecor's `split(" 0 0")[0]` and ours, read the same single
//!   clipboard superkey and the same single greeting macro out of
//!   every form. Pinned by the defaults tests at both levels (token
//!   equality where the sources are string-equal, parsed equality
//!   where they are not).
//! - **Raise 2** — full default config, typed per physical layout
//!   ([`DefaultConfig::Raise2`]). No real-device Raise 2 capture
//!   exists, so there is no backup to cross-check against — the
//!   evidence is app-side only, and it is GRADED HONESTLY per
//!   variant:
//!   - **ANSI**: Bazecor's two copies of the virtual keyboard — the
//!     hardware-virtual definition the app's virtual-keyboard mode
//!     serves, and the shipped `{device, virtual}` record — agree
//!     token for token on every config command. Two agreeing
//!     app-side sources, the same class as the settings (no device
//!     capture behind either); the same evidence class the Defy and
//!     Sonsei settings defaults ride on.
//!   - **ISO**: the two copies agree on the superkeys, macros, and
//!     palette token for token, and drift in TWO recorded places.
//!     The keymap: 28 positions of layer 3, every difference a
//!     placeholder swap (`0` in one copy, `65535` transparent in the
//!     other — no real key differs; pinned exactly by test). The
//!     colormap: the shipped record carries a drifted 1440-token
//!     copy (10 × 144) that contradicts both its own descriptor
//!     geometry and the 176-LED layer every other Raise 2 artifact
//!     uses. The defaults ship the DEFINITION's data for both — it
//!     is the copy the app actually serves; the colormap is
//!     internally consistent with its own 4×44 = 176 geometry and
//!     matches the ANSI layout's structure (the two layouts'
//!     colormaps differ at eleven LED positions where the physical
//!     layouts do). The drifted colormap copy is recorded in the
//!     conversions unit's OUTLIER expectations and is REJECTED by
//!     the typed [`Raise2Colormap`] (pinned by test), never
//!     replicated; the keymap drift is a placeholder-writing
//!     difference — no real-key token differs (all 28 differences
//!     are `0`-vs-`65535` tokens — pinned exactly by test, which
//!     implies the typed maps agree outside those slots too).
//!
//!   A Raise 2 default is therefore a single-evidence-class default:
//!   no firmware form exists to consult (the Raise 2 sketch is not
//!   in the local Kaleidoscope checkout), and no device capture
//!   exists to cross-check. The per-command agreement checks that
//!   ARE possible are pinned by the tests.
//!
//! [`default_config`] takes the product AND, for the Raise
//! generations, the physical layout — a
//! [`ConversionTarget`] —
//! because a Raise-generation default is per-layout data: the
//! compiler will not let a caller ask for "the Raise default"
//! without choosing ANSI or ISO. Every product the SDK knows now has
//! a modeled default, so the call is infallible.

use crate::devices::conversions::{ConversionTarget, KeyboardType};
use crate::devices::defy::{DefyColormap, DefyKeymap, MacroMap, SuperkeyMap};
use crate::devices::raise::{RaiseColormap, RaiseKeymap};
use crate::devices::raise2::{Raise2Colormap, Raise2Keymap};
use crate::devices::sonsei::{SonseiColormap, SonseiKeymap};
use crate::focus_api::parsing::palette::{Palette, RgbPalette};

/// A product's default configuration: the human-readable config files
/// a keyboard of that product starts its life with, before any user
/// edits — what `config init` writes.
///
/// Every field of every variant is a fully-typed map of this SDK's
/// human-readable format: the values round-trip through `serde` (the
/// JSON files) and serialize to exactly the wire shapes the apply
/// paths send. There is no partial or raw form: a [`DefaultConfig`]
/// either exists for a product or it does not. The payloads are boxed: each product's
/// config is a multi-kilobyte stack value (ten full layers of typed
/// maps), and the enum would otherwise carry the largest product's
/// whole footprint in every variant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DefaultConfig {
    /// The Defy's default config.
    Defy(Box<DefyDefaultConfig>),
    /// The Sonsei's default config.
    Sonsei(Box<SonseiDefaultConfig>),
    /// The Raise's default config, in one physical layout.
    Raise(Box<RaiseDefaultConfig>),
    /// The Raise 2's default config, in one physical layout.
    Raise2(Box<Raise2DefaultConfig>),
}

/// The Defy's default configuration: the five typed maps
/// `config init` writes for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DefyDefaultConfig {
    /// The default keymap — every layer as shipped.
    pub keymap: DefyKeymap,
    /// The default superkeys map.
    pub superkeys: SuperkeyMap,
    /// The default macro map.
    pub macros: MacroMap,
    /// The default 16-color RGBW palette.
    pub palette: Palette,
    /// The default ten-layer colormap.
    pub colormap: DefyColormap,
}

/// The Sonsei's default configuration: the five typed maps
/// `config init` writes for it. The superkeys and macros maps are the
/// product-independent wire formats; the keymap, colormap, and palette
/// are the Sonsei's own shapes (60 positions, 56 LEDs, RGB).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SonseiDefaultConfig {
    /// The default keymap — every layer as shipped.
    pub keymap: SonseiKeymap,
    /// The default superkeys map.
    pub superkeys: SuperkeyMap,
    /// The default macro map.
    pub macros: MacroMap,
    /// The default 16-color RGB palette.
    pub palette: RgbPalette,
    /// The default ten-layer colormap.
    pub colormap: SonseiColormap,
}

/// A Raise's default configuration: the five typed maps `config
/// init` writes for it, in one physical layout (ANSI or ISO — the
/// layouts' defaults differ at three layer-2 positions, so the
/// keymap and colormap are per-layout; the superkeys, macros, and
/// palette are the same in both).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RaiseDefaultConfig {
    /// The default keymap — every layer as shipped, in this layout.
    pub keymap: RaiseKeymap,
    /// The default superkeys map.
    pub superkeys: SuperkeyMap,
    /// The default macro map.
    pub macros: MacroMap,
    /// The default 16-color RGB palette (`RGBWMode: false`).
    pub palette: RgbPalette,
    /// The default ten-layer colormap, in this layout.
    pub colormap: RaiseColormap,
}

/// A Raise 2's default configuration: the five typed maps `config
/// init` writes for it, in one physical layout (ANSI or ISO — the
/// layouts sit different keys and LED indices at the same wire
/// positions, so the keymap and colormap are per-layout; the
/// superkeys, macros, and palette are the same in both).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Raise2DefaultConfig {
    /// The default keymap — every layer as shipped, in this layout.
    pub keymap: Raise2Keymap,
    /// The default superkeys map.
    pub superkeys: SuperkeyMap,
    /// The default macro map.
    pub macros: MacroMap,
    /// The default 16-color RGBW palette (`RGBWMode: true`).
    pub palette: Palette,
    /// The default ten-layer colormap, in this layout.
    pub colormap: Raise2Colormap,
}

/// The default configuration of a Dygma product, in this SDK's
/// human-readable config-file shapes — every product the SDK knows
/// is modeled (see the [module docs](self) for the evidence per
/// product).
///
/// A Raise-generation default is per physical layout: pass the
/// [`KeyboardType`] with the product, as a
/// [`ConversionTarget`]. The Defy and the Sonsei have a single
/// layout each.
pub fn default_config(target: ConversionTarget) -> DefaultConfig {
    match target {
        ConversionTarget::Defy => DefaultConfig::Defy(Box::new(defy_default_config())),
        ConversionTarget::Sonsei => DefaultConfig::Sonsei(Box::new(sonsei_default_config())),
        ConversionTarget::Raise(keyboard_type) => {
            DefaultConfig::Raise(Box::new(raise_default_config(keyboard_type)))
        }
        ConversionTarget::Raise2(keyboard_type) => {
            DefaultConfig::Raise2(Box::new(raise2_default_config(keyboard_type)))
        }
    }
}

/// The Defy's default keymap data — the factory Base/Navigation/FKeys
/// layout, all ten layers, verbatim from the evidence sources.
///
/// One data-level detail, handled honestly: the ten wire-padding
/// positions the [`crate::devices::defy::LAYOUT`] does not model (7,
/// 8, 23, 24, 39, 40, 54–57) are not keys, and the recorded default's
/// layers 3–10 carry `65535` (transparent) in them where layers 1–2
/// carry `0`. The typed [`DefyKeymap`] structurally cannot hold those
/// slots, and the serializer normalizes them to `0` — the value this
/// SDK's keymap serializer has always emitted for padding, and what a
/// real Defy write carries. So the default config this module builds
/// matches the recorded default on all 70 modeled keys of every
/// layer, byte for byte, and differs from the recorded *string* only
/// at those 10 non-key slots per layer (`65535` → `0` on layers 3–10)
/// — a normalization pinned exactly by the tests.
const DEFY_DEFAULT_KEYMAP_DATA: &str = "41 30 31 32 33 34 45 0 0 46 35 36 37 38 39 42 43 20 26 8 21 23 47 0 0 48 28 24 12 18 19 76 57 4 22 7 9 10 17152 0 0 49 11 13 14 15 51 52 225 29 27 6 25 5 0 0 0 0 17 16 54 55 56 229 53980 17452 44 49211 49721 227 0 0 54109 54108 101 49209 49162 44 230 53852 41 0 0 0 0 0 0 0 0 0 83 0 84 85 46 42 43 0 74 82 77 75 0 0 0 0 0 95 96 97 86 0 0 70 80 81 79 78 0 0 0 0 0 92 93 94 87 0 225 0 0 0 0 0 0 0 0 0 99 89 90 91 88 0 0 0 65535 65535 65535 65535 0 0 0 0 0 0 65535 98 0 0 53 58 59 60 61 62 63 65535 65535 64 65 66 67 68 69 0 0 0 0 22710 22709 23785 0 65535 65535 0 0 23663 0 0 65535 0 0 0 22713 22711 22733 23786 0 65535 65535 0 0 23664 20866 20865 0 0 0 0 0 0 0 19682 65535 65535 65535 65535 0 0 0 0 0 0 0 65535 0 0 0 0 0 0 0 0 0 0 0 0 0 0 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535";

/// The Defy's default superkeys map data, verbatim: ONE superkey, on
/// superkey 1 — the factory clipboard superkey, tap `262` (Ctrl+C),
/// hold `281` (Ctrl+V), tap-hold `2329` (Ctrl+Shift+V), double-tap
/// `283` (Ctrl+X), and no double-tap-hold action (`1` is the wire's
/// "no action" value). The trailing `0 0` is the superkey terminator
/// and the map terminator.
const DEFY_DEFAULT_SUPERKEYS_DATA: &str = "262 281 2329 283 1 0 0";

/// The Defy's default macro map data, verbatim: ONE macro — the
/// factory macro, which types "Hey, dygmate": RightShift held while
/// tapping `H`, released; `E` `Y` `,` `Space` tapped; RightShift held
/// while tapping `D`, released; `Y` `G` `M` `A` `T` `E` tapped.
/// Opcodes `6`/`7`/`8` are keycode-down/up/tap, each carrying one u8
/// keycode. The trailing `0 0` is the macro terminator and the map
/// terminator.
const DEFY_DEFAULT_MACROS_DATA: &str =
    "6 229 8 11 7 229 8 8 8 28 8 54 8 44 6 229 8 7 7 229 8 28 8 10 8 16 8 4 8 23 8 8 0 0";

/// The Defy's default palette, verbatim: the 16 factory RGBW colors,
/// 64 decimals, exactly as recorded (the recorded wireless virtual
/// keyboard's string, trailing separator space included — the device
/// ends every value it sends with one; the wired virtual's copy is
/// token-identical without it).
///
/// Color 0 is the warm amber framing the factory Base layer, color 2
/// is pure white through the white channel alone (`0 0 0 255`), and
/// every color here carries a zero in at least one colored channel —
/// which is also why Bazecor's lossy app round trip happens to
/// reproduce these exact bytes (see the
/// [`RgbwColor`][crate::focus_api::parsing::palette::RgbwColor] docs).
const DEFY_DEFAULT_PALETTE_DATA: &str = "255 196 0 0 0 254 24 0 0 0 0 255 231 255 0 0 0 254 234 0 0 52 255 0 255 0 232 0 0 77 168 87 125 0 235 19 20 0 36 219 85 0 126 129 255 9 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 ";

/// The Defy's default colormap, verbatim: ten layers of 178 palette
/// indices painting the factory theme — the amber/silver Base layer,
/// and the per-layer accent colors the factory ships.
const DEFY_DEFAULT_COLORMAP_DATA: &str = "10 4 4 4 4 4 7 10 4 4 4 4 4 7 2 4 4 4 4 4 0 2 4 4 4 4 4 10 3 1 5 8 2 0 0 5 4 4 4 4 4 7 11 4 4 4 4 4 7 7 7 4 4 4 4 7 2 7 7 7 4 4 10 2 1 3 8 2 6 6 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 10 12 12 12 12 12 12 10 12 9 3 9 10 12 12 1 3 3 3 10 12 2 12 12 12 12 12 12 12 1 6 5 2 12 12 5 0 0 0 12 11 12 12 0 3 3 3 12 12 12 0 3 0 3 12 12 12 6 3 3 3 3 12 12 9 3 12 12 12 12 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 7 4 8 8 8 8 8 8 15 12 15 7 7 9 15 15 12 3 0 1 9 15 15 15 15 15 15 9 15 8 15 12 15 15 15 15 15 8 8 8 8 8 8 15 15 15 12 3 15 15 15 12 11 0 3 12 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 3 6 6 6 6 6 3 3 6 6 6 6 6 3 3 6 6 6 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15";

/// The Sonsei's default keymap data — the factory layout, all ten
/// layers, verbatim from the evidence sources.
///
/// One data-level detail, handled exactly like the Defy's (see
/// [`DEFY_DEFAULT_KEYMAP_DATA`]): the four wire positions the
/// [`crate::devices::sonsei::LAYOUT`] does not model (52–55, the
/// thumb row's center) are not keys, and the recording carries
/// `65535` (transparent) in them on layers 2–10 and `0` on layer 1.
/// The typed [`SonseiKeymap`] structurally cannot hold those slots,
/// and the serializer normalizes them to `0` — so the default config
/// this module builds matches the recorded default on all 56 modeled
/// keys of every layer, byte for byte, and differs from the recorded
/// *string* only at those 4 non-key slots per layer (`65535` → `0` on
/// layers 2–10) — a normalization pinned exactly by the tests.
const SONSEI_DEFAULT_KEYMAP_DATA: &str = "41 30 31 32 33 34 35 36 37 38 39 42 43 20 26 8 21 23 28 24 12 18 19 40 53983 4 22 7 9 10 11 13 14 15 51 52 225 29 27 6 25 5 17 16 54 55 56 229 224 227 44 53981 0 0 0 0 53980 44 53982 54109 53 0 0 0 0 0 83 46 84 85 86 65535 65535 2094 46 2093 45 0 0 95 96 97 87 65535 65535 0 49 47 48 0 0 92 93 94 88 0 65535 65535 2097 2095 2096 0 0 89 90 91 99 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 98 65535 65535 65535 58 59 60 61 62 63 64 65 66 67 68 65535 20865 22710 22709 23785 0 75 74 82 77 0 69 65535 20866 54108 22733 23786 0 78 80 81 79 70 0 65535 65535 17154 17153 17152 0 0 0 0 0 0 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535";

/// The Sonsei's default superkeys map data, verbatim: FOUR superkeys
/// — the factory key-pattern set (each: one action on tap, one on
/// hold, `1` — the wire's "no action" — everywhere else), each with
/// its `0` terminator, then the map's own terminating `0`. The exact
/// actions are decoded through the types and pinned by the tests.
const SONSEI_DEFAULT_SUPERKEYS_DATA: &str =
    "42 17452 1 1 1 0 40 17451 1 1 1 0 17154 230 1 1 1 0 57 17452 1 1 1 0 0";

/// The Sonsei's default macro map data, verbatim: TWO macros — the
/// factory greeting pair, typing "Hello" and "Hola": LeftShift held
/// while tapping `H`, released; the remaining letters tapped.
/// Opcodes `6`/`7`/`8` are keycode-down/up/tap, each carrying one u8
/// keycode. Each macro's terminator `0`, then the map's own.
const SONSEI_DEFAULT_MACROS_DATA: &str =
    "6 225 8 11 7 225 8 8 8 15 8 15 8 18 0 6 225 8 11 7 225 8 18 8 15 8 4 0 0";

/// The Sonsei's default palette, verbatim: the 16 factory RGB colors,
/// 48 decimals (16 × 3 — the `RGBWMode: false` wire shape), exactly as
/// recorded. Color 0 is the white the factory theme frames its keys
/// with — the recorded colormap's default color for converted
/// positions (see the conversions module).
const SONSEI_DEFAULT_PALETTE_DATA: &str = "255 255 255 255 0 0 255 255 0 255 108 0 255 195 0 138 255 0 189 16 224 144 19 254 0 118 255 0 254 234 255 0 255 214 129 255 0 0 0 0 0 0 0 0 0 0 0 0";

/// The Sonsei's default colormap, verbatim: ten layers of 56 palette
/// indices painting the factory theme.
const SONSEI_DEFAULT_COLORMAP_DATA: &str = "4 0 0 0 0 0 0 0 0 0 0 3 4 0 0 0 0 0 0 0 0 0 0 5 4 0 0 0 0 0 0 0 0 0 0 0 4 0 0 0 0 0 0 0 0 0 0 4 4 4 0 5 3 0 4 8 0 15 15 15 15 15 6 4 4 4 4 15 15 0 0 0 0 14 15 0 0 0 4 15 15 15 8 9 9 14 15 0 9 0 5 15 15 15 8 5 5 15 15 0 0 0 4 15 15 15 15 15 15 0 15 15 15 0 0 0 0 0 0 0 0 0 0 0 15 3 8 8 9 14 3 3 2 3 15 0 15 3 5 9 9 15 3 2 2 2 8 15 15 15 2 2 2 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15";

/// The Raise's default keymap data (the ANSI layout) — the factory layout, all ten layers, verbatim from the evidence sources.
const RAISE_ANSI_DEFAULT_KEYMAP_DATA: &str = "41 30 31 32 33 34 35 0 0 36 37 38 39 45 46 42 43 20 26 8 21 23 0 0 28 24 12 18 19 47 48 40 57 4 22 7 9 10 0 0 0 11 13 14 15 51 52 49 225 100 29 27 6 25 5 0 0 0 17 16 54 55 56 229 224 227 226 44 44 0 42 51514 49162 76 44 44 230 231 101 228 53 58 59 60 61 62 63 0 0 64 65 66 67 68 69 65535 43 53980 74 82 77 75 0 0 86 95 96 97 84 83 0 65535 49226 70 80 81 79 78 0 0 0 87 92 93 94 85 53852 0 225 0 22710 22709 23786 23785 22733 0 0 0 98 89 90 91 99 82 224 227 226 44 44 0 42 51514 65535 17152 44 44 230 80 81 79 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535";

/// The Raise's default keymap data (the ISO layout) — the factory layout, all ten layers, verbatim from the evidence sources. Differs from the ANSI data at exactly three layer-2 positions (17, 46, 49 — the ANSI and ISO data sit Superkey-1, Macro-1, and transparent codes differently there; pinned by the defaults tests).
const RAISE_ISO_DEFAULT_KEYMAP_DATA: &str = "41 30 31 32 33 34 35 0 0 36 37 38 39 45 46 42 43 20 26 8 21 23 0 0 28 24 12 18 19 47 48 40 57 4 22 7 9 10 0 0 0 11 13 14 15 51 52 49 225 100 29 27 6 25 5 0 0 0 17 16 54 55 56 229 224 227 226 44 44 0 42 51514 49162 76 44 44 230 231 101 228 53 58 59 60 61 62 63 0 0 64 65 66 67 68 69 65535 43 53852 74 82 77 75 0 0 86 95 96 97 84 83 0 65535 49226 70 80 81 79 78 0 0 0 87 92 93 94 85 0 0 225 53980 22710 22709 23786 23785 22733 0 0 0 98 89 90 91 99 82 224 227 226 44 44 0 42 51514 65535 17152 44 44 230 80 81 79 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 ";

/// The Raise's default superkeys map data, verbatim (the parsed-content form both layouts share): ONE superkey, the factory clipboard superkey — tap 262 (Ctrl+C), hold 281 (Ctrl+V), no tap-hold, double-tap, or double-tap-hold action (1 is the wire's no-action value), then the superkey terminator and the map terminator. Every evidence source carries this same single superkey; the recorded definitions (both layouts) carry stale EEPROM bytes then 65535 padding after the map's 0 0 terminator — the shipped ANSI backup's entry then running pure 65535 padding after its live form, the shipped ISO backup's entry the live form plain — which the superkey wire format's parser (Bazecor's and ours) ignores. Embedded in the live form the serializers emit.
const RAISE_DEFAULT_SUPERKEYS_DATA: &str = "262 281 1 1 1 0 0";

/// The Raise's default macro map data (the parsed-content form both layouts share): ONE macro — the factory greeting, typing 'Hey, Dygmates' (LeftShift held while tapping H, released; then E, Y, comma, space; LeftShift held while tapping D, released; then Y, G, M, A, T, E, S). The macro's terminator 0, then the map's own. The recorded definitions (both layouts) and the shipped ANSI backup's entry (token-identical to its definition's — 2048 tokens; the definition's string carries a trailing separator the backup's lacks) carry stale EEPROM bytes then 255 eraser padding after the map's terminator; the shipped ISO backup's entry is the live form plain — which the macro wire format's parser ignores. Embedded in the live form the serializers emit.
const RAISE_DEFAULT_MACROS_DATA: &str =
    "6 225 8 11 7 225 8 8 8 28 8 54 8 44 6 225 8 7 7 225 8 28 8 10 8 16 8 4 8 23 8 8 8 22 0 0";

/// The Raise's default palette, verbatim: the 16 factory RGB colors, 48 decimals (16 × 3 — the RGBWMode: false wire shape), identical in the ANSI and ISO sources (the recorded strings differ only in a trailing separator space, a device-read artifact). Color 0 is the amber the factory theme frames its keys with.
const RAISE_DEFAULT_PALETTE_DATA: &str = "255 196 0 0 254 24 255 255 255 231 255 0 0 254 234 0 52 255 255 0 232 87 164 255 144 19 254 239 219 255 214 129 255 255 9 0 0 0 0 0 0 0 0 0 0 0 0 0";

/// The Raise's default colormap data (the ANSI layout), verbatim: ten layers of 132 palette indices (69 keyboard LEDs + 30 left + 33 right underglow) painting the factory theme.
const RAISE_ANSI_DEFAULT_COLORMAP_DATA: &str = "4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 1 1 5 6 5 4 4 4 4 4 4 6 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 1 1 10 3 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 8 8 8 8 8 8 4 3 9 0 9 10 7 8 0 0 0 10 4 15 7 7 4 4 1 4 4 4 1 1 5 6 5 8 8 8 8 8 8 6 15 11 10 9 9 9 10 15 3 10 9 9 9 10 0 10 9 9 9 9 0 0 0 4 1 1 11 3 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15";

/// The Raise's default colormap data (the ISO layout), verbatim: ten layers of 132 palette indices painting the factory theme. Differs from the ANSI data at exactly three positions of layer 2, where the physical layouts sit different keys.
const RAISE_ISO_DEFAULT_COLORMAP_DATA: &str = "4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 1 1 5 6 5 4 4 4 4 4 4 6 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 1 1 10 3 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 8 8 8 8 8 8 4 6 9 0 9 10 7 8 0 0 0 10 4 6 7 7 4 4 1 4 4 4 1 1 5 6 5 8 8 8 8 8 8 6 15 11 10 9 9 9 10 15 15 10 9 9 9 10 0 10 9 9 9 9 0 0 0 4 1 1 11 3 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 ";

/// The Raise 2's default keymap data (the ANSI layout) — the factory layout, all ten layers, verbatim from the evidence sources (the two artifact families agree token for token).
const RAISE2_ANSI_DEFAULT_KEYMAP_DATA: &str = "41 30 31 32 33 34 35 0 0 36 37 38 39 45 46 42 43 20 26 8 21 23 0 0 28 24 12 18 19 47 48 49 57 4 22 7 9 10 0 0 0 11 13 14 15 51 52 40 225 225 29 27 6 25 5 0 0 0 17 16 54 55 56 229 224 227 226 44 44 51770 51516 51770 49162 51806 44 44 230 231 101 228 65535 0 0 0 0 0 0 0 0 46 84 85 86 0 0 65535 65535 0 74 82 77 75 0 0 43 95 96 97 87 0 0 0 65535 70 80 81 79 78 0 0 0 83 92 93 94 88 0 65535 65535 65535 0 0 0 0 0 0 0 0 98 89 90 91 99 82 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 80 81 79 53 58 59 60 61 62 63 0 0 64 65 66 67 68 69 65535 65535 0 0 22710 22709 23785 0 0 0 23663 0 0 0 0 0 0 65535 0 22713 22711 22733 23786 0 0 0 0 23664 20866 20865 0 0 65535 65535 65535 0 0 54109 54108 19682 0 0 0 0 17154 17153 17152 0 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535";

/// The Raise 2's default keymap data (the ISO layout) — the factory layout, all ten layers, verbatim from the hardware-virtual DEFINITION, per the recorded-drifts policy (the shipped record's copy differs at 28 placeholder positions of layer 3 — see the module docs' Raise 2 ISO bullet).
const RAISE2_ISO_DEFAULT_KEYMAP_DATA: &str = "41 30 31 32 33 34 35 0 0 36 37 38 39 45 46 42 43 20 26 8 21 23 0 0 28 24 12 18 19 47 48 40 57 4 22 7 9 10 0 0 0 11 13 14 15 51 52 49 225 100 29 27 6 25 5 0 0 0 17 16 54 55 56 229 224 227 226 44 44 51770 51516 51770 49162 51806 44 44 230 231 101 228 65535 0 0 0 0 0 0 0 0 46 84 85 86 0 0 65535 65535 0 74 82 77 75 0 0 43 95 96 97 87 0 0 65535 65535 70 80 81 79 78 0 0 0 83 92 93 94 88 0 0 65535 0 0 0 0 0 0 0 0 0 98 89 90 91 99 82 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 80 81 79 53 58 59 60 61 62 63 0 0 64 65 66 67 68 69 65535 65535 0 0 22710 22709 23785 0 0 0 23663 0 0 0 0 0 65535 65535 0 22713 22711 22733 23786 0 0 0 0 23664 20866 20865 0 0 0 65535 0 0 0 54109 54108 19682 0 0 0 0 17154 17153 17152 0 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535";

/// The Raise 2's default superkeys map data (the parsed-content form both layouts share), verbatim: ONE superkey, the factory clipboard superkey — tap Ctrl+C, hold Ctrl+V, no other action — then the terminators; the rest of the recorded stream is 65535 padding to the 512-slot memory. Embedded in the live form the serializers emit.
const RAISE2_DEFAULT_SUPERKEYS_DATA: &str = "262 281 1 1 1 0 0";

/// The Raise 2's default macro map data (the parsed-content form both layouts share): ONE macro — a tap of space, then the Raise's factory greeting, 'Hey, Dygmates' (pinned action for action by the tests). Embedded in the live form the serializers emit; the recorded stream's tail past the map's terminator is stale EEPROM bytes then the 255 eraser padding to the 2048-byte memory, which the macro wire format's parser ignores.
const RAISE2_DEFAULT_MACROS_DATA: &str =
    "8 44 6 225 8 11 7 225 8 8 8 28 8 54 8 44 6 225 8 7 7 225 8 28 8 10 8 16 8 4 8 23 8 8 8 22 0 0";

/// The Raise 2's default palette, verbatim: the 16 factory RGBW colors, 64 decimals (16 × 4 — the RGBWMode: true wire shape), identical in the ANSI and ISO sources and in both artifact families.
const RAISE2_DEFAULT_PALETTE_DATA: &str = "250 255 0 0 0 254 24 0 0 0 0 255 255 152 0 0 0 254 234 0 0 142 217 34 255 0 232 0 255 0 4 0 125 0 235 19 20 0 36 219 85 0 126 129 255 9 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0";

/// The Raise 2's default colormap data (the ANSI layout), verbatim: ten layers of 176 palette indices (69 keyboard LEDs + 53 left + 54 right underglow) painting the factory theme.
const RAISE2_ANSI_DEFAULT_COLORMAP_DATA: &str = "3 4 4 4 4 4 4 2 4 4 4 4 4 2 4 4 4 4 4 2 2 4 4 4 4 4 2 2 2 1 1 8 0 3 4 4 4 4 4 4 4 4 4 4 4 4 4 4 3 4 4 4 4 4 4 2 4 4 4 4 4 2 2 2 2 1 1 8 0 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 3 15 15 15 15 15 15 2 15 9 4 9 10 2 6 4 4 4 10 2 2 15 15 15 15 15 2 2 2 1 1 8 0 3 15 15 8 8 8 8 15 15 15 8 0 0 0 2 8 15 10 0 8 0 10 4 8 0 0 0 0 4 4 4 2 1 1 8 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 4 8 8 8 8 8 8 2 15 15 5 5 2 2 15 0 3 1 2 2 2 15 15 7 8 10 2 2 2 2 2 8 0 2 8 8 8 8 8 8 15 15 15 15 15 15 0 15 2 15 15 11 3 0 15 2 15 2 2 10 15 2 2 2 2 2 2 8 0 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15";

/// The Raise 2's default colormap data (the ISO layout), verbatim: ten layers of 176 palette indices painting the factory theme. Differs from the ANSI data at eleven LED positions of layers 1–3, where the physical layouts sit different keys. This is the hardware-virtual DEFINITION's data (what the app's virtual-keyboard mode serves); the shipped ISO RECORD carries a drifted 1440-token copy instead — see the raise2 module's evidence map.
const RAISE2_ISO_DEFAULT_COLORMAP_DATA: &str = "3 4 4 4 4 4 4 2 4 4 4 4 4 2 4 4 4 4 4 2 4 4 4 4 4 4 2 2 2 1 1 8 0 3 4 4 4 4 4 4 3 4 4 4 4 4 4 4 4 4 4 4 4 4 4 2 4 4 4 4 4 2 2 2 2 1 1 8 0 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 3 15 15 15 15 15 15 2 15 9 4 9 10 2 6 4 4 4 10 2 15 15 15 15 15 15 2 2 2 1 1 8 0 3 15 15 8 8 8 8 8 15 15 8 0 0 0 2 15 15 10 0 8 0 10 4 8 0 0 0 0 4 4 4 2 1 1 8 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 4 8 8 8 8 8 8 2 15 15 5 5 2 2 15 0 3 1 2 2 15 15 15 11 6 10 2 2 2 2 2 8 0 2 8 8 8 8 8 8 2 15 15 15 15 15 0 15 15 15 15 11 3 0 15 2 15 2 2 10 15 2 2 2 2 2 2 8 0 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 8 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15";

/// Builds the Defy's typed default config from the embedded evidence
/// data.
///
/// The parses cannot fail: the constants are the recorded,
/// differentially-validated Bazecor defaults (the tests parse the
/// recorded expectations through the same types and compare), and a
/// mistyped constant would fail those tests on the first run — a
/// panic here would be caught before any release, never in the field.
fn defy_default_config() -> DefyDefaultConfig {
    DefyDefaultConfig {
        keymap: DEFY_DEFAULT_KEYMAP_DATA.parse().expect(
            "the embedded default keymap data is the differentially-validated \
             Bazecor default, pinned by the defaults tests",
        ),
        superkeys: DEFY_DEFAULT_SUPERKEYS_DATA.parse().expect(
            "the embedded default superkeys data is the differentially-validated \
             Bazecor default, pinned by the defaults tests",
        ),
        macros: DEFY_DEFAULT_MACROS_DATA.parse().expect(
            "the embedded default macros data is the differentially-validated \
             Bazecor default, pinned by the defaults tests",
        ),
        palette: DEFY_DEFAULT_PALETTE_DATA.parse().expect(
            "the embedded default palette data is the differentially-validated \
             Bazecor default, pinned by the defaults tests",
        ),
        colormap: DEFY_DEFAULT_COLORMAP_DATA.parse().expect(
            "the embedded default colormap data is the differentially-validated \
             Bazecor default, pinned by the defaults tests",
        ),
    }
}

/// Builds the Sonsei's typed default config from the embedded evidence
/// data.
///
/// The parses cannot fail: the constants are the recorded,
/// differentially-validated Bazecor defaults (the tests parse the
/// recorded expectations through the same types and compare), and a
/// mistyped constant would fail those tests on the first run — a
/// panic here would be caught before any release, never in the field.
fn sonsei_default_config() -> SonseiDefaultConfig {
    SonseiDefaultConfig {
        keymap: SONSEI_DEFAULT_KEYMAP_DATA.parse().expect(
            "the embedded default keymap data is the differentially-validated \
             Bazecor default, pinned by the defaults tests",
        ),
        superkeys: SONSEI_DEFAULT_SUPERKEYS_DATA.parse().expect(
            "the embedded default superkeys data is the differentially-validated \
             Bazecor default, pinned by the defaults tests",
        ),
        macros: SONSEI_DEFAULT_MACROS_DATA.parse().expect(
            "the embedded default macros data is the differentially-validated \
             Bazecor default, pinned by the defaults tests",
        ),
        palette: SONSEI_DEFAULT_PALETTE_DATA.parse().expect(
            "the embedded default palette data is the differentially-validated \
             Bazecor default, pinned by the defaults tests",
        ),
        colormap: SONSEI_DEFAULT_COLORMAP_DATA.parse().expect(
            "the embedded default colormap data is the differentially-validated \
             Bazecor default, pinned by the defaults tests",
        ),
    }
}

/// Builds the Raise's typed default config for one physical layout
/// from the embedded evidence data.
///
/// The parses cannot fail: the constants are the recorded,
/// differentially-validated Bazecor defaults (the tests parse the
/// recorded expectations through the same types and compare — token
/// for token where the sources agree, modulo the recorded trailing
/// separator, and as parsed maps where the sources carry the same
/// content in different padding forms), and a mistyped constant
/// would fail those tests on the first run — a panic here would be
/// caught before any release, never in the field.
fn raise_default_config(keyboard_type: KeyboardType) -> RaiseDefaultConfig {
    let (keymap_data, colormap_data) = match keyboard_type {
        KeyboardType::Ansi => (
            RAISE_ANSI_DEFAULT_KEYMAP_DATA,
            RAISE_ANSI_DEFAULT_COLORMAP_DATA,
        ),
        KeyboardType::Iso => (
            RAISE_ISO_DEFAULT_KEYMAP_DATA,
            RAISE_ISO_DEFAULT_COLORMAP_DATA,
        ),
    };

    RaiseDefaultConfig {
        keymap: keymap_data.parse().expect(
            "the embedded default keymap data is the differentially-validated \
             Bazecor default, pinned by the defaults tests",
        ),
        superkeys: RAISE_DEFAULT_SUPERKEYS_DATA.parse().expect(
            "the embedded default superkeys data is the differentially-validated \
             Bazecor default, pinned by the defaults tests",
        ),
        macros: RAISE_DEFAULT_MACROS_DATA.parse().expect(
            "the embedded default macros data is the differentially-validated \
             Bazecor default, pinned by the defaults tests",
        ),
        palette: RAISE_DEFAULT_PALETTE_DATA.parse().expect(
            "the embedded default palette data is the differentially-validated \
             Bazecor default, pinned by the defaults tests",
        ),
        colormap: colormap_data.parse().expect(
            "the embedded default colormap data is the differentially-validated \
             Bazecor default, pinned by the defaults tests",
        ),
    }
}

/// Builds the Raise 2's typed default config for one physical layout
/// from the embedded evidence data.
///
/// The parses cannot fail, exactly as the other products' (see
/// [`raise_default_config`]): the constants are the recorded Bazecor
/// defaults the tests pin.
fn raise2_default_config(keyboard_type: KeyboardType) -> Raise2DefaultConfig {
    let (keymap_data, colormap_data) = match keyboard_type {
        KeyboardType::Ansi => (
            RAISE2_ANSI_DEFAULT_KEYMAP_DATA,
            RAISE2_ANSI_DEFAULT_COLORMAP_DATA,
        ),
        KeyboardType::Iso => (
            RAISE2_ISO_DEFAULT_KEYMAP_DATA,
            RAISE2_ISO_DEFAULT_COLORMAP_DATA,
        ),
    };

    Raise2DefaultConfig {
        keymap: keymap_data.parse().expect(
            "the embedded default keymap data is the differentially-validated \
             Bazecor default, pinned by the defaults tests",
        ),
        superkeys: RAISE2_DEFAULT_SUPERKEYS_DATA.parse().expect(
            "the embedded default superkeys data is the differentially-validated \
             Bazecor default, pinned by the defaults tests",
        ),
        macros: RAISE2_DEFAULT_MACROS_DATA.parse().expect(
            "the embedded default macros data is the differentially-validated \
             Bazecor default, pinned by the defaults tests",
        ),
        palette: RAISE2_DEFAULT_PALETTE_DATA.parse().expect(
            "the embedded default palette data is the differentially-validated \
             Bazecor default, pinned by the defaults tests",
        ),
        colormap: colormap_data.parse().expect(
            "the embedded default colormap data is the differentially-validated \
             Bazecor default, pinned by the defaults tests",
        ),
    }
}

#[cfg(test)]
mod tests;
