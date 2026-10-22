//! Types for parsing and serializing the `palette` command's wire
//! format: the device's 16-color palette.
//!
//! The Defy is in `RGBWMode` (its device descriptor carries
//! `RGBWMode: true`), and its palette is 16 colors of 4 channels each —
//! red, green, blue, and white, one decimal 0–255 per channel, 64
//! decimals on the wire in palette order. This is what every real Defy
//! capture carries (the shipped Bazecor Defy backups' `palette` entries
//! are 64 decimals, values 0–255) and what the firmware's palette
//! handler reads and writes (`kaleidoscope/plugin/LED-Palette-Theme.cpp`
//! — the upstream loop reads and sends 16 colors per palette; the Defy
//! fork's RGBW color carries the fourth channel, which is what
//! Bazecor's `RGBWMode` switching parses around).
//!
//! Channels are stored and sent verbatim: the wire holds bytes, and
//! every byte is a valid channel value, so nothing is rejected or
//! normalized here. The white channel is a real, independent channel —
//! NOT derivable from the other three. Bazecor's app *does* derive it
//! (see [`RgbwColor::from_rgb`] and the type's docs for the lossy
//! round trip that falls out of that); this module does not.

use std::str::FromStr;

use winnow::{
    ModalResult, Parser,
    ascii::dec_uint,
    combinator::{eof, opt, repeat, terminated},
};

use crate::focus_api::parsing::parse_expectation;

/// How many colors a palette holds, on every product.
///
/// The firmware's palette storage is 16 colors
/// (`kaleidoscope/plugin/LED-Palette-Theme.cpp`: `16 * sizeof(cRGB)`,
/// and its read loop sends exactly 16).
pub const COLORS: usize = 16;

/// How many decimals one Defy palette carries (16 colors × 4 RGBW
/// channels).
pub const DEFY_PALETTE_DECIMALS: usize = COLORS * RgbwColor::CHANNELS;

/// Error when parsing palette wire data.
///
/// Carries where the parser gave up within the input it was given, so
/// errors can point at the offending part of the data instead of
/// quoting it whole. A wrong *color count* is its own variant: the
/// Defy palette is exactly [`COLORS`] colors, and a stream of well-formed
/// decimals that holds a different count is not one.
#[derive(Clone, Debug, Display, Error)]
pub enum ParsePaletteError {
    /// The palette data is not valid wire data.
    #[display("failed to parse palette data at byte offset {offset}: expected {expected}")]
    MalformedData {
        /// Byte offset of the failure — exactly the offending token:
        /// the parse is one flat token stream (the color-count check
        /// runs only after it ends), so a bad token fails the parse
        /// right where it sits, wherever it sits.
        offset: usize,
        /// What the parser expected at `offset`, from the parser's own
        /// context (e.g. `a channel value`).
        expected: String,
    },
    /// The data holds well-formed decimals but not exactly
    /// [`DEFY_PALETTE_DECIMALS`] of them, so it is not a Defy palette.
    #[display(
        "palette data holds {found} values, not the {} a Defy palette carries \
         ({} colors × {} RGBW channels)",
        DEFY_PALETTE_DECIMALS,
        COLORS,
        RgbwColor::CHANNELS
    )]
    WrongColorCount {
        /// How many decimals the data holds.
        found: usize,
    },
}

/// One RGBW color: the four independent channels the Defy's palette
/// stores per color.
///
/// Any `u8` is a valid channel value — the wire stores bytes — so
/// construction is total and the struct carries no invariants beyond
/// the channel width itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RgbwColor {
    /// Red channel, 0–255.
    pub r: u8,
    /// Green channel, 0–255.
    pub g: u8,
    /// Blue channel, 0–255.
    pub b: u8,
    /// White channel, 0–255. An independent channel on the Defy's
    /// RGBW LEDs, not a sum or average of the colored ones.
    pub w: u8,
}

impl RgbwColor {
    /// How many wire decimals one RGBW color carries.
    pub const CHANNELS: usize = 4;

    /// The white channel, added to each of the other three and clamped
    /// to the byte range — Bazecor's `rgbw2b`
    /// (`bazecor/src/api/color/RGBWtoRGB.ts`), the conversion the app
    /// applies to render a Defy palette color on screen.
    ///
    /// The clamp is saturating, exactly like `sanitizeIntensity`'s
    /// `Math.min(255, …)` on values that are already known to be 0–255.
    pub fn to_rgb(self) -> RgbColor {
        let saturating_add = |channel: u8| channel.saturating_add(self.w);

        RgbColor {
            r: saturating_add(self.r),
            g: saturating_add(self.g),
            b: saturating_add(self.b),
        }
    }

