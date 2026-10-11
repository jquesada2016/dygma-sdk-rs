//! Types for parsing macros.

use std::str::FromStr;

use itertools::Itertools;
use winnow::{
    ModalResult, Parser,
    ascii::dec_uint,
    combinator::{alt, cut_err, eof, fail, repeat_till},
    error::{StrContext, StrContextValue},
    token::rest,
};

use crate::keycode_tables::KeyKind;

/// Error when parsing a macro map.
#[derive(Clone, Debug, Display, Error, From)]
#[display("failed to parse macro map data:\n{_0}")]
pub struct ParseMacroMapError(#[error(not(source))] String);

/// Error returned when there are too many macros in a [`MacroMap`] and
/// thus, creating the command data would overflow the macro memory.
#[derive(Clone, Copy, Debug, Display, Error)]
#[display("the macro map does not fit in the macro memory")]
pub struct TooManyMacrosError;

/// Error returned when a key code is too large to fit in the single byte
/// of data that single-byte key code actions carry.
#[derive(Clone, Copy, Debug, Display, Error)]
#[display("key code {_0} does not fit in a single byte")]
pub struct KeyCodeTooLargeError(#[error(not(source))] u16);

/// Error returned when converting a [`MacroMap`] into command data.
#[derive(Clone, Copy, Debug, Display, Error)]
pub enum ToCommandDataError {
    /// The macro map does not fit in the macro memory.
    #[display("{_0}")]
    TooManyMacros(TooManyMacrosError),
    /// A key code was too large for a single-byte macro action.
    #[display("{_0}")]
    KeyCodeTooLarge(KeyCodeTooLargeError),
}

impl From<TooManyMacrosError> for ToCommandDataError {
    fn from(err: TooManyMacrosError) -> Self {
        Self::TooManyMacros(err)
    }
}

impl From<KeyCodeTooLargeError> for ToCommandDataError {
    fn from(err: KeyCodeTooLargeError) -> Self {
        Self::KeyCodeTooLarge(err)
    }
}

/// Struct containing a list of defined macros.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MacroMap(pub Vec<Macro>);

impl FromStr for MacroMap {
    type Err = ParseMacroMapError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // An empty or whitespace-only map holds no macros, and neither
        // does an erased one: a stream of `255` bytes. (Parsing these as
        // empty maps is user-ratified 2026-10-10.)
        if s.trim().is_empty() || s.split_whitespace().all(|token| token == "255") {
            return Ok(Self(Vec::new()));
        }

        let macros = macros_parser
            .parse(s)
            .map_err(|err| ParseMacroMapError(err.to_string()))?;

        Ok(Self(macros))
    }
}

impl MacroMap {
    /// Converts the macro map into a form suitable for sending to the
    /// keyboard as command data.
    ///
    /// `memory_size` is the device's macro map memory, as reported by
    /// the `macros.memory` command (see
    /// [`MacroMemorySize`](crate::focus_api::parsing::settings::MacroMemorySize)).
    ///
    /// An empty map is converted into an eraser payload of
    /// `memory_size` `255` bytes, and thus, erases all macros on the
    /// keyboard.
    pub fn to_command_data(&self, memory_size: usize) -> Result<String, ToCommandDataError> {
        if self.0.is_empty() {
            return Ok(std::iter::repeat_n("255", memory_size).join(" "));
        }

        let macros = self
            .0
            .iter()
            .map(Macro::to_command_data)
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            // We need to add a final 0 byte to indicate the end of the
            // macro map
            .chain([0])
            .collect::<Vec<_>>();

        if macros.len() > memory_size {
            return Err(TooManyMacrosError.into());
        }

        let mut res = vec![255; memory_size];

        res.iter_mut()
            .zip(macros)
            .for_each(|(res, byte)| *res = byte);

        Ok(res.iter().join(" "))
    }
}

/// Represents a single macro.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Macro {
    /// The actions this macro will perform.
    pub actions: Vec<MacroAction>,
}

impl Macro {
    /// Converts the macro into a form suitable for sending to the
    /// keyboard as command data.
    ///
    /// The data is terminated with a `0` byte, marking the end of this
    /// macro and separating it from the next one.
    pub fn to_command_data(&self) -> Result<Vec<u8>, KeyCodeTooLargeError> {
        let actions = self
            .actions
            .iter()
            .map(MacroAction::to_command_data)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(actions.into_iter().flatten().chain([0]).collect())
    }
}

