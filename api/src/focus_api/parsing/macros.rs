//! Types for parsing macros.

use crate::focus_api::parsing::keymap::KeyKind;
use winnow::{
    ModalResult, Parser,
    ascii::dec_uint,
    combinator::{fail, repeat, repeat_till},
    error::{StrContext, StrContextValue},
};

/// Represents a single macro.
#[derive(Clone, Debug)]
pub struct Macro {
    /// The actions this macro will perform.
    pub actions: Vec<MacroAction>,
}

/// The possible actions a macro can perform.
#[derive(Clone, Copy, Debug)]
pub enum MacroAction {
    /// Equivalent to  arapid keydown and keyup.
    #[debug("Press({_0})")]
    Press(KeyKind),
    /// Holds down the key.
    #[debug("KeyDown({_0})")]
    KeyDown(KeyKind),
    /// Releases the held key.
    #[debug("KeyUp({_0})")]
    KeyUp(KeyKind),
    /// We don't yet know what this key action type does.
    Unknown {
        /// The raw action type.
        kind: u8,
        /// The data associated with this action.
        data: RawActionData,
    },
}

/// Raw action data we don't yet know what to do with.
#[derive(Clone, Copy, Debug)]
pub enum RawActionData {
    /// A single byte of data.
    U8(u8),
    /// A single u16.
    OneU16(u16),
    /// Two sequential u16s.
    TwoU16(u16, u16),
}

/// Takes an [`str`] and tries to parse it into a macro map.
///
/// This function returns a [`Vec`] of [`Macro`] in it's first tuple field, and
/// the extra left-over data that might exist after the macro data.
pub fn parse_macros(input: &str) -> Result<(Vec<Macro>, Vec<Vec<Macro>>), String> {
    let ((macros, _), extra, _) = (
        repeat_till(1.., macro_parser, "0 "),
        repeat(
            ..,
            repeat_till::<_, _, Vec<_>, _, _, _, _>(.., macro_parser, "0 ").map(|res| res.0),
        ),
        repeat::<_, _, (), _, _>(.., "255 ")
            .context(StrContext::Label("macros"))
            .context(StrContext::Expected(StrContextValue::Description(
                "macro memory should be done here",
            )))
            .context(StrContext::Expected(StrContextValue::StringLiteral("255"))),
    )
        .parse(input)
        .map_err(|err| err.to_string())?;

    let extra: Vec<_> = extra;

    Ok((macros, extra))
}

fn macro_parser(input: &mut &str) -> ModalResult<Macro> {
    let (actions, _) = repeat_till(1.., action_parser, "0 ")
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
            let data = action_data_two_u16s_parser
                .context(context_label.clone())
                .parse_next(input)?;

            MacroAction::Unknown { kind, data }
        }
        2..=5 => {
            let data = action_data_one_u16_parser
                .context(context_label.clone())
                .parse_next(input)?;

            MacroAction::Unknown { kind, data }
        }
        6 => {
            let data = u8_parser.context(context_label.clone()).parse_next(input)?;

            MacroAction::KeyDown(KeyKind::from(data as u16))
        }
        7 => {
            let data = u8_parser.context(context_label.clone()).parse_next(input)?;

            MacroAction::KeyUp(KeyKind::from(data as u16))
        }
        8 => {
            let data = u8_parser.context(context_label.clone()).parse_next(input)?;

            MacroAction::Press(KeyKind::from(data as u16))
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
    let (res, _) = (dec_uint, " ")
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

    Ok(u16::from_ne_bytes([high_byte, low_byte]))
}

fn action_data_one_u16_parser(input: &mut &str) -> ModalResult<RawActionData> {
    let data = u16_parser
        .context(StrContext::Label("macro action data"))
        .context(StrContext::Expected(StrContextValue::Description(
            "two u16s representing action data",
        )))
        .parse_next(input)?;

    Ok(RawActionData::OneU16(data))
}

fn action_data_two_u16s_parser(input: &mut &str) -> ModalResult<RawActionData> {
    let (data_1, data_2) = (u16_parser, u16_parser)
        .context(StrContext::Label("macro action data"))
        .context(StrContext::Expected(StrContextValue::Description(
            "two u16s representing action data",
        )))
        .parse_next(input)?;

    Ok(RawActionData::TwoU16(data_1, data_2))
}

#[cfg(test)]
mod tests {
    use itertools::Itertools;

    use super::*;

    const MACRO_DATA: &str = "8 44 6 225 8 11 7 225 8 8 8 28 8 54 8 44 6 225 8 7 7 225 8 28 8 10 8 16 8 4 8 23 8 8 8 22 0 6 225 8 23 7 225 8 11 8 12 8 22 8 44 8 12 8 22 8 44 8 4 8 44 8 23 8 8 8 22 8 23 8 55 0 0 6 227 8 80 7 227 8 84 8 5 8 24 8 15 8 15 8 8 8 23 8 8 8 7 8 44 8 15 8 12 8 22 8 23 8 40 0 6 227 8 80 7 227 8 84 8 5 8 4 8 17 8 17 8 8 8 21 8 44 8 28 8 8 8 15 8 15 8 18 8 26 8 40 0 6 227 8 80 7 227 8 84 8 11 8 32 8 40 6 227 8 79 7 227 0 6 227 8 80 7 227 8 84 8 11 8 33 8 40 6 227 8 79 7 227 0 6 227 8 80 7 227 8 84 8 6 8 11 8 8 8 6 8 14 8 15 8 12 8 22 8 23 8 40 0 6 227 8 80 7 227 8 84 8 5 8 4 8 17 8 17 8 8 8 21 8 44 8 10 8 21 8 8 8 8 8 17 8 40 6 227 8 79 7 227 0 6 227 8 80 7 227 8 84 8 5 8 4 8 17 8 17 8 8 8 21 8 44 8 21 8 8 8 7 8 40 6 227 8 79 7 227 0 6 227 8 80 7 227 8 84 8 5 8 4 8 17 8 17 8 8 8 21 8 44 8 5 8 15 8 24 8 8 8 40 6 227 8 79 7 227 0 8 84 8 10 8 12 8 19 8 11 8 28 2 0 200 8 40 0 0 8 10 8 21 8 8 8 8 8 17 8 40 0 8 84 8 5 8 4 8 17 8 17 8 8 8 21 8 44 8 21 8 8 8 7 8 40 0 0 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 255 ";

    #[test]
    fn can_parse_2_u8s_to_u16() {
        let n = 512u16;

        let bytes = n.to_ne_bytes().iter().join(" ");

        let input = format!("{bytes} ");

        let res = u16_parser.parse(&input).unwrap();

        assert_eq!(res, n);
    }

    #[test]
    fn can_parse_macros() {
        let (macros, extra) = parse_macros(MACRO_DATA).unwrap();

        println!("# Macros");
        println!("{:#?}", macros);
        println!("# Extra");
        println!("{:#?}", extra);

        panic!()
    }
}