    /// Re-derives a white channel from an RGB color — Bazecor's
    /// `rgb2w` (`bazecor/src/api/color/RGBtoRGBW.ts`): the smallest
    /// colored channel becomes the white channel and is subtracted
    /// from the other three.
    ///
    /// This is NOT the inverse of [`RgbwColor::to_rgb`] — it is the
    /// lossy guess Bazecor performs when *writing* a Defy palette,
    /// because its in-app palette has already dropped the wire's white
    /// channel at parse time. See the [type docs](Self) for what that
    /// does to a round trip, and why this type keeps the real channel
    /// instead.
    pub fn from_rgb(color: RgbColor) -> Self {
        let white = color.r.min(color.g).min(color.b);

        Self {
            r: color.r - white,
            g: color.g - white,
            b: color.b - white,
            w: white,
        }
    }
}

/// One RGB color: the three colored channels without the white one.
///
/// On the Defy this is only the display shape
/// [`RgbwColor::to_rgb`] renders to (and the input
/// [`RgbwColor::from_rgb`] guesses from); on the `RGBWMode: false`
/// products (the Sonsei) it is the wire color itself — see
/// [`RgbPalette`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RgbColor {
    /// Red channel, 0–255.
    pub r: u8,
    /// Green channel, 0–255.
    pub g: u8,
    /// Blue channel, 0–255.
    pub b: u8,
}

impl RgbColor {
    /// How many wire decimals one RGB color carries.
    pub const CHANNELS: usize = 3;
}

/// A full device palette: exactly [`COLORS`] RGBW colors, in palette
/// order.
///
/// The count is the type: the device stores and reports exactly
/// [`COLORS`] colors and every Defy palette write carries them all, so
/// a [`Palette`] cannot hold a different number by construction, and
/// neither its device reads nor its writes (nor the JSON files built
/// from it) can carry a partial palette.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Palette(pub [RgbwColor; COLORS]);

impl FromStr for Palette {
    type Err = ParsePaletteError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let channels = palette_parser
            .parse(s)
            .map_err(|err| ParsePaletteError::MalformedData {
                offset: err.offset(),
                expected: parse_expectation(err.inner().context())
                    .unwrap_or_else(|| "a channel value".to_owned()),
            })?;

        // Exactly the palette's worth of channels, or no palette: a
        // short stream is a corrupt read, a long one a write the
        // firmware would answer as a read. Both fail here, by count.
        let (colors, remainder) = channels.as_chunks::<{ RgbwColor::CHANNELS }>();

        let colors: Vec<RgbwColor> = colors
            .iter()
            .map(|&[r, g, b, w]| RgbwColor { r, g, b, w })
            .collect();

        // `as_chunks` leaves a tail shorter than one color in
        // `remainder`; 64 channels plus a stray 65th must fail too, so
        // both the tail and the color count are checked.
        if !remainder.is_empty() || colors.len() != COLORS {
            return Err(ParsePaletteError::WrongColorCount {
                found: channels.len(),
            });
        }

        Ok(Palette(colors.try_into().expect(
            "the count check above proves exactly COLORS colors",
        )))
    }
}

impl std::fmt::Display for Palette {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut channels = self.0.iter().flat_map(|color| color.channels());

        write!(f, "{}", channels.next().unwrap_or_default())?;

        for channel in channels {
            write!(f, " {channel}")?;
        }

        Ok(())
    }
}

impl RgbwColor {
    /// The four channels in wire order (r, g, b, w).
    fn channels(self) -> [u8; 4] {
        [self.r, self.g, self.b, self.w]
    }
}

/// Error when parsing RGB palette wire data.
///
/// The RGB palette's sibling of
/// [`ParsePaletteError`], worded for the RGB shape — the two are
/// separate types on purpose (see [`RgbPalette`]'s docs for the design
/// call), so each names its own wire form.
#[derive(Clone, Debug, Display, Error)]
pub enum ParseRgbPaletteError {
    /// The palette data is not valid wire data.
    #[display("failed to parse palette data at byte offset {offset}: expected {expected}")]
    MalformedData {
        /// Byte offset of the failure — exactly the offending token:
        /// the parse is one flat token stream (the color-count check
        /// runs only after it ends), so a bad token fails the parse
        /// right where it sits, wherever it sits.
        offset: usize,
        /// What the parser expected at `offset`, from the parser's own
        /// context (e.g. `a channel value`).
        expected: String,
    },
    /// The data holds well-formed decimals but not exactly
    /// [`RGB_PALETTE_DECIMALS`] of them, so it is not an RGB palette.
    #[display(
        "palette data holds {found} values, not the {} an RGB palette carries \
         ({} colors × {} RGB channels)",
        RGB_PALETTE_DECIMALS,
        COLORS,
        RgbColor::CHANNELS
    )]
    WrongColorCount {
        /// How many decimals the data holds.
        found: usize,
    },
}

