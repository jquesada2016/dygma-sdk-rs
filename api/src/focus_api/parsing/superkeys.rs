//! Types for parsing superkeys.

use crate::keycode_tables::{Blank, KeyKind};
use itertools::Itertools;
use std::str::FromStr;
use winnow::{
    ModalResult, Parser,
    ascii::{dec_uint, space1},
    combinator::{alt, eof, repeat_till, terminated},
    error::StrContext,
    token::rest,
};

/// Error when parsing a superkey map from wire data.
///
/// Carries the byte offset of the failure within the input the parser
/// was given, so errors can point at the offending part of the data
/// instead of quoting it whole.
#[derive(Clone, Debug, Display, Error)]
#[display("failed to parse superkey map data at byte offset {offset}: expected {expected}")]
pub struct ParseSuperkeyMapError {
    /// Byte offset of the failure: the start of the token that could not
    /// be parsed, or the end of the input when the data ended where a
    /// token was expected.
    pub offset: usize,
    /// What the parser expected at `offset`, from the parser's own
    /// context (e.g. `superkey terminator`).
    pub expected: String,
}

/// Error returned when there are too many superkeys in a [`SuperkeyMap`]
/// and thus, creating the command data would overflow.
#[derive(Clone, Copy, Debug, Display, Error)]
pub struct TooManySuperkeysError;
/// Struct containing a list of defined superkeys.
#[derive(Clone, Debug)]
pub struct SuperkeyMap(pub Vec<Superkey>);

impl FromStr for SuperkeyMap {
    type Err = ParseSuperkeyMapError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // An empty or whitespace-only map holds no superkeys, and
        // neither does an erased one: a stream of `65535` bytes — the
        // eraser payload an empty map serializes to. (Parsing these as
        // empty maps is user-ratified 2026-10-10.)
        if s.trim().is_empty() || s.split_whitespace().all(|token| token == "65535") {
            return Ok(Self(Vec::new()));
        }

        let map = super_keys_parser
            .parse(s)
            .map_err(|err| ParseSuperkeyMapError {
                offset: err.offset(),
                expected: crate::focus_api::parsing::parse_expectation(err.inner().context())
                    .unwrap_or_else(|| "a superkey action value".to_owned()),
            })?;

        Ok(Self(map))
    }
}

impl SuperkeyMap {
    /// Converts the superkey map into a form suitable for sending to
    /// the keyboard as command data.
    pub fn to_command_data<const MEMORY_SIZE: usize>(
        &self,
    ) -> Result<String, TooManySuperkeysError> {
        if self.0.is_empty() {
            return Ok([u16::MAX; MEMORY_SIZE].iter().join(" "));
        }

        let superkeys = self
            .0
            .iter()
            .flat_map(|key| key.to_command_data())
            // We need to add a final 0 byte to indicate the end of the
            // superkey map
            .chain([0]);

        if superkeys.clone().count() > MEMORY_SIZE {
            return Err(TooManySuperkeysError);
        }

        let mut res = [u16::MAX; MEMORY_SIZE];

        res.iter_mut()
            .zip(superkeys)
            .for_each(|(res, key)| *res = key);

        Ok(res.iter().join(" "))
    }
}

/// Superkey containing uninterpreted actions.
#[derive(Clone, Debug)]
pub struct Superkey {
    /// Action performed when tapping the key.
    pub tap: Option<KeyKind>,
    /// Action performed when holding the key.
    pub hold: Option<KeyKind>,
    /// Action performed when tapping and holding the key.
    pub tap_hold: Option<KeyKind>,
    /// Action performed when double tapping the key.
    pub double_tap: Option<KeyKind>,
    /// Action performed when double tapping and holding the key.
    pub double_tap_hold: Option<KeyKind>,
}

impl Superkey {
    /// Converts the key into a form suitable for sending to the
    /// keyboard.
    pub fn to_command_data(&self) -> [u16; 6] {
        let Self {
            tap,
            hold,
            tap_hold,
            double_tap,
            double_tap_hold,
        } = *self;

        let action_to_u16 = |key| match key {
            None => 1,
            Some(KeyKind::Blank(Blank::NoKey)) => 1,
            Some(key) => u16::from(key),
        };

        let [tap, hold, tap_hold, double_tap, double_tap_hold] =
            [tap, hold, tap_hold, double_tap, double_tap_hold].map(action_to_u16);

        [tap, hold, tap_hold, double_tap, double_tap_hold, 0]
    }
}

