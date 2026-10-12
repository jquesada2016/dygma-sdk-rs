//! Types for parsing and serializing the `colormap.map` command's wire
//! format: the per-layer palette indices that paint every LED.
//!
//! The Defy's map is layer-major, [`LEDS_PER_LAYER`] LEDs per layer
//! (70 keyboard LEDs, then 108 underglow), one decimal per LED. The
//! decimals are palette indices, and they are nibble-sized by
//! construction: the firmware stores the map two indices per EEPROM
//! byte (`kaleidoscope/plugin/LED-Palette-Theme.cpp`,
//! `lookupColorIndexAtPosition`/`updateColorIndexAtPosition`) and its
//! Focus handler composes each stored byte as `(first << 4) + second`
//! from the decimal pair it reads — so a decimal above 15 would carry
//! into its neighbor and corrupt both. Every real Defy capture stays
//! within that range (the shipped Bazecor Defy backups' `colormap.map`
//! entries are 1780 decimals, all 0–15). [`PaletteIndex`] is that
//! 0–15 domain as a type.
//!
//! How many layers a map holds is the *domain's* business, not this
//! wire format's: like the keymap wire format, the parser here accepts
//! any whole number of complete layers (the device answers with all
//! ten of its layers; a write updates layers until the data runs out),
//! and the exactly-ten write shape a Defy carries is enforced by the
//! map type at the devices layer (`DefyColormap`).

use std::str::FromStr;

use winnow::{
    ModalResult, Parser,
    ascii::dec_uint,
    combinator::{cut_err, fail, opt, repeat, terminated},
    error::{StrContext, StrContextValue},
};

use crate::focus_api::parsing::parse_expectation;

/// What the range rejection's cut carries in its context, by which the
/// FromStr mapping recognizes it (the same trick the macros parser's
/// unsupported-kind rejection uses).
const RANGE_EXPECTATION: &str = "a palette index (0-15)";

/// How many LEDs one colormap layer paints, on the Defy: 70 keyboard
/// LEDs followed by 108 underglow LEDs (the shipped backup device
/// descriptor's `keyboard.ledsLeft`/`ledsRight` and
/// `keyboardUnderglow.ledsLeft`/`ledsRight` arrays cover 0–69 and
/// 70–175 respectively, contiguously; positions 176–177 are named by
/// no array in that descriptor — Bazecor's runtime hardware
/// descriptors extend the underglow arrays through 177 — and the
/// underglow grid's 2 × 89 extent is what makes the layer 178).
pub const LEDS_PER_LAYER: usize = 178;

/// How many of one layer's LED positions are keyboard LEDs (positions
/// 0–69 of the layer).
pub const KEYBOARD_LEDS: usize = 70;

/// How many of one layer's LED positions are underglow LEDs (positions
/// 70–177 of the layer).
pub const UNDERGLOW_LEDS: usize = LEDS_PER_LAYER - KEYBOARD_LEDS;

/// Error when parsing colormap wire data.
///
/// Carries the byte offset of the failure within the input the parser
/// was given, so errors can point at the offending part of the data
/// instead of quoting it whole.
#[derive(Clone, Debug, Display, Error)]
#[display("failed to parse colormap data at byte offset {offset}: expected {expected}")]
pub struct ParseColormapError {
    /// Byte offset of the failure within the parsed input.
    ///
    /// This is the start of the token that could not be parsed, or the
    /// start of the layer that failed when the failure is mid-layer
    /// (walk the tokens forward from there to find the first invalid
    /// one) — except when an out-of-range palette index is rejected:
    /// the parser has already consumed that token, so the offset sits
    /// just past it (see [`Self::consumed_offender`]).
    pub offset: usize,
    /// What the parser expected at `offset`, from the parser's own
    /// context (e.g. `a palette index`).
    pub expected: String,
    /// Whether the failure is an out-of-range palette index rejection.
    ///
    /// That rejection is the one failure whose offset does not sit on
    /// the offending token: the index decimal is consumed before it is
    /// rejected (a `16` is a well-formed number — it is just not a
    /// storable palette index), so `offset` marks where parsing
    /// stopped, just past the offender.
    pub consumed_offender: bool,
}

