//! Provides parsers for Focus API responses.

use std::str::FromStr;
use winnow::{
    Partial,
    ascii::{line_ending, till_line_ending},
    combinator::{repeat_till, terminated},
    prelude::*,
    stream::Accumulate,
};

/// Error when parsing a Focus API response fails.
#[derive(Clone, Debug, Display, Error)]
pub enum ParseResponseError {
    /// We don't yet have enough data to parse the response.
    #[display("data is incomplete")]
    Incomplete,
    /// We encountered an unrecoverable error parsing the response.
    #[display("failed to parse focus API response:\n{_0}")]
    Err(#[error(not(source))] String),
}

impl ParseResponseError {
    fn from_winnow_err(err: winnow::error::ErrMode<winnow::error::ContextError>) -> Self {
        match err {
            winnow::error::ErrMode::Incomplete(_) => Self::Incomplete,
            err => Self::Err(err.to_string()),
        }
    }
}

/// String response from Focus API command.
#[derive(Clone, Debug, Deref)]
pub struct Response(pub String);

impl FromStr for Response {
    type Err = ParseResponseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let res = response_parser
            .parse_next(&mut Partial::new(s))
            .map_err(ParseResponseError::from_winnow_err)?;

        Ok(Self(res))
    }
}

fn response_parser(input: &mut Partial<&str>) -> ModalResult<String> {
    struct LineAccumulator(String);

    impl Accumulate<&str> for LineAccumulator {
        fn initial(capacity: Option<usize>) -> Self {
            let acc = String::with_capacity(capacity.unwrap_or(1));

            Self(acc)
        }

        fn accumulate(&mut self, acc: &str) {
            if self.0.is_empty() {
                self.0.push_str(acc);
            } else {
                self.0.push('\n');
                self.0.push_str(acc);
            }
        }
    }

    let (res, _): (LineAccumulator, _) = repeat_till(1.., line_parser, ".").parse_next(input)?;

    Ok(res.0)
}

fn line_parser<'s>(input: &mut Partial<&'s str>) -> ModalResult<&'s str> {
    terminated(till_line_ending, line_ending).parse_next(input)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn parser_succeeds_on_single_response() {
        let data = "this.is a test\r\nto test the parser\r\n.";

        let res = data.parse::<Response>().unwrap().0;

        assert_eq!(
            "this.is a test\n\
            to test the parser",
            res
        );
    }

    #[test]
    fn parser_succeeds_on_multiple_response() {
        let data = "\
            this.is a test\r\n\
            to test the parser\r\n.\
            this.is another test\r\n\
            to test the parser\r\n.\
            ";

        let mut input = Partial::new(data);

        let res = response_parser(&mut input).unwrap();

        let new_input = input.into_inner();

        assert_eq!(
            res,
            "this.is a test\n\
            to test the parser",
        );

        assert_eq!(
            new_input,
            "this.is another test\r\n\
            to test the parser\r\n.",
        );
    }

    #[test]
    fn parser_fails_on_incomplete_data() {
        let data = "\
            This is a test\r\n\
            to test the parser\r\n\
            ";

        let mut input = Partial::new(data);

        let res = response_parser(&mut input).unwrap_err();

        assert!(matches! {res, winnow::error::ErrMode::Incomplete(_)});
    }
}
