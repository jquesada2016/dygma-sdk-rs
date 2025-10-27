//! Types for parsing superkeys.

use crate::parsing::keymap::{Blank, KeyKind};
use std::str::FromStr;
use winnow::{
    ModalResult, Parser,
    ascii::{dec_uint, space1},
    combinator::{repeat, repeat_till, terminated},
};

/// Error when parsing a superkeys map..
#[derive(Clone, Debug, Display, Error, From)]
#[display("failed to parse superkey map data:\n{_0}")]
pub struct ParseSuperkeyMapError(#[error(not(source))] String);

/// Struct containing a list of defined superkeys.
#[derive(Clone, Debug)]
pub struct SuperkeyMap(pub Vec<SuperKey>);

impl FromStr for SuperkeyMap {
    type Err = ParseSuperkeyMapError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let map = super_keys_parser
            .parse(s)
            .map_err(|err| ParseSuperkeyMapError(err.to_string()))?;

        Ok(Self(map))
    }
}

/// Superkey containing uninterpreted actions.
#[derive(Clone, Debug)]
pub struct SuperKey {
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

fn super_keys_parser(input: &mut &str) -> ModalResult<Vec<SuperKey>> {
    let (superkey_map, _) =
        repeat_till(1.., terminated(super_key_parser, "0 "), "0 ").parse_next(input)?;

    let _: Vec<_> = repeat(.., "65535 ").parse_next(input)?;

    Ok(superkey_map)
}

fn super_key_parser(input: &mut &str) -> ModalResult<SuperKey> {
    let (tap, hold, tap_hold, double_tap, double_tap_hold) = (
        superkey_action_parser,
        superkey_action_parser,
        superkey_action_parser,
        superkey_action_parser,
        superkey_action_parser,
    )
        .parse_next(input)?;

    Ok(SuperKey {
        tap,
        hold,
        tap_hold,
        double_tap,
        double_tap_hold,
    })
}

fn superkey_action_parser(input: &mut &str) -> ModalResult<Option<KeyKind>> {
    let (action, _) = (dec_uint::<_, u16, _>, space1).parse_next(input)?;

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
}
