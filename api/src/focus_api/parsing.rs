//! Provides parsers for the Focus API.

pub mod backup;
pub mod focus_api;
pub mod keymap;
pub mod macros;
pub mod settings;
pub mod superkeys;

/// The most specific expectation from a winnow context stack, for
/// building a parse-failure message.
///
/// Winnow accumulates contexts innermost first as the error propagates
/// outward, so the first `expected` description in the stack is the one
/// closest to the failure; when the stack carries no description, the
/// innermost label is the best that is known.
pub(crate) fn parse_expectation<'a>(
    context: impl Iterator<Item = &'a winnow::error::StrContext>,
) -> Option<String> {
    let mut label = None;

    for context in context {
        match context {
            winnow::error::StrContext::Expected(winnow::error::StrContextValue::Description(
                description,
            )) => return Some((*description).to_owned()),
            winnow::error::StrContext::Label(name) if label.is_none() => label = Some(*name),
            _ => {}
        }
    }

    label.map(str::to_owned)
}