/// The possible actions a macro can perform.
///
/// The firmware has two flavors of key actions: ones carrying a full u16
/// key code, and ones carrying the key code in a single byte, which can
/// only refer to keys without modifiers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MacroAction {
    /// Applies a random delay between the min and max range, in ms.
    Interval {
        /// The minimum delay amount in ms.
        min: u16,
        /// The maximum delay amount in ms.
        max: u16,
    },
    /// Waits for a fixed amount of ms.
    Wait(u16),
    /// Holds down the key. Carries a full u16 key code.
    KeyDown(KeyKind),
    /// Releases the held key. Carries a full u16 key code.
    KeyUp(KeyKind),
    /// Taps the key. Carries a full u16 key code.
    Tap(KeyKind),
    /// Holds down the key. Carries the key code in a single byte, so only
    /// keys without modifiers can be used.
    KeyCodeDown(KeyKind),
    /// Releases the held key. Carries the key code in a single byte, so
    /// only keys without modifiers can be used.
    KeyCodeUp(KeyKind),
    /// Taps the key. Carries the key code in a single byte, so only keys
    /// without modifiers can be used.
    TapCode(KeyKind),
}

impl MacroAction {
    /// Converts the action into the bytes it occupies in the macro map's
    /// command data.
    pub fn to_command_data(&self) -> Result<Vec<u8>, KeyCodeTooLargeError> {
        match *self {
            Self::Interval { min, max } => Ok(vec![
                1,
                (min >> 8) as u8,
                (min & 0xFF) as u8,
                (max >> 8) as u8,
                (max & 0xFF) as u8,
            ]),
            Self::Wait(delay) => Ok(vec![2, (delay >> 8) as u8, (delay & 0xFF) as u8]),
            Self::KeyDown(key) => Ok(u16_key_command_data(3, key)),
            Self::KeyUp(key) => Ok(u16_key_command_data(4, key)),
            Self::Tap(key) => Ok(u16_key_command_data(5, key)),
            Self::KeyCodeDown(key) => u8_key_command_data(6, key),
            Self::KeyCodeUp(key) => u8_key_command_data(7, key),
            Self::TapCode(key) => u8_key_command_data(8, key),
        }
    }
}

/// The bytes a u16 key code action occupies in the macro map's command
/// data: the action kind, followed by the key code's high and low bytes.
fn u16_key_command_data(kind: u8, key: KeyKind) -> Vec<u8> {
    let code = u16::from(key);

    vec![kind, (code >> 8) as u8, (code & 0xFF) as u8]
}

/// The bytes a single-byte key code action occupies in the macro map's
/// command data: the action kind, followed by the key code.
///
/// Fails if the key code does not fit in a single byte.
fn u8_key_command_data(kind: u8, key: KeyKind) -> Result<Vec<u8>, KeyCodeTooLargeError> {
    let code = u16::from(key);
    let code = u8::try_from(code).map_err(|_| KeyCodeTooLargeError(code))?;

    Ok(vec![kind, code])
}

fn macros_parser(input: &mut &str) -> ModalResult<Vec<Macro>> {
    let (macros, _) = repeat_till(0.., macro_parser, macro_end_parser).parse_next(input)?;

    let _ = rest.parse_next(input)?;

    Ok(macros)
}

/// The `0` that terminates a macro and the macro map, followed by an
/// optional separating space (or the end of the data).
fn macro_end_parser(input: &mut &str) -> ModalResult<()> {
    ("0", alt((" ".void(), eof.void())))
        .context(StrContext::Label("macro terminator"))
        .parse_next(input)
        .map(|_| ())
}

fn macro_parser(input: &mut &str) -> ModalResult<Macro> {
    let (actions, _) = repeat_till(1.., action_parser, macro_end_parser)
        .context(StrContext::Label("macro"))
        .parse_next(input)?;

    Ok(Macro { actions })
}