/// How many decimals one RGB palette carries (16 colors × 3 RGB
/// channels) — the Sonsei's `palette` wire shape.
pub const RGB_PALETTE_DECIMALS: usize = COLORS * RgbColor::CHANNELS;

/// A full RGB palette: exactly [`COLORS`] RGB colors, in palette
/// order — the palette wire shape of the products whose descriptors
/// carry `RGBWMode: false`: the Sonsei's descriptor states it
/// explicitly in BOTH of Bazecor's sources, the hardware-virtual
/// keyboard and the shipped backup (48 decimals on the wire; the
/// shipped `palette` capture is exactly that).
///
/// This sits beside the RGBW [`Palette`] as its own type rather than a
/// parameterized one: the two shapes differ in their error wordings
/// (a count failure names the shape it wanted) and their JSON forms
/// (3-channel vs 4-channel color objects), and keeping the Defy's
/// types, messages, and diagnostics untouched was worth more than the
/// few shared lines a generic would have saved. [`RgbColor`] — the
/// display shape [`RgbwColor::to_rgb`] renders to — is the wire color
/// here, with all three channels stored and sent verbatim.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RgbPalette(pub [RgbColor; COLORS]);

impl FromStr for RgbPalette {
    type Err = ParseRgbPaletteError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let channels =
            palette_parser
                .parse(s)
                .map_err(|err| ParseRgbPaletteError::MalformedData {
                    offset: err.offset(),
                    expected: parse_expectation(err.inner().context())
                        .unwrap_or_else(|| "a channel value".to_owned()),
                })?;

        // Exactly the palette's worth of channels, or no palette: a
        // short stream is a corrupt read, a long one a write the
        // firmware would answer as a read. Both fail here, by count.
        let (colors, remainder) = channels.as_chunks::<{ RgbColor::CHANNELS }>();

        let colors: Vec<RgbColor> = colors
            .iter()
            .map(|&[r, g, b]| RgbColor { r, g, b })
            .collect();

        // `as_chunks` leaves a tail shorter than one color in
        // `remainder`; 48 channels plus a stray 49th must fail too, so
        // both the tail and the color count are checked.
        if !remainder.is_empty() || colors.len() != COLORS {
            return Err(ParseRgbPaletteError::WrongColorCount {
                found: channels.len(),
            });
        }

        Ok(RgbPalette(colors.try_into().expect(
            "the count check above proves exactly COLORS colors",
        )))
    }
}

impl std::fmt::Display for RgbPalette {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut channels = self.0.iter().flat_map(|color| [color.r, color.g, color.b]);

        write!(f, "{}", channels.next().unwrap_or_default())?;

        for channel in channels {
            write!(f, " {channel}")?;
        }

        Ok(())
    }
}