/// A palette index: one colormap LED position's color, 0–15.
///
/// The inner value is private so anything above 15 is unrepresentable:
/// the firmware packs indices two per EEPROM byte as
/// `(first << 4) + second`, so a larger index is not a storable color
/// choice but a corruption of its neighbor (see the [module
/// docs](self)). Construct values with [`PaletteIndex::new`],
/// [`PaletteIndex::MAX`], or `FromStr`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PaletteIndex(u8);

impl PaletteIndex {
    /// The largest index a palette position can carry: the palette has
    /// [`COLORS`](crate::focus_api::parsing::palette::COLORS) colors,
    /// so 15 is the top.
    pub const MAX: u8 = 15;

    /// The smallest index a palette position can carry.
    pub const MIN: u8 = 0;

    /// Creates an index, returning `None` when the value exceeds
    /// [`PaletteIndex::MAX`] and thus cannot be stored by the
    /// firmware.
    pub fn new(value: u8) -> Option<Self> {
        (value <= Self::MAX).then_some(Self(value))
    }

    /// The index value, 0–15.
    pub const fn get(self) -> u8 {
        self.0
    }
}

impl FromStr for PaletteIndex {
    type Err = ParseColormapError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        index_parser.parse(s).map_err(|err| {
            let expected = parse_expectation(err.inner().context())
                .unwrap_or_else(|| "a palette index".to_owned());

            ParseColormapError {
                offset: err.offset(),
                consumed_offender: expected == RANGE_EXPECTATION,
                expected,
            }
        })
    }
}

impl serde::Serialize for PaletteIndex {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for PaletteIndex {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = u8::deserialize(deserializer)?;

        // Checked at the JSON boundary like `FromStr` checks it at the
        // wire boundary: a config file holding an unstoreable index
        // cannot come into existence as a [`PaletteIndex`] either, so
        // every consumer of the type can rely on the firmware's
        // nibble domain.
        Self::new(value).ok_or_else(|| {
            serde::de::Error::custom(format!(
                "palette index {value} is out of range: indices go up to {}",
                Self::MAX
            ))
        })
    }
}

/// One raw colormap layer: [`LEDS_PER_LAYER`] palette indices, in wire
/// order (keyboard LEDs first, then underglow).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColormapLayer(pub [PaletteIndex; LEDS_PER_LAYER]);

/// A raw colormap with uninterpreted palette indices, layer-major.
///
/// Holds any whole number of complete layers, exactly like the wire
/// format does; a partial (ragged) layer is not a colormap and fails
/// the parse at that layer's start.
#[derive(Clone, Debug, PartialEq, Eq, Deref)]
pub struct Colormap(pub Vec<ColormapLayer>);

impl FromStr for Colormap {
    type Err = ParseColormapError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        layers_parser.parse(s).map(Self).map_err(|err| {
            let expected = parse_expectation(err.inner().context())
                .unwrap_or_else(|| "a palette index".to_owned());

            ParseColormapError {
                offset: err.offset(),
                // Only the range rejection consumes its offender, and
                // only it carries the domain in its expectation.
                consumed_offender: expected == RANGE_EXPECTATION,
                expected,
            }
        })
    }
}

impl std::fmt::Display for Colormap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Written token by token rather than first-then-rest: a
        // layer-less colormap (`Colormap`'s field is public) renders as
        // an empty string — which fails the parse on the way back —
        // never as a phantom `0` index.
        let mut indices = self
            .0
            .iter()
            .flat_map(|layer| layer.0.iter())
            .map(|index| index.get());

        if let Some(first) = indices.next() {
            write!(f, "{first}")?;

            for index in indices {
                write!(f, " {index}")?;
            }
        }

        Ok(())
    }
}