fn action_parser(input: &mut &str) -> ModalResult<MacroAction> {
    let context_label = StrContext::Label("macro action");

    let kind = u8_parser
        .context(context_label.clone())
        .context(StrContext::Expected(StrContextValue::Description(
            "u8 representing action type",
        )))
        .parse_next(input)?;

    // Kinds 9 to 13 are documented firmware actions (report control and
    // tap sequences) that we don't support yet. They must fail
    // deterministically before the stray-byte skip below, which would
    // otherwise consume their data bytes as if they were kinds.
    if (9..=13).contains(&kind) {
        cut_err(fail)
            .context(context_label.clone())
            .context(StrContext::Expected(StrContextValue::Description(
                "a supported action kind; kinds 9 to 13 are not yet supported",
            )))
            .parse_next(input)?;
    }

    // If the kind byte is not between 1 and 8 inclusive, we need to skip it.
    // No clue why there would be a seemingly random byte here
    let kind = if !(1..=8).contains(&kind) {
        u8_parser
            .context(context_label.clone())
            .context(StrContext::Expected(StrContextValue::Description(
                "u8 representing action type",
            )))
            .parse_next(input)?
    } else {
        kind
    };

    let action = match kind {
        1 => {
            let (min, max) = (u16_parser, u16_parser)
                .context(context_label.clone())
                .parse_next(input)?;

            MacroAction::Interval { min, max }
        }
        2 => {
            let delay = u16_parser
                .context(context_label.clone())
                .parse_next(input)?;

            MacroAction::Wait(delay)
        }
        3 => {
            let key = u16_parser
                .context(context_label.clone())
                .parse_next(input)?;

            MacroAction::KeyDown(KeyKind::from(key))
        }
        4 => {
            let key = u16_parser
                .context(context_label.clone())
                .parse_next(input)?;

            MacroAction::KeyUp(KeyKind::from(key))
        }
        5 => {
            let key = u16_parser
                .context(context_label.clone())
                .parse_next(input)?;

            MacroAction::Tap(KeyKind::from(key))
        }
        6 => {
            let key = u8_parser.context(context_label.clone()).parse_next(input)?;

            MacroAction::KeyCodeDown(KeyKind::from(u16::from(key)))
        }
        7 => {
            let key = u8_parser.context(context_label.clone()).parse_next(input)?;

            MacroAction::KeyCodeUp(KeyKind::from(u16::from(key)))
        }
        8 => {
            let key = u8_parser.context(context_label.clone()).parse_next(input)?;

            MacroAction::TapCode(KeyKind::from(u16::from(key)))
        }
        _ => {
            let _ = fail
                .context(context_label)
                .context(StrContext::Expected(StrContextValue::Description(
                    "action type between 1 and 8 inclusive",
                )))
                .parse_next(input)?;

            unreachable!()
        }
    };

    Ok(action)
}

fn u8_parser(input: &mut &str) -> ModalResult<u8> {
    let (res, _) = (dec_uint, alt((" ".void(), eof.void())))
        .context(StrContext::Label("u8"))
        .parse_next(input)?;

    Ok(res)
}

fn u16_parser(input: &mut &str) -> ModalResult<u16> {
    let (high_byte, low_byte) = (u8_parser, u8_parser)
        .context(StrContext::Label("u16"))
        .context(StrContext::Expected(
            winnow::error::StrContextValue::Description("two sequential u8s"),
        ))
        .parse_next(input)?;

    // The wire format is big endian: the first byte carries the high bits.
    Ok(u16::from(high_byte) << 8 | u16::from(low_byte))
}

#[cfg(test)]
mod tests {
    use crate::keycode_tables::Spacing;
    use itertools::Itertools;

    use super::*;

    const MACRO_DATA: &str = "8 44 6 225 8 11 7 225 8 8 8 28 8 54 8 44 2 3 232 6 225 8 7 7 225 8 28 8 10 8 16 8 4 8 23 8 8 5 68 43 5 68 44 5 68 86 5 210 93 5 67 2 5 80 65 0 6 225 8 23 7 225 8 11 8 12 8 22 8 44 8 12 8 22 8 44 8 4 8 44 8 23 8 8 8 22 8 23 8 55 0 0 23 8 8 8 7 8 44 8 15 8 12 8 22 8 23 8 40 0 6 227 8 80 7 227 8 84 8 5 8 4 8 17 8 17 8 8 8 21 8 44 8 28 8 8 8 15 8 15 8 18 8 26 8 40 0 6 227 8 80 7 227 8 84 8 11 8 32 8 40 6 227 8 79 7 227 0 6 227 8 80 7 227 8 84 8 11 8 33 8 40 6 227 8 79 7 227 0 6 227 8 80 7 227 8 84 8 6 8 11 8 8 8 6 8 14 8 15 8 12 8 22 8 23 8 40 0 6 227 8 80 7 227 8 84 8 5 8 4 8 17 8 17 8 8 8 21 8 44 8 10 8 21 8 8 8 8 8 17 8 40 6 227 8 79 7 227 0 6 227 8 80 7 227 8 84 8 5 8 4 8 17 8 17 8 8 8 21 8 44 8 21 8 8 8 7 8 40 6 227 8 79 7 227 0 6 227 8 80 7 227 8 84 8 5 8 4 8 17 8 17 8 8 8 21 8 44 8 5 8 15 8 24 8 8 8 40 6 227 8 79 7 227 0 8 84 8 10 8 12 8 19 8 11 8 28 2 0 200 8 40 0 0 8 10 8 21 8 8 8 8 8 17 8 40 0 8 84 8 5 8 4 8 17 8 17 8 8 8 21 8 44 8 21 8 8 8 7 8 40 0 0 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255";