fn palette_parser(input: &mut &str) -> ModalResult<Vec<u8>> {
    // Every color's four decimals, then the end. Surplus data fails in
    // one of two places: a well-formed surplus stream (say 68 valid
    // decimals) parses whole, passes `eof`, and fails the count check
    // in `FromStr` (`WrongColorCount`) rather than being silently
    // truncated, while a malformed surplus token stops the `repeat`
    // with input unconsumed and fails at `eof`. Parsing directly into
    // `u8` makes an out-of-range channel (a `300`) fail instead of
    // truncate, like the settings decimals do.
    let channels = repeat(0.., terminated(dec_uint::<_, u8, _>, opt(" "))).parse_next(input)?;

    let _ = eof.parse_next(input)?;

    Ok(channels)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The real `palette` data of the shipped Defy wireless backup
    /// (`bazecor/src/defaultBackups/Defy-wireless-backup.json`), 16
    /// RGBW colors, 64 decimals.
    const REAL_DEFY_PALETTE_DATA: &str = "255 196 0 0 0 254 24 0 0 0 0 255 231 255 0 0 0 254 234 0 0 52 255 0 255 0 232 0 0 77 168 87 125 0 235 19 20 0 36 219 85 0 126 129 255 9 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0";

    #[test]
    fn parses_the_real_defy_palette() {
        let palette = REAL_DEFY_PALETTE_DATA.parse::<Palette>().unwrap();

        assert_eq!(palette.0.len(), COLORS);

        // Spot-check the real colors, wire order preserved: color 0 is
        // the warm amber the Defy boots with, color 2 pure white
        // (w=255, all colored channels dark).
        assert_eq!(
            palette.0[0],
            RgbwColor {
                r: 255,
                g: 196,
                b: 0,
                w: 0
            }
        );
        assert_eq!(
            palette.0[2],
            RgbwColor {
                r: 0,
                g: 0,
                b: 0,
                w: 255
            }
        );
        assert_eq!(
            palette.0[7],
            RgbwColor {
                r: 0,
                g: 77,
                b: 168,
                w: 87
            }
        );
    }

    #[test]
    fn parses_with_a_trailing_separator_and_normalizes_none() {
        // Device responses end every value with a separator space
        // (FocusSerial `send`); the recorded virtual-keyboard palette
        // carries the same trailing space. Both forms parse to the
        // same palette.
        let with_trailing = format!("{REAL_DEFY_PALETTE_DATA} ");

        assert_eq!(
            with_trailing.parse::<Palette>().unwrap(),
            REAL_DEFY_PALETTE_DATA.parse::<Palette>().unwrap()
        );
    }

    #[test]
    fn serializes_to_the_write_form_and_round_trips() {
        let palette = REAL_DEFY_PALETTE_DATA.parse::<Palette>().unwrap();

        // The write form is Bazecor's: decimals joined with single
        // spaces, no trailing separator (the `args.join(" ")` of
        // LayoutEditor's `updatePalette`).
        let data = palette.to_string();

        assert!(!data.ends_with(' '));
        assert_eq!(data.split(' ').count(), DEFY_PALETTE_DECIMALS);
        assert_eq!(data.parse::<Palette>().unwrap(), palette);
    }

    #[test]
    fn a_short_or_long_stream_is_not_a_palette() {
        // Three colors short of a palette: a corrupt read, not a
        // partial state.
        let short = REAL_DEFY_PALETTE_DATA
            .split(' ')
            .take(DEFY_PALETTE_DECIMALS - 4)
            .collect::<Vec<_>>()
            .join(" ");

        assert!(matches!(
            short.parse::<Palette>(),
            Err(ParsePaletteError::WrongColorCount { found: 60 })
        ));

        // A seventeenth color: surplus data fails, nothing is dropped.
        let long = format!("{REAL_DEFY_PALETTE_DATA} 1 2 3 4");

        assert!(matches!(
            long.parse::<Palette>(),
            Err(ParsePaletteError::WrongColorCount { found: 68 })
        ));
    }

    #[test]
    fn malformed_data_locates_the_failure() {
        // The eleventh token is not a number; the parser fails exactly
        // there (the decimals before it are all consumed as tokens —
        // the color-count check runs only after the token stream
        // ends).
        let err = "255 196 0 0 0 254 24 0 0 0 0 X"
            .parse::<Palette>()
            .unwrap_err();

        let ParsePaletteError::MalformedData { offset, expected } = err else {
            panic!("expected a malformed-data error, got {err:?}");
        };

        assert_eq!(offset, 29);
        assert_eq!(expected, "a channel value");

        // An out-of-range channel (259 > u8) fails the same way.
        assert!(matches!(
            "255 259 0 0".parse::<Palette>(),
            Err(ParsePaletteError::MalformedData { .. })
        ));
    }

    #[test]
    fn to_rgb_is_bazecors_rgbw2b() {
        // rgbw2b adds the white channel to each colored one, clamped
        // to the byte range (`bazecor/src/api/color/RGBWtoRGB.ts`).
        // Real palette color: no clamp needed.
        assert_eq!(
            RgbwColor {
                r: 0,
                g: 77,
                b: 168,
                w: 87
            }
            .to_rgb(),
            RgbColor {
                r: 87,
                g: 164,
                b: 255
            }
        );

        // The clamp: 250 + 100 saturates at 255.
        assert_eq!(
            RgbwColor {
                r: 250,
                g: 100,
                b: 50,
                w: 100
            }
            .to_rgb(),
            RgbColor {
                r: 255,
                g: 200,
                b: 150
            }
        );
    }

    #[test]
    fn from_rgb_is_bazecors_rgb2w() {
        // rgb2w moves the smallest colored channel into the white one
        // (`bazecor/src/api/color/RGBtoRGBW.ts`).
        assert_eq!(
            RgbwColor::from_rgb(RgbColor {
                r: 87,
                g: 164,
                b: 255
            }),
            RgbwColor {
                r: 0,
                g: 77,
                b: 168,
                w: 87
            }
        );
    }

    #[test]
    fn from_rgb_is_not_the_inverse_of_to_rgb() {
        // The Bazecor round-trip quirk, pinned: its app parses the
        // wire RGBW into display RGB (dropping w), then re-derives w
        // with rgb2w on write. A color whose smallest colored channel
        // is positive comes back DIFFERENT from the wire, because
        // rgb2w(to_rgb(c)) moves w+min(r,g,b) into w. The SDK's wire
        // type keeps the original bytes; this lossy path only exists
        // where Bazecor's display pipeline forces it.
        let wire = RgbwColor {
            r: 200,
            g: 100,
            b: 50,
            w: 100,
        };

        let re_derived = RgbwColor::from_rgb(wire.to_rgb());

        assert_ne!(re_derived, wire);
        assert_eq!(
            re_derived,
            RgbwColor {
                r: 105,
                g: 50,
                b: 0,
                w: 150,
            }
        );
    }

    #[test]
    fn json_round_trips() {
        let palette = REAL_DEFY_PALETTE_DATA.parse::<Palette>().unwrap();

        let json = serde_json::to_string(&palette).unwrap();

        // The file form is a plain array of channel objects.
        assert!(json.starts_with(r#"[{"r":255,"g":196,"b":0,"w":0},"#));

        let parsed: Palette = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed, palette);
    }

    /// The recorded Bazecor behavior (generated by
    /// `tools/bazecor-differential/record-leds.mjs`, running Bazecor's
    /// own `parsePaletteRaw` and color sources on the shipped default
    /// backups' `palette` data).
    #[derive(serde::Deserialize)]
    struct RecordedLeds {
        bazecor_commit: String,
        devices: std::collections::BTreeMap<String, RecordedFixture>,
    }

    /// One fixture's recorded behavior.
    #[derive(serde::Deserialize)]
    struct RecordedFixture {
        rgbw_mode: bool,
        palette_wire: String,
        display_colors: Vec<RgbColor>,
        /// Present only for the RGBW fixtures: on an RGB device
        /// Bazecor's writeback IS the display (its non-RGBW branch
        /// writes the display channels as-is), so there is nothing to
        /// record separately.
        app_writeback: Option<Vec<RgbwColor>>,
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
    fn our_display_rgb_matches_bazecors_parse_of_the_real_palettes() {
        // For every recorded fixture: our wire parse of the very same
        // palette data matches what Bazecor's `parsePaletteRaw` (its
        // `rgbw2b` for the RGBW products) produces — through `to_rgb`
        // for the RGBW palettes, and channel for channel for the plain
        // RGB one, which `RgbPalette` parses directly.
        let recorded = recorded_leds();

        for (fixture, expected) in &recorded.devices {
            if !expected.rgbw_mode {
                // The Sonsei's palette is plain RGB on the wire —
                // `RgbPalette`'s exact shape. The recording proves it
                // end to end: the wire data parses through `RgbPalette`
                // to exactly the colors Bazecor renders for it (the
                // RGB wire color IS the display color — no white
                // channel, no conversion).
                let palette: RgbPalette = expected
                    .palette_wire
                    .parse()
                    .unwrap_or_else(|err| panic!("{fixture} palette does not parse: {err}"));

                assert_eq!(palette.0.to_vec(), expected.display_colors, "{fixture}");

                continue;
            }

            let palette: Palette = expected
                .palette_wire
                .parse()
                .unwrap_or_else(|err| panic!("{fixture} palette does not parse: {err}"));

            let ours: Vec<RgbColor> = palette.0.iter().map(|color| color.to_rgb()).collect();

            assert_eq!(ours, expected.display_colors, "{fixture}");
        }
    }

    #[test]
    fn our_from_rgb_matches_bazecors_writeback_math() {
        // LayoutEditor's `updatePalette` writes `rgb2w(display)` per
        // color on an RGBW device. Our `from_rgb` is that same math —
        // pinned here against Bazecor's own output, so the documented
        // lossy round trip is characterized from its source, not from
        // our reading of it. (Our wire type round-trips VERBATIM; this
        // re-derivation exists only where a display color is all you
        // have. On an RGB device the writeback is the display itself,
        // so nothing is recorded there.)
        let recorded = recorded_leds();

        for (fixture, expected) in &recorded.devices {
            let Some(writeback) = &expected.app_writeback else {
                assert!(!expected.rgbw_mode, "{fixture}");
                continue;
            };

            let ours: Vec<RgbwColor> = expected
                .display_colors
                .iter()
                .copied()
                .map(RgbwColor::from_rgb)
                .collect();

            assert_eq!(ours, *writeback, "{fixture}");
        }
    }
}