fn layers_parser(input: &mut &str) -> ModalResult<Vec<ColormapLayer>> {
    repeat(1.., layer_parser).parse_next(input)
}

fn layer_parser(input: &mut &str) -> ModalResult<ColormapLayer> {
    let layer = repeat::<_, _, Vec<_>, _, _>(LEDS_PER_LAYER, terminated(index_parser, opt(" ")))
        .parse_next(input)?;
    let layer = layer.try_into().unwrap();

    Ok(ColormapLayer(layer))
}

/// Parses one palette index decimal. Parsing directly into the typed
/// range (not into a raw `u8` to be checked later) is what makes an
/// out-of-domain value (a `16`, which the firmware's nibble packing
/// would corrupt) fail at the parse instead.
fn index_value_parser(input: &mut &str) -> ModalResult<PaletteIndex> {
    let value: u8 = dec_uint.parse_next(input)?;

    if let Some(index) = PaletteIndex::new(value) {
        return Ok(index);
    }

    // The value was consumed to be judged, so the rejection cuts (no
    // backtracking into trying something else with these bytes) and
    // carries the domain in its expectation — the message a reader
    // needs to know WHY a well-formed number is rejected, and the
    // marker the FromStr mapping reads `consumed_offender` from.
    cut_err(fail)
        .context(StrContext::Expected(StrContextValue::Description(
            RANGE_EXPECTATION,
        )))
        .parse_next(input)
}

