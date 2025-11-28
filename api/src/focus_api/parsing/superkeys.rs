//! Types for parsing superkeys.

use crate::keycode_tables::{Blank, KeyKind};
use itertools::Itertools;
use std::str::FromStr;
use winnow::{
    ModalResult, Parser,
    ascii::{dec_uint, space1},
    combinator::{repeat, repeat_till, terminated},
    token::rest,
};

/// Error when parsing a superkeys map..
#[derive(Clone, Debug, Display, Error, From)]
#[display("failed to parse superkey map data:\n{_0}")]
pub struct ParseSuperkeyMapError(#[error(not(source))] String);

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
        let map = super_keys_parser
            .parse(s)
            .map_err(|err| ParseSuperkeyMapError(err.to_string()))?;

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
    let (superkey_map, _) =
        repeat_till(1.., terminated(super_key_parser, "0 "), "0 ").parse_next(input)?;

    let _ = rest.parse_next(input)?;

    Ok(superkey_map)
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