fn super_keys_parser(input: &mut &str) -> ModalResult<Vec<Superkey>> {
    let (superkey_map, _) = repeat_till(
        1..,
        terminated(super_key_parser, superkey_end_parser),
        superkey_end_parser,
    )
    .parse_next(input)?;

    // Anything after the map's structural `0 0` terminator is stale
    // data — the `65535` padding, or bytes left behind by a previous,
    // larger map — and is intentionally ignored, mirroring the macro
    // parser and Bazecor, which splits the raw string on the first
    // `" 0 0"` and discards the tail. Unlike that substring split, the
    // terminator here is structural, so slot values containing `0 0`
    // are not cut short. (Trailing-data tolerance adjudicated
    // intentional 2026-10-10.)
    let _ = rest.parse_next(input)?;

    Ok(superkey_map)
}

/// The `0` that terminates a superkey and the superkey map itself,
/// followed by an optional separating space (or the end of the data).
///
/// Mirrors the macro map's terminator: the device capture ends with a
/// trailing space, but Bazecor backup entries end `... 0 0` with no
/// trailing space, so both must parse.
fn superkey_end_parser(input: &mut &str) -> ModalResult<()> {
    ("0", alt((space1.void(), eof.void())))
        .context(StrContext::Label("superkey terminator"))
        .parse_next(input)
        .map(|_| ())
}

fn super_key_parser(input: &mut &str) -> ModalResult<Superkey> {
    let (tap, hold, tap_hold, double_tap, double_tap_hold) = (
        superkey_action_parser,
        superkey_action_parser,
        superkey_action_parser,
        superkey_action_parser,
        superkey_action_parser,
    )
        .parse_next(input)?;

    Ok(Superkey {
        tap,
        hold,
        tap_hold,
        double_tap,
        double_tap_hold,
    })
}

