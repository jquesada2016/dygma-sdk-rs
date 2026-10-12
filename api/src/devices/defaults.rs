//! Per-product default configurations: the config a keyboard of a
//! given product ships with, before any user edits — the data
//! `config init` writes as the human-readable JSON files.
//!
//! This is the SDK's replacement for Bazecor's *virtual keyboard*
//! mode (its offline `restoreVirtual` flow): rather than simulating a
//! device, it hands the user that device's default config as editable
//! files — `keymap.json`, `superkeys.json`, `macros.json` — which the
//! existing apply paths accept as-is.
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
//! - The **settings** values the virtual keyboards carry (qukeys
//!   250/80, superkeys timings 500/250/20/200/20, LED brightness 153,
//!   mouse 20/2/20/2/2/100/127, idle timeouts 300/300) have a thinner
//!   evidence class: backups capture no settings, so only the
//!   simulation holds them. They are recorded in the expectations
//!   file and not modeled here — this SDK has no settings file
//!   format; settings live on the device (`settings get`/`set`).
//! - The default **palette** and **colormap** are recorded in the
//!   expectations file too, and are not modeled as files here: this
//!   SDK has no human-readable colormap/palette format yet (WS-C).
//!
//! Per-product status:
//!
//! - **Defy** — full default config, typed
//!   ([`DefaultConfig`]), differentially validated against the
//!   recorded expectations (the tests compare this module's output
//!   byte for byte, at the data level, with the recorded Bazecor
//!   defaults).
//! - **Sonsei** — the defaults exist and are recorded (one wireless
//!   variant; four default superkeys, two default macros), but this
//!   SDK models no human-readable Sonsei config files, so there is
//!   nothing to hand back yet: [`default_config`] answers
//!   [`NoDefaultConfigError`].
//! - **Raise / Raise 2** — the virtual keyboards are recorded (each
//!   in ANSI and ISO), but they have no device support, conversion, or
//!   human-readable config model here, and the Raise shipped backups
//!   carry the older padded superkey-map wire form, so the Raise
//!   defaults were never cross-checked against anything current.
//!   [`default_config`] answers [`NoDefaultConfigError`]. Evidence for
//!   them is thin.

use crate::devices::conversions::Product;
use crate::devices::defy::{DefyKeymap, MacroMap, SuperkeyMap};
pub use error::*;

mod error {
    use super::*;

    /// Error returned by [`default_config`]: the product's default
    /// config is either not modeled as human-readable files (the
    /// Sonsei — its defaults are recorded in the differential
    /// expectations) or has none this SDK can stand behind (the Raise
    /// generations). See the [module docs](super) for the per-product
    /// evidence map.
    #[derive(Clone, Copy, Debug, Display, Error, PartialEq, Eq)]
    #[display("no human-readable default config files are modeled for the {product}")]
    pub struct NoDefaultConfigError {
        /// The product that has no modeled default config.
        pub product: Product,
    }
}

/// A product's default configuration: the human-readable config files
/// a keyboard of that product starts its life with, before any user
/// edits — what `config init` writes.
///
/// Every field is a fully-typed map of this SDK's human-readable
/// format: the values round-trip through `serde` (the JSON files) and
/// serialize to exactly the wire shapes the apply paths send. There is
/// no partial or raw form: a [`DefaultConfig`] either exists for a
/// product or [`default_config`] refuses with [`NoDefaultConfigError`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DefaultConfig {
    /// The default keymap — every layer as shipped.
    pub keymap: DefyKeymap,
    /// The default superkeys map.
    pub superkeys: SuperkeyMap,
    /// The default macro map.
    pub macros: MacroMap,
}

/// The default configuration of a Dygma product, in this SDK's
/// human-readable config-file shapes.
///
/// Only the Defy is modeled today (see the [module docs](self) for the
/// evidence per product): the Sonsei's defaults exist and are
/// recorded, but this SDK defines no human-readable Sonsei config
/// files for them to fill; the Raise generations have neither. The
/// error names the product either way.
pub fn default_config(product: Product) -> Result<DefaultConfig, NoDefaultConfigError> {
    match product {
        Product::Defy => Ok(defy_default_config()),
        other => Err(NoDefaultConfigError { product: other }),
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

/// Builds the Defy's typed default config from the embedded evidence
/// data.
///
/// The three parses cannot fail: the constants are the recorded,
/// differentially-validated Bazecor defaults (the tests parse the
/// recorded expectations through the same types and compare), and a
/// mistyped constant would fail those tests on the first run — a
/// panic here would be caught before any release, never in the field.
fn defy_default_config() -> DefaultConfig {
    DefaultConfig {
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
    }
}

#[cfg(test)]
mod tests;