    #[test]
    fn can_parse_2_u8s_to_u16() {
        let n = 1_000u16;
        let bytes = [3, 232].iter().join(" ");

        let input = format!("{bytes} ");

        let res = u16_parser.parse(&input).unwrap();

        assert_eq!(res, n);
    }

    #[test]
    fn can_parse_macros() {
        let macros = MACRO_DATA.parse::<MacroMap>().unwrap();

        // Parsing stops at the map's structural `0 0` terminator (the
        // terminator is tried before each action); anything after it is
        // stale data left behind by a previous, larger macro map, and is
        // ignored. Unlike Bazecor's raw substring split on `" 0 0"`, this
        // is structural: an action whose bytes legitimately contain
        // `0 0`, like `Interval { min: 0, max: 0 }`, parses fine.
        assert_eq!(macros.0.len(), 2);

        // The first action of the first macro is a tap of the space key,
        // sent as a single-byte key code.
        assert!(matches!(
            macros.0[0].actions[0],
            MacroAction::TapCode(KeyKind::Spacing(Spacing::Space))
        ));

        // A fixed delay of 1000 ms, sent as a u16 high byte first.
        assert!(macros.0[0].actions.contains(&MacroAction::Wait(1000)));
    }

    #[test]
    fn can_parse_interval() {
        let map = "1 0 100 1 44 0 0 ".parse::<MacroMap>().unwrap();

        assert_eq!(
            map.0,
            vec![Macro {
                actions: vec![MacroAction::Interval { min: 100, max: 300 }],
            }]
        );
    }

    #[test]
    fn can_parse_zero_interval() {
        // An action whose bytes contain `0 0` parses fine: the map
        // terminator is structural, not a substring search like Bazecor's.
        let map = "1 0 0 0 0 0 0 ".parse::<MacroMap>().unwrap();

        assert_eq!(
            map.0,
            vec![Macro {
                actions: vec![MacroAction::Interval { min: 0, max: 0 }],
            }]
        );
    }

    #[test]
    fn can_parse_erased_macros() {
        // An erased macro map is a stream of `255` bytes.
        let map = "255 255 255 255 255 ".parse::<MacroMap>().unwrap();

        assert!(map.0.is_empty());
    }

    #[test]
    fn can_parse_structurally_empty_macros() {
        // A map holding no macros at all: just the macro and map
        // terminators, with or without a trailing space.
        for data in ["0 0", "0 0 "] {
            let macros = data.parse::<MacroMap>().unwrap();

            assert!(macros.0.is_empty());
        }
    }

    #[test]
    fn can_parse_empty_input() {
        // An empty or whitespace-only map holds no macros, symmetric
        // with the erased all-`255` stream above. (Parsing these as
        // empty maps is user-ratified 2026-10-10.)
        for data in ["", "   "] {
            let macros = data.parse::<MacroMap>().unwrap();

            assert!(macros.0.is_empty());
        }
    }

    #[test]
    fn can_parse_without_trailing_space() {
        // Device-verified: the firmware parses `macros.map` data fine
        // without a trailing space — the space is a Bazecor backup
        // serialization quirk, so we accept both.
        let macros = "6 4 0 0".parse::<MacroMap>().unwrap();

        assert_eq!(
            macros.0,
            vec![Macro {
                actions: vec![MacroAction::KeyCodeDown(KeyKind::from(4))],
            }]
        );
    }