fn superkey_action_parser(input: &mut &str) -> ModalResult<Option<KeyKind>> {
    let (action, _) = (
        dec_uint::<_, u16, _>,
        // The final byte of the data may be at the end of the input, with
        // no separating space after it.
        alt((space1.void(), eof.void())),
    )
        .parse_next(input)?;

    if action == 1 {
        return Ok(None);
    }

    let key = KeyKind::from(action);

    if key == Blank::NoKey {
        return Ok(None);
    }

    Ok(Some(key))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SUPERKEY_DATA: &str = "44 57 1 1 1 0 44 57 1 1 1 0 89 58 1 1 1 0 90 59 1 1 1 0 91 60 1 1 1 0 92 61 1 1 1 0 93 62 1 1 1 0 94 63 1 1 1 0 95 64 1 1 1 0 96 65 1 1 1 0 97 66 1 1 1 0 98 67 1 1 1 0 0 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 ";

    #[test]
    fn parse_succeeds() {
        let _ = SUPERKEY_DATA.parse::<SuperkeyMap>().unwrap();
    }

    #[test]
    fn can_parse_without_trailing_space() {
        // Mirrors the macro parser: device captures end with a trailing
        // space (see `SUPERKEY_DATA` above), but Bazecor backup entries
        // carry none, so we accept both.
        let map = "44 57 1 1 1 0 0".parse::<SuperkeyMap>().unwrap();

        assert_eq!(map.0.len(), 1);
        assert_eq!(map.0[0].tap, Some(KeyKind::from(44)));
        assert_eq!(map.0[0].hold, Some(KeyKind::from(57)));
        // An action value of `1` means "no action".
        assert_eq!(map.0[0].tap_hold, None);
        assert_eq!(map.0[0].double_tap, None);
        assert_eq!(map.0[0].double_tap_hold, None);
    }

    #[test]
    fn can_parse_bazecor_backup_superkeys() {
        // The real `superkeys.map` entry from Bazecor's
        // `src/defaultBackups/Defy-wired-backup.json`, byte-identical in
        // the wireless backup: one superkey, then the superkey and map
        // terminators. Byte-verified: it ends `... 0 0` with NO trailing
        // space, and Bazecor stores no `65535` padding — only the live
        // bytes.
        const BAZECOR_BACKUP_SUPERKEY_DATA: &str = "262 281 2329 283 1 0 0";

        let map = BAZECOR_BACKUP_SUPERKEY_DATA.parse::<SuperkeyMap>().unwrap();

        assert_eq!(map.0.len(), 1);
        assert_eq!(map.0[0].tap, Some(KeyKind::from(262)));
        assert_eq!(map.0[0].hold, Some(KeyKind::from(281)));
        assert_eq!(map.0[0].tap_hold, Some(KeyKind::from(2329)));
        assert_eq!(map.0[0].double_tap, Some(KeyKind::from(283)));
        assert_eq!(map.0[0].double_tap_hold, None);

        // Round-trip: serializing pads the live bytes to the 512-value
        // superkey memory, and the padded data parses back to the same
        // superkey.
        let str_data = map.to_command_data::<512>().unwrap();

        assert!(str_data.starts_with(BAZECOR_BACKUP_SUPERKEY_DATA));

        let reparsed = str_data.parse::<SuperkeyMap>().unwrap();

        assert_eq!(reparsed.0.len(), 1);
        assert_eq!(reparsed.0[0].tap, Some(KeyKind::from(262)));
        assert_eq!(reparsed.0[0].double_tap_hold, None);
    }

    #[test]
    fn can_parse_sonsei_backup_superkeys() {
        // The real `superkeys.map` entry from Bazecor's
        // `src/defaultBackups/Sonsei.json`: four superkeys. Byte-verified
        // like the Defy backups: no trailing space, no padding.
        const SONSEI_BACKUP_SUPERKEY_DATA: &str =
            "42 17452 1 1 1 0 40 17451 1 1 1 0 17154 230 1 1 1 0 57 17452 1 1 1 0 0";

        let map = SONSEI_BACKUP_SUPERKEY_DATA.parse::<SuperkeyMap>().unwrap();

        assert_eq!(map.0.len(), 4);
        assert_eq!(map.0[0].tap, Some(KeyKind::from(42)));
        assert_eq!(map.0[3].tap, Some(KeyKind::from(57)));
        assert_eq!(map.0[3].hold, Some(KeyKind::from(17452)));
        assert_eq!(map.0[3].double_tap_hold, None);

        let str_data = map.to_command_data::<512>().unwrap();

        assert!(str_data.starts_with(SONSEI_BACKUP_SUPERKEY_DATA));
    }

    #[test]
    fn can_parse_erased_superkeys() {
        // An erased superkey map is the eraser payload: 512 `65535`
        // bytes, exactly what an empty map serializes to.
        let eraser_data = vec!["65535"; 512].join(" ");

        let map = eraser_data.parse::<SuperkeyMap>().unwrap();

        assert!(map.0.is_empty());

        // The round trip is a fixed point: serializing the empty map
        // reproduces the eraser payload byte for byte.
        assert_eq!(
            SuperkeyMap(Vec::new()).to_command_data::<512>().unwrap(),
            eraser_data
        );

        // The erased state is recognized at any length, symmetric with
        // the macro parser's all-`255` ruling.
        assert!(
            "65535 65535 65535 65535 65535 "
                .parse::<SuperkeyMap>()
                .unwrap()
                .0
                .is_empty()
        );
    }

    #[test]
    fn can_parse_empty_input() {
        // An empty or whitespace-only map holds no superkeys, symmetric
        // with the erased all-`65535` stream above. (Parsing these as
        // empty maps is user-ratified 2026-10-10.)
        for data in ["", "   "] {
            let map = data.parse::<SuperkeyMap>().unwrap();

            assert!(map.0.is_empty());
        }
    }

    #[test]
    fn malformed_data_fails_cleanly() {
        // Garbage instead of superkey data: the failure carries its
        // offset and the parser's expectation, not the input.
        let err = "garbage".parse::<SuperkeyMap>().unwrap_err();

        assert_eq!(err.offset, 0);
        assert_eq!(
            err.to_string(),
            "failed to parse superkey map data at byte offset 0: expected a superkey action value"
        );

        // A truncated map: a superkey terminator without the map's
        // final `0`. The parser fails at the end of the input looking
        // for the next superkey's first action.
        let err = "44 57 1 1 1 0".parse::<SuperkeyMap>().unwrap_err();

        assert_eq!(err.offset, 13);
        assert_eq!(err.expected, "a superkey action value");

        // A superkey with fewer than five actions.
        assert!("44 57 1 1 0".parse::<SuperkeyMap>().is_err());

        // A mid-map token that is not a number fails exactly at its
        // start.
        let err = "44 57 O 1 1 0".parse::<SuperkeyMap>().unwrap_err();

        assert_eq!(err.offset, 6);

        // Later superkeys fail exactly at the offending token too —
        // both a non-numeric action and a terminator mismatch (the
        // offset sits on the token where the `0` was expected).
        let err = "44 57 1 1 1 0 44 57 O 1 1 0"
            .parse::<SuperkeyMap>()
            .unwrap_err();

        assert_eq!(err.offset, 20);
        assert_eq!(err.expected, "a superkey action value");

        let err = "44 57 1 1 1 0 44 57 1 1 1 1 0"
            .parse::<SuperkeyMap>()
            .unwrap_err();

        assert_eq!(err.offset, 26);
        assert_eq!(err.expected, "superkey terminator");
    }
}