fn index_parser(input: &mut &str) -> ModalResult<PaletteIndex> {
    terminated(index_value_parser, opt(" ")).parse_next(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The real `colormap.map` data of the shipped Defy wireless
    /// backup (`bazecor/src/defaultBackups/Defy-wireless-backup.json`),
    /// first two full layers of 178 indices each.
    const TWO_REAL_LAYERS: &str = "10 4 4 4 4 4 7 10 4 4 4 4 4 7 2 4 4 4 4 4 0 2 4 4 4 4 4 10 3 1 5 8 2 0 0 5 4 4 4 4 4 7 11 4 4 4 4 4 7 7 7 4 4 4 4 7 2 7 7 7 4 4 10 2 1 3 8 2 6 6 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 10 12 12 12 12 12 12 10 12 9 3 9 10 12 12 1 3 3 3 10 12 2 12 12 12 12 12 12 12 1 6 5 2 12 12 5 0 0 0 12 11 12 12 0 3 3 3 12 12 12 0 3 0 3 12 12 12 6 3 3 3 3 12 12 9 3 12 12 12 12 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 7";

    /// The real full map shape: the two real layers, five times, is
    /// ten layers of 178 indices.
    fn real_colormap_data() -> String {
        std::iter::repeat_n(TWO_REAL_LAYERS, 5)
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// The full real map as its individual tokens (owned — they are
    /// split off a locally built string).
    fn real_tokens() -> Vec<String> {
        real_colormap_data().split(' ').map(String::from).collect()
    }

    #[test]
    fn parses_the_real_defy_colormap() {
        let colormap = real_colormap_data().parse::<Colormap>().unwrap();

        assert_eq!(colormap.len(), 10);

        // Spot-checks from the real capture: layer 0 opens with the
        // amber accent (index 10) framing silver keys (4), the
        // keyboard/underglow boundary at position 70 sits mid-run, and
        // layer 1 opens back on the amber accent (10).
        assert_eq!(colormap[0].0[0].get(), 10);
        assert_eq!(colormap[0].0[1].get(), 4);
        assert_eq!(colormap[0].0[KEYBOARD_LEDS - 1].get(), 6);
        assert_eq!(colormap[0].0[KEYBOARD_LEDS].get(), 4);
        assert_eq!(colormap[1].0[0].get(), 10);
    }

    #[test]
    fn parses_with_a_trailing_separator() {
        // Device responses end every value with a separator space.
        let with_trailing = format!("{} ", real_colormap_data());

        assert_eq!(
            with_trailing.parse::<Colormap>().unwrap(),
            real_colormap_data().parse::<Colormap>().unwrap()
        );
    }

    #[test]
    fn serializes_to_the_write_form_and_round_trips() {
        let colormap = real_colormap_data().parse::<Colormap>().unwrap();

        let data = colormap.to_string();

        assert!(!data.ends_with(' '));
        assert_eq!(data.split(' ').count(), 10 * LEDS_PER_LAYER);
        assert_eq!(data.parse::<Colormap>().unwrap(), colormap);
    }

    #[test]
    fn any_whole_layer_count_parses_but_a_ragged_layer_fails() {
        // One layer alone is valid wire data (a write updating just
        // the first layer), exactly like the wire format's semantics.
        let one: Vec<_> = real_tokens().into_iter().take(LEDS_PER_LAYER).collect();

        assert_eq!(one.join(" ").parse::<Colormap>().unwrap().len(), 1);

        // Seven indices short of two full layers: the parse fails at
        // the second layer's start, pointing forward from where the
        // offender lives — the same contract the keymap parser has.
        let ragged: Vec<_> = real_tokens()
            .into_iter()
            .take(2 * LEDS_PER_LAYER - 7)
            .collect();

        let err = ragged.join(" ").parse::<Colormap>().unwrap_err();

        let layer_one: String = real_tokens()
            .iter()
            .take(LEDS_PER_LAYER)
            .cloned()
            .collect::<Vec<_>>()
            .join(" ");

        assert!(!err.consumed_offender);
        assert_eq!(err.offset, layer_one.len() + 1);
        assert_eq!(err.expected, "a palette index");
    }

    #[test]
    fn an_index_above_the_nibble_domain_fails_the_parse() {
        // 16 is a well-formed u8, but the firmware stores two indices
        // per byte as (first << 4) + second: a 16 would corrupt the
        // neighbor LED's index. It is not a palette index.
        let err = "0 16".parse::<Colormap>().unwrap_err();

        // The rejection consumed the token, so the offset sits just
        // past it — the same contract the macros parser's
        // unsupported-kind rejection has.
        assert!(err.consumed_offender);
        assert_eq!(err.offset, 4);
        assert_eq!(err.expected, "a palette index (0-15)");
    }

    #[test]
    fn an_out_of_range_index_in_a_later_layer_is_located() {
        // An out-of-range index at the start of layer 1's underglow
        // (position 70): the cut failure sits just past that token.
        let mut tokens = real_tokens();
        tokens[LEDS_PER_LAYER + KEYBOARD_LEDS] = "99".to_owned();

        let data = tokens.join(" ");
        let err = data.parse::<Colormap>().unwrap_err();

        assert!(err.consumed_offender);
        assert_eq!(err.expected, RANGE_EXPECTATION);

        // The offset is where parsing STOPPED, just past the offender:
        // the end of the replaced `99` token. Everything before it is
        // one full layer, its separator, layer 2's 70 keyboard
        // indices, and their separator.
        let layer_one: String = real_tokens()
            .iter()
            .take(LEDS_PER_LAYER)
            .cloned()
            .collect::<Vec<_>>()
            .join(" ");
        let layer_two_keyboard: String = real_tokens()
            .iter()
            .skip(LEDS_PER_LAYER)
            .take(KEYBOARD_LEDS)
            .cloned()
            .collect::<Vec<_>>()
            .join(" ");

        assert_eq!(
            err.offset,
            layer_one.len() + 1 + layer_two_keyboard.len() + 1 + 2
        );
    }

    #[test]
    fn palette_index_construction_and_json_are_range_checked() {
        assert!(PaletteIndex::new(0).is_some());
        assert_eq!(PaletteIndex::MAX, 15);
        assert_eq!(PaletteIndex::MIN, 0);
        assert!(PaletteIndex::new(16).is_none());

        assert_eq!(
            "15".parse::<PaletteIndex>()
                .expect("15 is in the index domain")
                .get(),
            15
        );
        assert!("16".parse::<PaletteIndex>().is_err());

        // The JSON boundary checks like the wire boundary: a config
        // file cannot smuggle an unstoreable index in.
        assert!(serde_json::from_str::<PaletteIndex>("15").is_ok());

        let err = serde_json::from_str::<PaletteIndex>("16").unwrap_err();

        assert!(err.to_string().contains("out of range"));

        assert_eq!(
            serde_json::to_string(&PaletteIndex::new(7).unwrap()).unwrap(),
            "7"
        );
    }

    #[test]
    fn a_single_index_parses_with_the_wire_separator() {
        assert_eq!(
            "7 ".parse::<PaletteIndex>().unwrap(),
            PaletteIndex::new(7).unwrap()
        );
        assert_eq!("7".parse::<PaletteIndex>().unwrap().get(), 7);
        assert!("x".parse::<PaletteIndex>().is_err());
    }

    /// The recorded Bazecor behavior (generated by
    /// `tools/bazecor-differential/record-leds.mjs`, running Bazecor's
    /// own `parseColormapRaw` on the shipped default backups'
    /// `colormap.map` data at each product's layer size).
    #[derive(serde::Deserialize)]
    struct RecordedLeds {
        bazecor_commit: String,
        devices: std::collections::BTreeMap<String, RecordedFixture>,
    }

    /// One fixture's recorded behavior.
    #[derive(serde::Deserialize)]
    struct RecordedFixture {
        colormap_wire: String,
        colormap_layers: usize,
        colormap_layer_size: usize,
    }

    /// The recorded expectations.
    fn recorded_leds() -> RecordedLeds {
        serde_json::from_str(include_str!(
            "../../../../tools/bazecor-differential/expectations/leds.json"
        ))
        .expect("the recorded leds expectation file parses")
    }

    #[test]
    fn differential_recording_is_pinned_to_the_audited_bazecor_commit() {
        assert_eq!(
            recorded_leds().bazecor_commit,
            "ff9e10b35f9606479c63d01da06c40b7c074f8bb"
        );
    }

    #[test]
    fn our_chunking_matches_bazecors_parse_of_the_real_maps() {
        // For every recorded fixture (Defy at 178 LEDs per layer,
        // Sonsei at 56): our wire parse chunks the very same map data
        // into exactly the layer count and per-layer size Bazecor's
        // `parseColormapRaw` records, and our re-serialization is
        // byte-identical to the recorded wire.
        let recorded = recorded_leds();

        for (fixture, expected) in &recorded.devices {
            // The wire module is the DEFI's: 178 LEDs per layer. The
            // Sonsei's map (56 per layer) is a different product's
            // format this SDK does not parse — for it, the recording
            // still proves the shape facts: exactly layers × size
            // tokens, every one a storable palette index.
            if expected.colormap_layer_size != LEDS_PER_LAYER {
                let tokens = expected.colormap_wire.split_whitespace().count();

                assert_eq!(
                    tokens,
                    expected.colormap_layers * expected.colormap_layer_size,
                    "{fixture}"
                );
                assert!(
                    expected
                        .colormap_wire
                        .split_whitespace()
                        .all(|token| matches!(
                            token.parse::<u8>(),
                            Ok(value) if value <= PaletteIndex::MAX
                        )),
                    "{fixture}"
                );

                continue;
            }

            let colormap: Colormap = expected
                .colormap_wire
                .parse()
                .unwrap_or_else(|err| panic!("{fixture} colormap does not parse: {err}"));

            assert_eq!(colormap.len(), expected.colormap_layers, "{fixture}");
            assert!(
                colormap
                    .iter()
                    .all(|layer| layer.0.len() == expected.colormap_layer_size),
                "{fixture}"
            );
            assert_eq!(colormap.to_string(), expected.colormap_wire, "{fixture}");
        }
    }
}