    #[test]
    fn can_parse_bazecor_backup_macros() {
        // The real `macros.map` entry from Bazecor's
        // `src/defaultBackups/Defy-wired-backup.json` (identical in the
        // wireless backup; Sonsei's differs — LeftShift-based, two
        // macros): one macro of 16 actions (a RightShift-held key
        // sequence), then the macro and map terminators, with no
        // trailing space.
        const BAZECOR_BACKUP_MACRO_DATA: &str =
            "6 229 8 11 7 229 8 8 8 28 8 54 8 44 6 229 8 7 7 229 8 28 8 10 8 16 8 4 8 23 8 8 0 0";

        let macros = BAZECOR_BACKUP_MACRO_DATA.parse::<MacroMap>().unwrap();

        assert_eq!(macros.0.len(), 1);
        assert_eq!(macros.0[0].actions.len(), 16);
        assert_eq!(
            macros.0[0].actions[0],
            MacroAction::KeyCodeDown(KeyKind::from(229))
        );

        let str_data = macros.to_command_data(2048).unwrap();

        // Serializing reproduces the live bytes, followed by padding.
        assert!(str_data.starts_with(BAZECOR_BACKUP_MACRO_DATA));
    }

    #[test]
    fn unsupported_action_kinds_fail_cleanly() {
        // Kind 12 is TAP_SEQUENCE in the firmware, which the SDK does not
        // support yet. It must fail deterministically instead of being
        // skipped as a stray byte and misparsing the sequence's bytes as
        // action kinds.
        let err = "12 14 7 18 18 26 0 0 ".parse::<MacroMap>().unwrap_err();

        assert!(err.to_string().contains("not yet supported"));

        // A stray byte followed by an unsupported kind still fails,
        // through the generic out-of-range path.
        assert!("14 12 8 44 0 0 ".parse::<MacroMap>().is_err());

        // Unparsable data still fails.
        assert!("garbage".parse::<MacroMap>().is_err());
    }

    #[test]
    fn macros_round_trip_from_str() {
        let map = MACRO_DATA.parse::<MacroMap>().unwrap();

        let str_data = map.to_command_data(2048).unwrap();

        // The serialized map reproduces the live part of the capture (the
        // stale data after the first `0 0` is gone).
        let live_data = MACRO_DATA.split(" 0 0 ").next().unwrap();

        assert!(str_data.starts_with(live_data));

        // Serializing is a fixed point: parsing the serialized data and
        // serializing it again yields the exact same string.
        let str_data_again = str_data
            .parse::<MacroMap>()
            .unwrap()
            .to_command_data(2048)
            .unwrap();

        assert_eq!(str_data, str_data_again);
    }

    #[test]
    fn macros_round_trip_is_stable() {
        // A real-shaped map exercising both key action flavors: a
        // single-byte key down of `A` and a 300 ms wait, followed by a
        // full u16 key down of the mute key, a full u16 tap of `A`, and a
        // random interval of 100 to 300 ms.
        let data = "6 4 2 1 44 0 3 76 226 5 0 4 1 0 100 1 44 0 0 ";

        let map = data.parse::<MacroMap>().unwrap();

        let str_data = map.to_command_data(19).unwrap();

        assert_eq!(format!("{str_data} "), data);
    }

    #[test]
    fn empty_map_erases() {
        let str_data = MacroMap(Vec::new()).to_command_data(8).unwrap();

        assert_eq!(str_data, "255 255 255 255 255 255 255 255");
    }

    #[test]
    fn key_code_too_large_errors() {
        let map = MacroMap(vec![Macro {
            actions: vec![MacroAction::KeyCodeDown(KeyKind::from(19682))],
        }]);

        assert!(matches!(
            map.to_command_data(2048),
            Err(ToCommandDataError::KeyCodeTooLarge(_))
        ));
    }

    #[test]
    fn too_many_macros_errors() {
        // Each macro takes 4 bytes of data (3 action bytes + terminator),
        // so 5 macros plus the final map terminator need 21 bytes.
        let map = MacroMap(vec![
            Macro {
                actions: vec![MacroAction::Wait(1000)],
            };
            5
        ]);

        assert!(matches!(
            map.to_command_data(16),
            Err(ToCommandDataError::TooManyMacros(_))
        ));

        // A map that exactly fits the memory size serializes with no
        // padding bytes left over.
        let exact_fit = MacroMap(vec![
            Macro {
                actions: vec![MacroAction::Wait(1000)],
            };
            3
        ]);

        assert_eq!(
            exact_fit.to_command_data(13).unwrap(),
            "2 3 232 0 2 3 232 0 2 3 232 0 0"
        );
    }
}
