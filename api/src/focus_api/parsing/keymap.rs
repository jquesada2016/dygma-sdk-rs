//! Types for parsing keymaps.

pub mod keycode_tables;

use std::str::FromStr;
use winnow::{
    ascii::dec_uint,
    combinator::{opt, repeat, terminated},
    prelude::*,
};

use crate::keycode_tables::KeyKind;

/// Error when parsing a keymap from wire data.
///
/// Carries where the parser gave up within the input it was given, so
/// errors can point at the offending part of the data instead of
/// quoting it whole.
#[derive(Clone, Debug, Display, Error)]
#[display("failed to parse keymap data at byte offset {offset}: expected {expected}")]
pub struct ParseKeymapError {
    /// Byte offset of the failure.
    ///
    /// This is where the parser gave up, which is exact when the
    /// offending token sits in the first layer or at a layer boundary;
    /// inside a later layer it is the start of the layer that failed.
    /// Walk the whitespace-separated tokens forward from here to find
    /// the first one that is not a valid key code.
    pub offset: usize,
    /// What the parser expected at `offset`, from the parser's own
    /// context (e.g. `a key code`).
    pub expected: String,
}

/// A complete raw keymap with individual layers and uninterpreted keycodes.
#[derive(Clone, Debug, Deref)]
pub struct Keymap(pub Vec<[KeyKind; 80]>);

impl FromStr for Keymap {
    type Err = ParseKeymapError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let layers = layers_parser.parse(s).map_err(|err| ParseKeymapError {
            offset: err.offset(),
            expected: crate::focus_api::parsing::parse_expectation(err.inner().context())
                .unwrap_or_else(|| "a key code".to_owned()),
        })?;

        Ok(Self(layers))
    }
}

impl std::fmt::Display for Keymap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut iter = self.0.iter().flatten();

        write!(f, "{}", u16::from(*iter.next().unwrap()))?;

        for key in iter {
            write!(f, " {}", u16::from(*key))?;
        }

        Ok(())
    }
}

fn layers_parser(input: &mut &str) -> ModalResult<Vec<[KeyKind; 80]>> {
    repeat(1.., layer_parser).parse_next(input)
}

fn layer_parser(input: &mut &str) -> ModalResult<[KeyKind; 80]> {
    let layer = repeat::<_, _, Vec<_>, _, _>(80, terminated(key_kind_parser, opt(" ")))
        .parse_next(input)?;
    let layer = layer.try_into().unwrap();

    Ok(layer)
}

fn key_kind_parser(input: &mut &str) -> ModalResult<KeyKind> {
    let key: u16 = dec_uint.parse_next(input)?;

    Ok(KeyKind::from(key))
}

#[cfg(test)]
mod tests {
    use crate::focus_api::parsing::keymap::Keymap;

    #[test]
    fn parse_default_keymap() {
        // Data from a virtual keyboard JSON config
        let data = "41 30 31 32 33 34 35 0 0 51518 37 38 39 45 46 42 43 20 26 8 21 23 128 0 0 28 24 12 18 19 47 48 57 4 22 7 9 10 129 0 0 11 13 14 15 51 52 229 49 29 27 6 25 5 0 0 0 0 17 16 54 55 56 229 224 227 42 76 44 40 226 225 79 81 80 40 17152 74 82 77 41 58 59 60 61 62 63 0 0 64 65 66 67 68 69 42 43 0 82 0 0 0 0 0 0 86 36 37 38 84 0 40 57 80 81 79 0 0 0 0 0 87 33 34 35 85 0 49 225 49 0 0 0 0 0 0 0 0 99 30 31 32 82 229 224 227 226 44 76 40 42 44 79 228 76 17492 39 44 80 81 ";

        let layers = data.parse::<Keymap>().unwrap().0;

        assert_eq!(layers.len(), 2);
    }

    #[test]
    fn parse_custom_keymap() {
        // Data from a virtual keyboard JSON config
        let data = "41 30 31 32 33 34 0 0 0 0 35 36 37 38 39 0 43 20 26 8 21 23 0 0 0 0 28 24 12 18 19 0 57 4 22 7 9 10 17152 0 0 0 11 13 14 15 51 52 53980 29 27 6 25 5 0 0 0 0 17 16 54 55 56 0 53853 17452 44 49467 49209 226 227 0 0 231 76 49209 52028 44 49162 230 41 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 43 85 95 96 97 87 0 0 0 0 75 74 82 77 0 0 0 84 92 93 94 86 83 0 0 0 78 80 81 79 70 0 0 46 89 90 91 99 0 0 0 0 0 0 0 0 0 0 0 0 98 65535 65535 65535 0 0 0 0 0 65535 65535 65535 65535 0 0 58 59 60 61 62 63 65535 65535 64 65 66 67 68 69 0 0 0 0 22710 22709 23785 0 65535 65535 0 0 23663 0 0 65535 0 0 0 22713 22711 22733 23785 0 65535 65535 0 0 23664 20866 20865 0 0 0 0 0 0 0 19682 65535 65535 65535 65535 0 0 0 0 0 0 0 65535 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 65535 65535 0 0 0 0 0 0 0 0 53 2079 2080 2081 2101 0 65535 65535 0 2083 2095 2096 2093 2094 0 0 2078 56 2102 2103 2082 0 65535 65535 0 2084 2086 2087 45 46 0 0 0 0 49 2097 0 65535 65535 65535 65535 0 47 48 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 65535 0 0 0 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 65535 ";

        let layers = data.parse::<Keymap>().unwrap().0;

        assert_eq!(layers.len(), 10);
    }

    #[test]
    fn parse_error_locates_the_failure() {
        // Layer 1 parses; the first token of layer 2 is corrupted. The
        // error carries the offset and the parser's expectation instead
        // of quoting the input.
        let err = "41 30 O".parse::<Keymap>().unwrap_err();

        assert_eq!(err.offset, 6);
        assert_eq!(
            err.to_string(),
            "failed to parse keymap data at byte offset 6: expected a key code"
        );

        // Data that ends where a token was expected fails at the end of
        // the input.
        let err = "41 30 31 ".parse::<Keymap>().unwrap_err();

        assert_eq!(err.offset, 9);
    }

    #[test]
    fn mid_layer_failure_reports_the_layer_start() {
        // A full ten-layer wire string (the real default layer, ten
        // times) with one token of layer 2 corrupted: the parser gives
        // up at the start of the layer that failed rather than at the
        // token. (The offset documents this; consumers walk forward
        // from it to find the token that is not a valid key code.)
        let layer = "41 30 31 32 33 34 35 0 0 51518 37 38 39 45 46 42 43 20 26 8 21 23 128 0 0 28 24 12 18 19 47 48 57 4 22 7 9 10 129 0 0 11 13 14 15 51 52 229 49 29 27 6 25 5 0 0 0 0 17 16 54 55 56 229 224 227 42 76 44 40 226 225 79 81 80 40 17152 74 82 77";
        let mut tokens: Vec<&str> = std::iter::repeat_n(layer, 10)
            .flat_map(str::split_whitespace)
            .collect();

        assert_eq!(tokens.len(), 10 * 80);

        tokens[80 + 20] = "O";

        let data = tokens.join(" ");
        let err = data.parse::<Keymap>().unwrap_err();

        // Layer 2 starts right after layer 1 and its separating space.
        assert_eq!(err.offset, layer.len() + 1);
    }
}
