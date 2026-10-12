//! Context-full error rendering for the CLI binary (issue #16).
//!
//! The library keeps plain `derive_more` error types and this binary
//! wraps them in `error_stack::Report` (the two-layer convention); this
//! module is the third, binary-only layer. At the top-level error exit,
//! the report is rendered as a miette diagnostic: a source snippet with
//! a highlighted span and a label wherever the failure can be located,
//! with the command contexts the failure sites attached to the
//! error-stack chain as `related` items, so what the CLI was doing
//! stays visible. With `NO_COLOR` set (or a non-terminal stderr) the
//! same diagnostic renders without ANSI escapes.
//!
//! Snippets are built at the failure sites — the only places that still
//! hold the raw input — and travel inside the report as opaque
//! attachments, so every error, with or without a snippet, renders
//! through the same exit path.

use std::fmt;
use std::io::IsTerminal;
use std::path::Path;

use dygma_cli::devices::conversions::{ConversionError, LayeredWireError, LayeredWireErrorKind};
use dygma_cli::devices::defy::{
    BackupEntryError, BackupValidationError, CaptureFailure, DefyKeymap, MacroMap,
    ParseKeymapError, ParseMacroMapError, ParseSuperkeyMapError, SuperkeyMap,
};
use dygma_cli::focus_api::parsing::backup::DefyBackup;
use dygma_cli::names::NeuronNameStore;
use error_stack::{AttachmentKind, FrameKind, IntoReport};
use miette::{
    Diagnostic, GraphicalReportHandler, GraphicalTheme, LabeledSpan, NamedSource, SourceCode,
};

use crate::Error;

/// Render width of the graphical handler, in columns. Pinned so output
/// is deterministic (and testable) on every terminal.
const RENDER_WIDTH: usize = 100;

/// How many bytes of context the rendered snippet shows around a span
/// when the source is one huge line (a wire-data string), where miette
/// would otherwise print the whole line.
const SNIPPET_RADIUS: usize = 55;

// =====================================================================================
// Payload: a located failure that rides inside the report
// =====================================================================================

/// A located failure: the source text it happened in, where, and why.
///
/// Built at the failure site (where the raw input is still at hand) and
/// attached to the report as an opaque attachment; the renderer at the
/// top-level exit picks it up and turns it into the miette snippet.
#[derive(Clone, Debug)]
pub(super) struct ErrorSnippet {
    /// Stable identifier for the failure kind, e.g.
    /// `dygma-cli::json-parse`.
    code: &'static str,
    /// One-line headline for the diagnostic.
    message: String,
    /// Display name of the source, e.g. the file path or the argument
    /// the data came from.
    source_name: String,
    /// The source text the span indexes into.
    source_text: String,
    /// Byte offset and length of the highlighted span.
    span: (usize, usize),
    /// What the span points at.
    label: String,
    /// What the CLI was doing when it failed ("parsing file contents",
    /// the path, …) — the context half of the rendered related list.
    /// The library error detail lives in the message, label, and span.
    context: Vec<String>,
    /// Optional advice beyond the label.
    help: Option<String>,
}

impl ErrorSnippet {
    /// Builds a diagnostic from this snippet plus the command root.
    fn into_diagnostic(self, root: String) -> CliDiagnostic {
        let mut related: Vec<_> = self.context.into_iter().map(RelatedDiagnostic).collect();

        related.push(RelatedDiagnostic(root));

        CliDiagnostic {
            code: self.code,
            message: self.message,
            source: Some(NamedSource::new(self.source_name, self.source_text)),
            span: Some(self.span),
            label: Some(self.label),
            help: self.help,
            related,
        }
    }
}

// =====================================================================================
// Rendering: the single exit path for every CLI error
// =====================================================================================

/// One line of the error-stack context chain, with whether it says
/// something more specific than the generic command context.
struct ChainItem {
    /// The frame's message.
    message: String,
    /// Whether the frame is not the generic command context.
    specific: bool,
}

/// The error-stack context chain of a report, outermost frame first.
/// Location attachments and opaque attachments (the snippets
/// themselves) are skipped.
fn chain(report: &error_stack::Report<Error>) -> Vec<ChainItem> {
    report
        .frames()
        .filter(|frame| frame.downcast_ref::<std::panic::Location>().is_none())
        .filter_map(|frame| match frame.kind() {
            FrameKind::Context(context) => Some(ChainItem {
                message: context.to_string(),
                specific: !frame.is::<Error>(),
            }),
            FrameKind::Attachment(AttachmentKind::Printable(attachment)) => Some(ChainItem {
                message: attachment.to_string(),
                specific: true,
            }),
            // `AttachmentKind` is `#[non_exhaustive]`; opaque
            // attachments (the snippets) stay out of the chain.
            _ => None,
        })
        .collect()
}

/// The most specific message in the chain: the deepest frame that is
/// not the generic command context. For a bare report that is the
/// generic context itself.
fn headline(report: &error_stack::Report<Error>) -> String {
    let chain = chain(report);

    chain
        .iter()
        .rev()
        .find(|item| item.specific)
        .or_else(|| chain.last())
        .map(|item| item.message.clone())
        .unwrap_or_else(|| "something went wrong running the command".to_owned())
}

/// The chain's messages minus one occurrence of `headline`, so the
/// headline is not rendered twice.
fn related_excluding(
    report: &error_stack::Report<Error>,
    headline: &str,
) -> Vec<RelatedDiagnostic> {
    let mut excluded = false;

    chain(report)
        .into_iter()
        .map(|item| item.message)
        .filter(|message| {
            if !excluded && message == headline {
                excluded = true;
                false
            } else {
                true
            }
        })
        .map(RelatedDiagnostic)
        .collect()
}

/// Renders a report the way the CLI exits with it: as a miette
/// diagnostic with the error-stack context chain as related items.
pub(super) fn render(report: &error_stack::Report<Error>) -> String {
    render_with_theme(report, exit_theme())
}

/// Renders a report with an explicit theme. The no-color theme is the
/// plain-text fallback the CLI uses when miette's graphical rendering
/// is unavailable.
fn render_with_theme(report: &error_stack::Report<Error>, theme: GraphicalTheme) -> String {
    let diagnostic = match report
        .frames()
        .find_map(|frame| frame.downcast_ref::<ErrorSnippet>())
    {
        // The snippet carries the failure and its CLI context; the
        // command root closes the related list. The library error
        // detail the rest of the chain holds is already in the
        // snippet's message, label, and span.
        Some(snippet) => snippet.clone().into_diagnostic(Error.to_string()),
        None => {
            let headline = headline(report);

            CliDiagnostic {
                code: "dygma-cli::error",
                message: headline.clone(),
                source: None,
                span: None,
                label: None,
                help: None,
                related: related_excluding(report, &headline),
            }
        }
    };

    let handler = GraphicalReportHandler::new_themed(theme).with_width(RENDER_WIDTH);
    let mut out = String::new();

    handler
        .render_report(&mut out, &diagnostic)
        .expect("rendering a diagnostic into a String cannot fail");

    out
}

/// The theme for a real CLI exit: unicode with ANSI color on a
/// terminal, and the plain (no-color) rendering everywhere else —
/// `NO_COLOR` set to a non-empty value (the no-color.org convention: an
/// empty value does not count), or stderr not a terminal.
fn exit_theme() -> GraphicalTheme {
    theme_for(
        std::env::var_os("NO_COLOR").as_deref(),
        std::io::stderr().is_terminal(),
    )
}

/// The theme decision, factored out of the environment so it can be
/// pinned: `NO_COLOR` set to a non-empty value or a non-terminal
/// stderr selects the plain rendering.
fn theme_for(no_color: Option<&std::ffi::OsStr>, is_tty: bool) -> GraphicalTheme {
    match no_color {
        Some(value) if !value.is_empty() => GraphicalTheme::unicode_nocolor(),
        _ if is_tty => GraphicalTheme::unicode(),
        _ => GraphicalTheme::unicode_nocolor(),
    }
}

/// One related item: a single line of the error-stack context chain.
#[derive(Debug)]
struct RelatedDiagnostic(String);

impl fmt::Display for RelatedDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for RelatedDiagnostic {}

impl Diagnostic for RelatedDiagnostic {}

/// The CLI's miette diagnostic: a headline, an optional located snippet,
/// and the error-stack context chain as related items.
#[derive(Debug)]
struct CliDiagnostic {
    /// Stable identifier for the failure kind.
    code: &'static str,
    /// One-line headline.
    message: String,
    /// The located source, when the failure carries a snippet.
    source: Option<NamedSource<String>>,
    /// Byte offset and length of the highlighted span.
    span: Option<(usize, usize)>,
    /// What the span points at.
    label: Option<String>,
    /// Optional advice beyond the label.
    help: Option<String>,
    /// The error-stack context chain, outermost first.
    related: Vec<RelatedDiagnostic>,
}

impl fmt::Display for CliDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CliDiagnostic {}

impl Diagnostic for CliDiagnostic {
    fn code<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        Some(Box::new(self.code))
    }

    fn source_code(&self) -> Option<&dyn SourceCode> {
        self.source.as_ref().map(|source| source as &dyn SourceCode)
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        let (offset, len) = self.span?;

        Some(Box::new(std::iter::once(LabeledSpan::new(
            self.label.clone(),
            offset,
            len,
        ))))
    }

    fn help<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        self.help
            .as_ref()
            .map(|help| Box::new(help) as Box<dyn fmt::Display>)
    }

    fn related<'a>(&'a self) -> Option<Box<dyn Iterator<Item = &'a dyn Diagnostic> + 'a>> {
        Some(Box::new(
            self.related.iter().map(|item| item as &dyn Diagnostic),
        ))
    }
}

// =====================================================================================
// Failure sites: snippet builders and the reports they attach to
// =====================================================================================

/// The domain a JSON file holds, for the help hint of its parse
/// diagnostic.
pub(super) trait JsonDomainHint {
    /// One-line advice shown under a JSON parse failure.
    const HINT: Option<&'static str> = None;
}

impl JsonDomainHint for DefyKeymap {
    const HINT: Option<&'static str> = Some(
        "keys are human-readable names like `A` or `Left Ctrl` — check one with \
         `dygma-cli key-code parse`",
    );
}

impl JsonDomainHint for SuperkeyMap {
    const HINT: Option<&'static str> =
        Some("superkeys are six space-separated actions per superkey; `1` means no action");
}

impl JsonDomainHint for MacroMap {
    const HINT: Option<&'static str> =
        Some("macros are space-separated bytes; see `dygma-cli macros to-command-data`");
}

impl JsonDomainHint for DefyBackup {
    const HINT: Option<&'static str> = Some(
        "backups are Bazecor-compatible JSON files — compare with a file written \
         by `dygma-cli backup create`",
    );
}

impl JsonDomainHint for NeuronNameStore {
    const HINT: Option<&'static str> =
        Some("the names store maps chip ids to neuron names — see `dygma-cli names list`");
}

/// Builds the report for a JSON file that failed to parse, with the
/// located snippet attached. The context chain matches the chain the
/// parse failure produced before this module existed.
pub(super) fn json_parse_report<T>(
    path: &Path,
    data: &[u8],
    err: serde_json::Error,
) -> error_stack::Report<Error>
where
    T: JsonDomainHint,
{
    let context = vec![
        "parsing file contents".to_owned(),
        path.to_string_lossy().into_owned(),
    ];
    let snippet = json_snippet::<T>(path, &String::from_utf8_lossy(data), &err, context.clone());

    err.into_report()
        .change_context(Error)
        .attach("parsing file contents")
        .attach(path.to_string_lossy().into_owned())
        .attach_opaque(snippet)
}

/// Locates a JSON parse failure in the raw file text: serde's line and
/// column, refined by a text search when the error names the offending
/// string token (serde reports where parsing *stopped* — the token
/// after the offending one — so the search is what pins the token
/// itself).
fn json_snippet<T: JsonDomainHint>(
    path: &Path,
    text: &str,
    err: &serde_json::Error,
    context: Vec<String>,
) -> ErrorSnippet {
    let message = err.to_string();
    let position = line_col_to_offset(text, err.line(), err.column());

    // Custom deserializer messages lead with the backquoted token they
    // rejected (`CTRl` is not a parsable key); other shapes name types,
    // not tokens, and a blind search could pin an unrelated occurrence.
    let refined = message
        .starts_with('`')
        .then(|| {
            let needle = format!("\"{}\"", backquoted(&message)?);

            text[..position]
                .rfind(&needle)
                .map(|start| (start, needle.len()))
        })
        .flatten();

    let (span, label) = match refined {
        Some(found) => (found, trim_position_suffix(&message).to_owned()),
        None => {
            // serde's column points at the offending character for
            // syntax errors (a stray comma, a missing colon) and just
            // past the consumed value for the other shapes — anchor the
            // one-byte span accordingly.
            let span = match err.classify() {
                serde_json::error::Category::Syntax => (position.min(text.len()), 1),
                _ => (position.saturating_sub(1).min(text.len()), 1),
            };

            (span, trim_position_suffix(&message).to_owned())
        }
    };

    ErrorSnippet {
        code: "dygma-cli::json-parse",
        message: format!("invalid JSON in {}", path.display()),
        source_name: path.display().to_string(),
        source_text: text.to_owned(),
        span,
        label,
        context,
        help: T::HINT.map(str::to_owned),
    }
}

/// The wire formats the CLI parses from `--keymap`-style arguments.
#[derive(Clone, Copy)]
pub(super) enum WireFormat {
    /// A `keymap.custom` data string (80 key codes per layer).
    Keymap,
    /// A `superkeys.map` data string (6 values per superkey).
    Superkeys,
    /// A `macros.map` data string (bytes per macro).
    Macros,
}

impl WireFormat {
    /// The argument the data came from, as the user typed it.
    fn argument(self) -> &'static str {
        match self {
            Self::Keymap => "--keymap",
            Self::Superkeys => "--superkeys",
            Self::Macros => "--macros",
        }
    }

    /// The human noun for the format, used in messages.
    fn noun(self) -> &'static str {
        match self {
            Self::Keymap => "keymap",
            Self::Superkeys => "superkeys",
            Self::Macros => "macros",
        }
    }

    /// Stable identifier for the failure kind.
    fn code(self) -> &'static str {
        match self {
            Self::Keymap => "dygma-cli::keymap::wire-parse",
            Self::Superkeys => "dygma-cli::superkeys::wire-parse",
            Self::Macros => "dygma-cli::macros::wire-parse",
        }
    }

    /// Advice shown under the diagnostic.
    fn help(self) -> &'static str {
        match self {
            Self::Keymap => {
                "keymap wire data is space-separated u16 key codes — 80 per layer, \
                 10 layers per keymap.custom write"
            }
            Self::Superkeys => {
                "superkeys wire data is space-separated u16 values — six actions per \
                 superkey, then a `0` terminator"
            }
            Self::Macros => {
                "macros wire data is space-separated u8 values — one byte per action, \
                 ending with a `0` terminator"
            }
        }
    }
}

/// What a wire parse rejected, as the CLI sees it: the parser's
/// complaint plus the byte offset it located, when it located one.
pub(super) struct WireFailure {
    /// Byte offset of the failure within the parsed input.
    offset: Option<usize>,
    /// Whether the parser consumed the offending byte before rejecting
    /// it (the unsupported-action-kind rejection), so the offset marks
    /// where parsing stopped, just past the offender — scanning forward
    /// from it cannot find the failure and would mis-attribute a later
    /// bad token.
    consumed_offender: bool,
    /// The library error's own message.
    reason: String,
}

impl WireFailure {
    /// Extracts the failure from a keymap parse.
    pub(super) fn keymap(err: &ParseKeymapError) -> Self {
        match err {
            ParseKeymapError::MalformedData(inner) => Self {
                offset: Some(inner.offset),
                consumed_offender: false,
                reason: err.to_string(),
            },
            ParseKeymapError::IncorrectLayerCount(_) => Self {
                offset: None,
                consumed_offender: false,
                reason: err.to_string(),
            },
        }
    }

    /// Extracts the failure from a superkey map parse.
    pub(super) fn superkeys(err: &ParseSuperkeyMapError) -> Self {
        match err {
            ParseSuperkeyMapError::MalformedData(inner) => Self {
                offset: Some(inner.offset),
                consumed_offender: false,
                reason: err.to_string(),
            },
            ParseSuperkeyMapError::TooManySuperkeys(_) => Self {
                offset: None,
                consumed_offender: false,
                reason: err.to_string(),
            },
        }
    }

    /// Extracts the failure from a macro map parse.
    pub(super) fn macros(err: &ParseMacroMapError) -> Self {
        match err {
            ParseMacroMapError::MalformedData(inner) => Self {
                offset: Some(inner.offset),
                consumed_offender: inner.unsupported_kind,
                reason: err.to_string(),
            },
            ParseMacroMapError::TooManyMacros(_) => Self {
                offset: None,
                consumed_offender: false,
                reason: err.to_string(),
            },
        }
    }
}

/// Where in wire data a parse failure renders, decided once for both
/// failure sites — the direct `--keymap`-style arguments and the
/// backup entries — so neither can forget a case.
///
/// The decision, in the order the parser's own view of the failure
/// allows:
///
/// * No located failure (a layer-count or map-size rejection):
///   [`WireSite::Unlocated`], pointing at the first token.
/// * A consumed-offender rejection (the unsupported macro action
///   kind): the offset marks where parsing stopped, just past the
///   rejected kind, so scanning forward from it could only
///   mis-attribute a later bad token — the site is the stop, or the
///   end of the data when nothing follows.
/// * Otherwise the keymap parser gives up at the start of the layer
///   that failed rather than at the token, so the scan looks for the
///   first token from the offset on that is not a valid value of the
///   format's width: when it finds one, that token is the failure;
///   when every token is valid, the site is the stop where parsing
///   gave up — or the end of the data when nothing follows.
enum WireSite<'data> {
    /// The offending token — the failure itself — at its offset.
    Offender {
        /// Byte offset of the token within the data.
        offset: usize,
        /// The offending token.
        token: &'data str,
    },
    /// Parsing stopped at `offset` with data still ahead: `token` is
    /// the next token; no single token is the failure, so the span
    /// marks where parsing stopped.
    Stop {
        /// Byte offset of the token within the data.
        offset: usize,
        /// The token parsing stopped at.
        token: &'data str,
    },
    /// The data ended where a value was expected.
    End,
    /// The parser located no failure; `token` is the data's first
    /// token, when there is one, for the span to point at.
    Unlocated {
        /// The first token and its offset, when the data has one.
        token: Option<(usize, &'data str)>,
    },
}

/// Runs the one wire-failure decision over `text`: `located` is the
/// parser's view of the failure — the format, the byte offset it
/// located, and whether it consumed the offending byte before
/// rejecting it — or `None` when it located nothing.
fn wire_site<'data>(
    text: &'data str,
    located: Option<(WireFormat, usize, bool)>,
) -> WireSite<'data> {
    let Some((format, offset, consumed_offender)) = located else {
        return WireSite::Unlocated {
            token: token_at(text, 0),
        };
    };

    // A consumed-offender rejection (the unsupported macro action
    // kind): scanning forward from the stop could only mis-attribute a
    // later bad token, so skip the scan.
    if !consumed_offender && let Some((token_offset, token)) = wire_offender(text, offset, format) {
        return WireSite::Offender {
            offset: token_offset,
            token,
        };
    }

    // No offender at or after the offset: the stop when data follows
    // it, the end of the data when it does not.
    match token_at(text, offset) {
        Some((token_start, token)) => WireSite::Stop {
            offset: token_start,
            token,
        },
        None => WireSite::End,
    }
}

/// Builds the report for a `--keymap`-style wire string that failed to
/// parse, with the located snippet attached. The label names the wire
/// data for what it is — these arguments are not JSON files.
pub(super) fn wire_parse_report(
    format: WireFormat,
    input: &str,
    failure: WireFailure,
) -> error_stack::Report<Error> {
    let context = vec![format!("parsing the `{}` wire data", format.argument())];

    Error
        .into_report()
        .attach(context[0].clone())
        .attach(failure.reason.clone())
        .attach_opaque(wire_snippet(format, input, &failure, context))
}

/// Assembles the diagnostic for a wire parse failure: the failure site
/// comes from the one shared decision, dressed in this path's
/// vocabulary — the offending token, the layer for keymap data, and
/// the parser's complaint.
fn wire_snippet(
    format: WireFormat,
    input: &str,
    failure: &WireFailure,
    context: Vec<String>,
) -> ErrorSnippet {
    let located = failure
        .offset
        .map(|offset| (format, offset, failure.consumed_offender));

    let (span, label, message, source_text) = match wire_site(input, located) {
        // The offending token, highlighted in an excerpt, because wire
        // data is one huge line.
        WireSite::Offender { offset, token } => {
            // Counting the tokens before the failure (not the
            // spaces) keeps the layer number right for device
            // captures, which end every layer with a trailing
            // space, and for any mixed separator runs.
            let layer = match format {
                WireFormat::Keymap => format!(
                    " (layer {})",
                    input[..offset].split_whitespace().count() / 80 + 1
                ),
                _ => String::new(),
            };

            let label = if !token.bytes().all(|byte| byte.is_ascii_digit()) {
                "not a number".to_owned()
            } else {
                match format {
                    WireFormat::Macros => "out of range for a u8".to_owned(),
                    _ => "out of range for a u16".to_owned(),
                }
            };

            let (source_text, span) = excerpt(input, offset, token.len());

            (
                span,
                label,
                format!(
                    "invalid token `{token}` in {} wire data{layer}",
                    format.noun()
                ),
                source_text,
            )
        }
        WireSite::Stop { offset, token } => {
            let (source_text, span) = excerpt(input, offset, token.len());

            (
                span,
                "parsing stopped here".to_owned(),
                failure.reason.clone(),
                source_text,
            )
        }
        WireSite::End => {
            let (source_text, span) = excerpt(input, input.len(), 0);

            // A consumed-offender rejection keeps the reason as the
            // message even at the end of the data: the true failure is
            // the consumed byte, not the end.
            let message = if failure.consumed_offender {
                failure.reason.clone()
            } else {
                format!(
                    "the {} wire data ended where a value was expected",
                    format.noun()
                )
            };

            (span, "the data ends here".to_owned(), message, source_text)
        }
        // No located failure (a layer-count or map-size rejection):
        // point at the first token and let the reason carry the message.
        WireSite::Unlocated { token } => {
            let (source_text, span) = match token {
                Some((start, token)) => excerpt(input, start, token.len()),
                None => excerpt(input, 0, 0),
            };

            let label = match format {
                WireFormat::Keymap => {
                    let numbers = input.split_whitespace().count();

                    format!(
                        "{numbers} codes = {} layers; a `keymap.custom` write \
                         carries exactly 800 = 10",
                        numbers / 80
                    )
                }
                _ => format!("the `{}` argument", format.argument()),
            };

            (span, label, failure.reason.clone(), source_text)
        }
    };

    // The library reason rides in the rendered related list unless the
    // headline already is that reason.
    let mut context = context;

    if message != failure.reason {
        context.push(failure.reason.clone());
    }

    ErrorSnippet {
        code: format.code(),
        message,
        source_name: format!("{} argument (excerpt)", format.argument()),
        source_text,
        span,
        label,
        context,
        help: Some(format.help().to_owned()),
    }
}

/// Builds the report for a backup that failed restore validation, with
/// the located snippet attached. The raw file text is re-read here
/// because this is where the path is at hand; when it cannot be read,
/// the diagnostic renders without a snippet.
pub(super) fn backup_validation_report(
    path: &Path,
    err: BackupValidationError,
) -> error_stack::Report<Error> {
    let text = std::fs::read_to_string(path).ok();

    backup_validation_report_with_text(path, text.as_deref(), err)
}

/// Builds the validation-failure report from file text the caller
/// already holds, so it can be exercised without touching the
/// filesystem; when the text is unavailable, the diagnostic renders
/// without a snippet.
fn backup_validation_report_with_text(
    path: &Path,
    text: Option<&str>,
    err: BackupValidationError,
) -> error_stack::Report<Error> {
    let context_line = format!("validating the backup in {}", path.display());
    let report = err
        .clone()
        .into_report()
        .change_context(Error)
        .attach(context_line.clone());

    match text {
        Some(text) => report.attach_opaque(backup_validation_snippet(
            path,
            text,
            &err,
            vec![context_line],
        )),
        None => report,
    }
}

/// Builds the report for an interrupted `backup create`: the capture
/// failure itself, plus the recovery story — the partial file that was
/// saved, what it holds and misses, and how to complete or force it.
/// `capture_context` is what was being attempted when the capture
/// was interrupted ("capturing the backup from the Defy" for a fresh
/// create, "completing the partial capture in <path>" for
/// `--retry-missing`) — the caller computes it, so the story always
/// matches the run that failed. A device failure has no source text to
/// span, so this renders as the plain context chain.
pub(super) fn interrupted_capture_report(
    path: &Path,
    partial: &DefyBackup,
    source: CaptureFailure,
    capture_context: String,
) -> error_stack::Report<Error> {
    let missing_line = match partial.missing_commands() {
        [] => "the capture was interrupted after every command but before the \
               backup could be assembled completely"
            .to_owned(),
        missing => format!("missing: {}", missing.join(", ")),
    };

    source
        .into_report()
        .change_context(Error)
        .attach(capture_context)
        .attach(format!(
            "a PARTIAL capture was saved to {} ({} command(s) captured, {} missing)",
            path.display(),
            partial.backup.len(),
            partial.missing_commands().len()
        ))
        .attach(missing_line)
        .attach(
            "`backup restore` refuses this file unless --allow-partial is passed; \
             complete it with `dygma-cli backup create --retry-missing <file>` \
             (the capture commands are reads, so this is safe)",
        )
}

/// Locates a backup restore-validation failure in the raw JSON text.
fn backup_validation_snippet(
    path: &Path,
    text: &str,
    err: &BackupValidationError,
    context: Vec<String>,
) -> ErrorSnippet {
    match err {
        BackupValidationError::NotADefyBackup { product } => ErrorSnippet {
            code: "dygma-cli::backup::not-a-defy",
            message: format!("the backup is for a `{product}`, not a Defy"),
            source_name: path.display().to_string(),
            source_text: text.to_owned(),
            span: locate_product_value(text, product).unwrap_or((0, 0)),
            label: format!("this backup was captured from a `{product}`"),
            context,
            help: Some(
                "the SDK speaks the Defy wire formats only — restoring another \
                 product's backup would write foreign data to the device"
                    .to_owned(),
            ),
        },
        BackupValidationError::PartialCapture { .. } => {
            // The refusal's wording lives on the error type's Display
            // (one source, including the empty-missing case); the
            // snippet renders it verbatim.
            let message = err.to_string();

            ErrorSnippet {
                code: "dygma-cli::backup::partial-capture",
                message,
                source_name: path.display().to_string(),
                source_text: text.to_owned(),
                span: locate_partial_marker(text).unwrap_or((0, 0)),
                label: "this marker says the file was saved from a failed capture".to_owned(),
                context,
                help: Some(
                    "restoring it would write a half-captured configuration to the \
                     device — complete the capture first (`dygma-cli backup create \
                     --retry-missing <file>`), re-capture, or pass `--allow-partial` \
                     to restore what the file holds"
                        .to_owned(),
                ),
            }
        }
        BackupValidationError::InvalidEntry { command, source } => {
            let (span, label) = locate_data_value(text, command)
                .map(|(start, len)| entry_span_and_label(&text[start..start + len], start, source))
                .unwrap_or(((0, 0), source.to_string()));

            // The `data` value is one huge line; show an excerpt around
            // the span instead of the whole line.
            let (source_text, span) = excerpt(text, span.0, span.1);

            ErrorSnippet {
                code: "dygma-cli::backup::invalid-entry",
                message: format!("the `{command}` entry failed restore validation: {source}"),
                source_name: format!("{} — `{command}` `data` (excerpt)", path.display()),
                source_text,
                span,
                label,
                context,
                help: Some(
                    "backups hold whole device maps — re-capture with \
                     `dygma-cli backup create`"
                        .to_owned(),
                ),
            }
        }
    }
}

/// The span and label for a backup entry whose `data` failed
/// validation, with the span offset based at `start` (the beginning of
/// the entry's data value in the file).
fn entry_span_and_label(
    data: &str,
    start: usize,
    source: &BackupEntryError,
) -> ((usize, usize), String) {
    match wire_site(data, wire_failure(source)) {
        WireSite::Offender { offset, token } => (
            (start + offset, token.len()),
            format!("not valid wire data: `{token}`"),
        ),
        WireSite::Stop { offset, token } => (
            (start + offset, token.len()),
            "parsing stopped here".to_owned(),
        ),
        WireSite::End => (
            (start, 0),
            "the data ended where a value was expected".to_owned(),
        ),
        // Map-size rejections and the keymap layer count carry no
        // offset. A keymap whose layer count is wrong parses as wire
        // data, so the count is the whole story: say it with the
        // numbers. The rest let the source's own message carry it.
        WireSite::Unlocated { token } => {
            if let BackupEntryError::Keymap(ParseKeymapError::IncorrectLayerCount(_)) = source {
                let numbers = data.split_whitespace().count();
                let first_token = token.map_or(0, |(_, token)| token.len());

                return (
                    (start, first_token),
                    format!(
                        "{numbers} codes = {} layers; a `keymap.custom` write carries \
                         exactly 800 = 10",
                        numbers / 80
                    ),
                );
            }

            ((start, 0), source.to_string())
        }
    }
}

/// The wire format and failure location of a backup entry's parse
/// failure, when it located one — plus whether the parser consumed the
/// offending byte before rejecting it.
fn wire_failure(source: &BackupEntryError) -> Option<(WireFormat, usize, bool)> {
    match source {
        BackupEntryError::Keymap(ParseKeymapError::MalformedData(inner)) => {
            Some((WireFormat::Keymap, inner.offset, false))
        }
        BackupEntryError::Superkeys(ParseSuperkeyMapError::MalformedData(inner)) => {
            Some((WireFormat::Superkeys, inner.offset, false))
        }
        BackupEntryError::Macros(ParseMacroMapError::MalformedData(inner)) => {
            Some((WireFormat::Macros, inner.offset, inner.unsupported_kind))
        }
        _ => None,
    }
}

// =====================================================================================
// Cross-product conversion failures
// =====================================================================================

/// Builds the report for a backup that failed cross-product
/// conversion, with the located snippet attached. The raw file text is
/// re-read here because this is where the path is at hand; when it
/// cannot be read, the diagnostic renders without a snippet.
pub(super) fn conversion_report(path: &Path, err: ConversionError) -> error_stack::Report<Error> {
    let text = std::fs::read_to_string(path).ok();

    conversion_report_with_text(path, text.as_deref(), err)
}

/// Builds the conversion-failure report from file text the caller
/// already holds, so it can be exercised without touching the
/// filesystem; when the text is unavailable, the diagnostic renders
/// without a snippet.
fn conversion_report_with_text(
    path: &Path,
    text: Option<&str>,
    err: ConversionError,
) -> error_stack::Report<Error> {
    let context_line = format!("converting the backup in {}", path.display());
    let report = err
        .clone()
        .into_report()
        .change_context(Error)
        .attach(context_line.clone());

    match text {
        Some(text) => {
            report.attach_opaque(conversion_snippet(path, text, &err, vec![context_line]))
        }
        None => report,
    }
}

/// Locates a conversion failure in the raw backup JSON text.
fn conversion_snippet(
    path: &Path,
    text: &str,
    err: &ConversionError,
    context: Vec<String>,
) -> ErrorSnippet {
    match err {
        ConversionError::MissingDescriptor | ConversionError::NoProductName => ErrorSnippet {
            code: "dygma-cli::backup-convert::no-product",
            message: err.to_string(),
            source_name: path.display().to_string(),
            source_text: text.to_owned(),
            span: (0, 0),
            label: "this backup's neuron object carries no usable device descriptor".to_owned(),
            context,
            help: Some(
                "a convertible backup must name its product in \
                 `neuron.device.info.product`, the field Bazecor's own \
                 restore switches its conversions on"
                    .to_owned(),
            ),
        },
        ConversionError::UnknownProduct { product } => ErrorSnippet {
            code: "dygma-cli::backup-convert::unknown-product",
            message: err.to_string(),
            source_name: path.display().to_string(),
            source_text: text.to_owned(),
            span: locate_product_value(text, product).unwrap_or((0, 0)),
            label: format!("this backup names the unknown product `{product}`"),
            context,
            help: Some(supported_conversions_help().to_owned()),
        },
        ConversionError::UnsupportedConversion { from, to } => ErrorSnippet {
            code: "dygma-cli::backup-convert::unsupported",
            message: err.to_string(),
            source_name: path.display().to_string(),
            source_text: text.to_owned(),
            span: locate_product_value(text, from.name()).unwrap_or((0, 0)),
            label: format!("this backup was captured from a `{from}`"),
            context,
            help: Some(format!(
                "{}, so a {from} → {to} conversion needs its own mapping work",
                supported_conversions_help()
            )),
        },
        ConversionError::SameProduct { product } => ErrorSnippet {
            code: "dygma-cli::backup-convert::same-product",
            message: err.to_string(),
            source_name: path.display().to_string(),
            source_text: text.to_owned(),
            span: locate_product_value(text, product.name()).unwrap_or((0, 0)),
            label: format!("this backup is already a `{product}` backup"),
            context,
            help: Some(supported_conversions_help().to_owned()),
        },
        ConversionError::MissingCommand { .. } => {
            // The command is absent — there is nothing in the file to
            // point at, so the diagnostic renders without a span.
            ErrorSnippet {
                code: "dygma-cli::backup-convert::missing-command",
                message: err.to_string(),
                source_name: path.display().to_string(),
                source_text: text.to_owned(),
                span: (0, 0),
                label: "converting needs this captured command".to_owned(),
                context,
                help: Some(
                    "a convertible backup must carry `keymap.custom`, \
                     `colormap.map`, and `palette`; re-capture it with \
                     `dygma-cli backup create` or in Bazecor"
                        .to_owned(),
                ),
            }
        }
        ConversionError::DuplicateCommand { command, count } => {
            // The multiplicity is the story, and it spans the whole
            // capture list — there is no single byte to highlight, so
            // the diagnostic renders without a span.
            ErrorSnippet {
                code: "dygma-cli::backup-convert::duplicate-command",
                message: err.to_string(),
                source_name: path.display().to_string(),
                source_text: text.to_owned(),
                span: (0, 0),
                label: format!("the backup captures `{command}` {count} times"),
                context,
                help: Some(
                    "a later duplicate would survive the conversion \
                     unconverted (Bazecor's conversion takes the first \
                     and silently drops the rest); remove the stale \
                     duplicate or re-capture"
                        .to_owned(),
                ),
            }
        }
        ConversionError::InvalidEntry { command, source } => {
            let data_start = locate_data_value(text, command)
                .map(|(start, _)| start)
                .unwrap_or(0);
            let (span, label) = entry_error_span_and_label(data_start, source);

            // The `data` value is one huge line; show an excerpt
            // around the span instead of the whole line.
            let (source_text, span) = excerpt(text, span.0, span.1);

            ErrorSnippet {
                code: "dygma-cli::backup-convert::invalid-entry",
                message: format!("the `{command}` entry is not convertible: {source}"),
                source_name: format!("{} — `{command}` `data` (excerpt)", path.display()),
                source_text,
                span,
                label,
                context,
                help: Some(
                    "converting rewrites the entry into the target product's \
                     layer shape, so the entry must hold the source product's \
                     shape exactly"
                        .to_owned(),
                ),
            }
        }
    }
}

/// The supported-conversions sentence shared by the product-shaped
/// diagnostics.
fn supported_conversions_help() -> &'static str {
    "the supported conversions are Defy → Sonsei and Sonsei → Defy"
}

/// The span and label for a converted entry whose data failed the
/// shape check, based at `data_start` (the beginning of the entry's
/// `data` value in the file).
fn entry_error_span_and_label(
    data_start: usize,
    source: &LayeredWireError,
) -> ((usize, usize), String) {
    match &source.kind {
        LayeredWireErrorKind::TokenNotANumber { offset, token } => (
            (data_start + offset, token.len()),
            format!("`{token}` is not a number"),
        ),
        LayeredWireErrorKind::TokenOutOfRange { offset, token, max } => (
            (data_start + offset, token.len()),
            format!("`{token}` exceeds the {max} ceiling"),
        ),
        LayeredWireErrorKind::WrongValueCount { found, expected } => (
            // No single token to point at — the counts are the story.
            (data_start, 0),
            format!("the data holds {found} values, not {expected}"),
        ),
    }
}

// =====================================================================================
// Text-location helpers
// =====================================================================================

/// Byte offset of a 1-based (line, column) pair.
fn line_col_to_offset(text: &str, line: usize, column: usize) -> usize {
    let mut offset = 0;

    for (index, line_text) in text.split('\n').enumerate() {
        if index + 1 == line {
            return offset + column.saturating_sub(1).min(line_text.len());
        }

        offset += line_text.len() + 1;
    }

    text.len()
}

/// Extracts a ``…``-quoted token from a message, if any.
fn backquoted(message: &str) -> Option<&str> {
    let start = message.find('`')? + 1;
    let end = message[start..].find('`')? + start;

    Some(&message[start..end])
}

/// Strips serde_json's trailing ` at line L column C` from an error
/// message: the rendered snippet's gutter shows the position already.
fn trim_position_suffix(message: &str) -> &str {
    let Some(suffix) = message.rfind(" at line ") else {
        return message;
    };

    let is_position = message[suffix + " at line ".len()..]
        .split_once(" column ")
        .is_some_and(|(line, column)| {
            !line.is_empty()
                && !column.is_empty()
                && line.bytes().all(|byte| byte.is_ascii_digit())
                && column.bytes().all(|byte| byte.is_ascii_digit())
        });

    match is_position {
        true => &message[..suffix],
        false => message,
    }
}

/// The whitespace-delimited token at `offset`, with its offset from the
/// start of `text`, if there is one.
fn token_at(text: &str, offset: usize) -> Option<(usize, &str)> {
    let slice = &text[offset.min(text.len())..];
    let token_start = slice.len() - slice.trim_start().len();
    let token = slice
        .trim_start()
        .split([' ', '\n', '\t'])
        .next()
        .unwrap_or_default();

    (!token.is_empty()).then_some((offset + token_start, token))
}

/// The first whitespace-delimited token at or after `offset` that is
/// not a valid value of the format's width (u8 for macros, u16
/// otherwise), with its offset — the offending token of a wire parse
/// failure.
fn wire_offender(text: &str, offset: usize, format: WireFormat) -> Option<(usize, &str)> {
    let mut cursor = offset.min(text.len());

    while let Some((token_start, token)) = token_at(text, cursor) {
        let invalid = match format {
            WireFormat::Macros => token.parse::<u8>().is_err(),
            WireFormat::Keymap | WireFormat::Superkeys => token.parse::<u16>().is_err(),
        };

        if invalid {
            return Some((token_start, token));
        }

        cursor = token_start + token.len();
    }

    None
}

/// Cuts a window of `text` around (offset, len) so a huge single-line
/// source renders tight; returns the window and the span rebased into
/// it. The window — and the span within it — never leaves `text` or
/// splits a char.
fn excerpt(text: &str, offset: usize, len: usize) -> (String, (usize, usize)) {
    let offset = offset.min(text.len());
    let len = len.min(text.len() - offset);
    let mut begin = offset.saturating_sub(SNIPPET_RADIUS);

    while begin > 0 && !text.is_char_boundary(begin) {
        begin -= 1;
    }

    let mut end = (offset + len + SNIPPET_RADIUS).min(text.len());

    while end < text.len() && !text.is_char_boundary(end) {
        end += 1;
    }

    (text[begin..end].to_owned(), (offset - begin, len))
}

/// The span of the `"product": "…"` value in a backup JSON, when it
/// names `product`.
fn locate_product_value(text: &str, product: &str) -> Option<(usize, usize)> {
    let key = text.find("\"product\"")?;
    let value = json_string_value(text, key)?;

    (text[value.0..value.0 + value.1] == *product).then_some(value)
}

/// The span of the `data` value of the entry whose `"command"` is
/// `command`, in a backup JSON.
fn locate_data_value(text: &str, command: &str) -> Option<(usize, usize)> {
    let mut search = 0;

    while let Some(found) = text[search..].find("\"command\"") {
        let key = search + found;
        let value = json_string_value(text, key)?;

        if text[value.0..value.0 + value.1] == *command {
            let data_key = text[value.0 + value.1..].find("\"data\"")? + value.0 + value.1;

            return json_string_value(text, data_key);
        }

        search = value.0 + value.1;
    }

    None
}

/// The span of the backup JSON's top-level `partial` marker key — the
/// thing the refusal is about. A plain first-occurrence search: the
/// marker is a top-level key, and the only other place the exact
/// `"partial"` byte sequence can occur is inside an opaque store
/// string, where a misplaced highlight is cosmetic (the verdict never
/// depends on the span).
fn locate_partial_marker(text: &str) -> Option<(usize, usize)> {
    let key = text.find("\"partial\"")?;

    Some((key, "\"partial\"".len()))
}

/// The span (inside the quotes) of the JSON string value that follows
/// the object key at `key`.
fn json_string_value(text: &str, key: usize) -> Option<(usize, usize)> {
    let colon = text[key..].find(':')? + key;
    let opening = text[colon..].find('"')? + colon;
    let closing = text[opening + 1..].find('"')? + opening + 1;

    Some((opening + 1, closing - opening - 1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    /// Layer 1 of a real Defy keymap, captured from a device (the stock
    /// default keymap's first layer).
    const LAYER_ONE: &str = "41 30 31 32 33 34 35 0 0 51518 37 38 39 45 46 42 43 20 26 8 21 23 128 0 0 28 24 12 18 19 47 48 57 4 22 7 9 10 129 0 0 11 13 14 15 51 52 229 49 29 27 6 25 5 0 0 0 0 17 16 54 55 56 229 224 227 42 76 44 40 226 225 79 81 80 40 17152 74 82 77";

    /// A full `keymap.custom` wire string: the real layer repeated to
    /// the ten layers every Defy keymap holds.
    fn keymap_wire(layers: usize) -> String {
        vec![LAYER_ONE.to_owned(); layers].join(" ")
    }

    /// The ten-layer wire string with one token of layer 2 corrupted.
    fn corrupted_wire() -> String {
        let mut tokens: Vec<String> = std::iter::repeat_n(LAYER_ONE.to_owned(), 10)
            .flat_map(|layer| layer.split(' ').map(str::to_owned).collect::<Vec<_>>())
            .collect();

        tokens[80 + 20] = "O".to_owned();

        tokens.join(" ")
    }

    /// A real-shaped backup JSON: Bazecor's structure with a single
    /// `keymap.custom` entry holding `data`.
    fn backup_json(product: &str, name: &str, data: &str) -> String {
        format!(
            r#"{{
  "neuronID": "8ed7f33db024a5b5",
  "neuron": {{
    "id": "8ed7f33db024a5b5",
    "name": "{name}",
    "layers": [],
    "macros": [],
    "superkeys": [],
    "device": {{
      "info": {{
        "vendor": "Dygma",
        "product": "{product}",
        "keyboardType": "wireless",
        "displayName": "Dygma Defy wireless"
      }},
      "usb": {{
        "vendorId": 13807,
        "productId": 18
      }},
      "wireless": true,
      "chipId": "8ed7f33db024a5b5"
    }}
  }},
  "versions": {{
    "bazecor": "v1.1.0"
  }},
  "backup": [
    {{
      "command": "keymap.custom",
      "data": "{data}"
    }}
  ]
}}"#
        )
    }

    /// The NO_COLOR rendering of a report: the plain-text fallback.
    fn render_plain(report: &error_stack::Report<Error>) -> String {
        render_with_theme(report, GraphicalTheme::unicode_nocolor())
    }
    /// The exact no-color render for the wire failure (golden).
    const WIRE_GOLDEN: &str = r#"dygma-cli::keymap::wire-parse

  × invalid token `O` in keymap wire data (layer 2)
   ╭─[--keymap argument (excerpt):1:56]
 1 │  31 32 33 34 35 0 0 51518 37 38 39 45 46 42 43 20 26 8 O 23 128 0 0 28 24 12 18 19 47 48 57 4 22 7 9 10 129 0 0
   ·                                                        ┬
   ·                                                        ╰── not a number
   ╰────
  help: keymap wire data is space-separated u16 key codes — 80 per layer, 10 layers per
        keymap.custom write

Error: 
  × parsing the `--keymap` wire data

Error: 
  × failed to parse keymap: failed to parse keymap data at byte offset 238: expected a key code

Error: 
  × something went wrong running the command
"#;

    /// The exact no-color render for the json failure (golden).
    const JSON_GOLDEN: &str = r#"dygma-cli::json-parse

  × invalid JSON in keymap.json
   ╭─[keymap.json:6:9]
 5 │       "row_1": [
 6 │         "CTRl",
   ·         ───┬──
   ·            ╰── `CTRl` is not a parsable key
 7 │         "1",
   ╰────
  help: keys are human-readable names like `A` or `Left Ctrl` — check one with `dygma-cli key-code
        parse`

Error: 
  × parsing file contents

Error: 
  × keymap.json

Error: 
  × something went wrong running the command
"#;

    /// The exact no-color render for the raise failure (golden).
    const RAISE_GOLDEN: &str = r#"dygma-cli::backup::not-a-defy

  × the backup is for a `Raise`, not a Defy
    ╭─[backup.json:12:21]
 11 │         "vendor": "Dygma",
 12 │         "product": "Raise",
    ·                     ──┬──
    ·                       ╰── this backup was captured from a `Raise`
 13 │         "keyboardType": "wireless",
    ╰────
  help: the SDK speaks the Defy wire formats only — restoring another product's backup would write
        foreign data to the device

Error: 
  × validating the backup in backup.json

Error: 
  × something went wrong running the command
"#;

    /// The exact no-color render for the UNSUPPORTED_CONVERSION_GOLDEN (golden).
    const UNSUPPORTED_CONVERSION_GOLDEN: &str = "dygma-cli::backup-convert::unsupported\n\n  × there is no conversion from Raise to Defy\n    ╭─[backup.json:12:21]\n 11 │         \"vendor\": \"Dygma\",\n 12 │         \"product\": \"Raise\",\n    ·                     ──┬──\n    ·                       ╰── this backup was captured from a `Raise`\n 13 │         \"keyboardType\": \"wireless\",\n    ╰────\n  help: the supported conversions are Defy → Sonsei and Sonsei → Defy, so a Raise → Defy\n        conversion needs its own mapping work\n\nError: \n  × converting the backup in backup.json\n\nError: \n  × something went wrong running the command\n";

    /// The exact no-color render for the CONVERSION_SHAPE_GOLDEN (golden).
    const CONVERSION_SHAPE_GOLDEN: &str = "dygma-cli::backup-convert::invalid-entry\n\n  × the `keymap.custom` entry is not convertible: expected 10 layers of 60 values, the data holds\n  │ 800 values, not 600\n   ╭─[backup.json — `keymap.custom` `data` (excerpt):2:44]\n 1 │ \"backup\": [\n 2 │     { \"command\": \"keymap.custom\", \"data\": \"41 30 31 32 33 34 35 0 0 51518 37 38 39 45 46 42 43 20 \n   ·                                            ▲\n   ·                                            ╰── the data holds 800 values, not 600\n   ╰────\n  help: converting rewrites the entry into the target product's layer shape, so the entry must\n        hold the source product's shape exactly\n\nError: \n  × converting the backup in backup.json\n\nError: \n  × something went wrong running the command\n";

    /// The exact no-color render for the nine failure (golden).
    const NINE_GOLDEN: &str = r#"dygma-cli::backup::invalid-entry

  × the `keymap.custom` entry failed restore validation: keymap does not have exactly 10 layers
   ╭─[backup.json — `keymap.custom` `data` (excerpt):3:16]
 2 │       "command": "keymap.custom",
 3 │       "data": "41 30 31 32 33 34 35 0 0 51518 37 38 39 45 46 42 43 20 26
   ·                ─┬
   ·                 ╰── 720 codes = 9 layers; a `keymap.custom` write carries exactly 800 = 10
   ╰────
  help: backups hold whole device maps — re-capture with `dygma-cli backup create`

Error: 
  × validating the backup in backup.json

Error: 
  × something went wrong running the command
"#;

    /// The exact no-color render for the partial-capture refusal
    /// (golden): the span sits on the `partial` marker key, the
    /// message names the missing commands, and the help names both
    /// recovery paths (complete it, or `--allow-partial`).
    const PARTIAL_REFUSAL_GOLDEN: &str = r#"dygma-cli::backup::partial-capture

  × the backup is a partial capture: 2 command(s) missing: palette, colormap.map
    ╭─[backup.json:34:3]
 33 │ ,
 34 │   "partial": { "missing": ["palette", "colormap.map"] }
    ·   ────┬────
    ·       ╰── this marker says the file was saved from a failed capture
 35 │ }
    ╰────
  help: restoring it would write a half-captured configuration to the device — complete the
        capture first (`dygma-cli backup create --retry-missing <file>`), re-capture, or pass
        `--allow-partial` to restore what the file holds

Error: 
  × validating the backup in backup.json

Error: 
  × something went wrong running the command
"#;

    /// Renders a `--keymap` wire string with one corrupted token and
    /// pins the exact no-color output: the span must point at the bad
    /// token and the full input must never be quoted.
    #[test]
    fn wire_failure_renders_the_offending_token() {
        let wire = corrupted_wire();
        let err = wire.parse::<DefyKeymap>().unwrap_err();
        let report = wire_parse_report(WireFormat::Keymap, &wire, WireFailure::keymap(&err));
        let rendered = render_plain(&report);

        assert_eq!(rendered, WIRE_GOLDEN);

        // The 2,378-char input must not be dumped, in full or in its
        // error-chain doubling (the shape this unit replaces).
        assert!(!rendered.contains(&wire));
        assert!(rendered.len() < 1_000);
    }

    /// Renders a nine-layer wire string — valid keymap data that is not
    /// a Defy keymap — and pins the exact no-color output: the label
    /// must state the counts and the input must not be dumped.
    const NINE_WIRE_GOLDEN: &str = r#"dygma-cli::keymap::wire-parse

  × keymap does not have exactly 10 layers
   ╭─[--keymap argument (excerpt):1:1]
 1 │ 41 30 31 32 33 34 35 0 0 51518 37 38 39 45 46 42 43 20 26
   · ─┬
   ·  ╰── 720 codes = 9 layers; a `keymap.custom` write carries exactly 800 = 10
   ╰────
  help: keymap wire data is space-separated u16 key codes — 80 per layer, 10 layers per
        keymap.custom write

Error: 
  × parsing the `--keymap` wire data

Error: 
  × something went wrong running the command
"#;

    #[test]
    fn nine_layer_wire_renders_the_layer_counts() {
        let wire = keymap_wire(9);
        let err = wire.parse::<DefyKeymap>().unwrap_err();
        let report = wire_parse_report(WireFormat::Keymap, &wire, WireFailure::keymap(&err));
        let rendered = render_plain(&report);

        assert_eq!(rendered, NINE_WIRE_GOLDEN);
        assert!(!rendered.contains(&wire));
    }

    /// A keymap token that is numeric but beyond u16 gets a range
    /// label, not "not a number".
    #[test]
    fn keymap_wire_out_of_u16_gets_a_range_label() {
        let wire = keymap_wire(10).replacen(" 51518 ", " 65536 ", 1);
        let err = wire.parse::<DefyKeymap>().unwrap_err();
        let report = wire_parse_report(WireFormat::Keymap, &wire, WireFailure::keymap(&err));

        let rendered = render_plain(&report);

        assert!(rendered.contains("out of range for a u16"));
        assert!(rendered.contains("invalid token `65536` in keymap wire data"));
    }

    /// The real `macros.map` entry from Bazecor's Defy backup (one
    /// macro, 16 actions, then the terminators).
    const MACROS_WIRE: &str =
        "6 229 8 11 7 229 8 8 8 28 8 54 8 44 6 229 8 7 7 229 8 28 8 10 8 16 8 4 8 23 8 8 0 0";

    /// Renders macros wire data holding a value that is a valid u16 but
    /// not a u8 and pins the exact no-color output: macros are bytes,
    /// so the span must sit on the out-of-range token.
    #[test]
    fn macros_wire_out_of_u8_renders_the_token() {
        let wire = MACROS_WIRE.replacen(" 229 8 11", " 300 8 11", 1);
        let err = wire.parse::<MacroMap>().unwrap_err();
        let report = wire_parse_report(WireFormat::Macros, &wire, WireFailure::macros(&err));
        let rendered = render_plain(&report);

        assert_eq!(rendered, MACROS_U8_GOLDEN);
        assert!(!rendered.contains(&wire));
    }

    /// Renders macros wire data carrying an unsupported action kind and
    /// pins the exact no-color output: the parser consumes the kind
    /// before rejecting it, so the message is the library's own reason
    /// and the span marks where parsing stopped — no invented
    /// end-of-data story.
    #[test]
    fn macros_wire_unsupported_kind_renders_the_reason() {
        let wire = "12 14 7 18 18 26 0 0 ";
        let err = wire.parse::<MacroMap>().unwrap_err();
        let report = wire_parse_report(WireFormat::Macros, wire, WireFailure::macros(&err));
        let rendered = render_plain(&report);

        assert_eq!(rendered, MACROS_KIND_GOLDEN);
    }

    /// Renders compound macros wire data — an unsupported kind followed
    /// by a token that is also not a u8 — and pins the exact no-color
    /// output: the consumed-kind rejection must render the stop-and-
    /// reason form, never attributing the failure to the later token.
    #[test]
    fn macros_wire_compound_rejection_renders_the_stop() {
        let wire = "12 300 0 0";
        let err = wire.parse::<MacroMap>().unwrap_err();
        let report = wire_parse_report(WireFormat::Macros, wire, WireFailure::macros(&err));
        let rendered = render_plain(&report);

        assert_eq!(rendered, MACROS_COMPOUND_GOLDEN);
    }

    /// A consumed-kind rejection at the very end of the data keeps the
    /// library reason as the message — the failure is the consumed
    /// byte, not the end.
    #[test]
    fn macros_wire_consumed_at_eof_renders_the_reason() {
        let report = wire_parse_report(
            WireFormat::Macros,
            "12 ",
            WireFailure::macros(&"12 ".parse::<MacroMap>().unwrap_err()),
        );
        let rendered = render_plain(&report);

        assert!(rendered.contains("not yet supported"));
        assert!(!rendered.contains("ended where a value was expected"));
    }

    /// A `--keymap` string cut off mid-layer fails at the end of the
    /// input with nothing after the offset: the end case of the shared
    /// decision. The label marks the end and the message says the data
    /// ended where a value was expected; the label itself is not drawn
    /// for a zero-length span at the source's end (miette), so the
    /// snippet fields are pinned directly.
    #[test]
    fn wire_failure_at_the_end_says_the_data_ended() {
        let wire = "41 30";
        let err = wire.parse::<DefyKeymap>().unwrap_err();
        let report = wire_parse_report(WireFormat::Keymap, wire, WireFailure::keymap(&err));

        let snippet = report
            .frames()
            .find_map(|frame| frame.downcast_ref::<ErrorSnippet>())
            .unwrap();

        assert_eq!(snippet.span, (5, 0));
        assert_eq!(snippet.label, "the data ends here");
        assert!(
            render_plain(&report).contains("the keymap wire data ended where a value was expected")
        );
    }

    /// Mixed separator runs must not skew the layer hint: a widened
    /// gap before layer 2 with the failure at layer 2's last token
    /// still reports layer 2 (a space count would say 3).
    #[test]
    fn keymap_layer_hint_survives_mixed_separators() {
        let wire = keymap_wire(10);
        let mut tokens: Vec<&str> = wire.split_whitespace().collect();

        tokens[80 + 79] = "O";

        let data = format!(
            "{}   {} {}",
            tokens[..80].join(" "),
            tokens[80..160].join(" "),
            tokens[160..].join(" ")
        );

        let err = data.parse::<DefyKeymap>().unwrap_err();
        let report = wire_parse_report(WireFormat::Keymap, &data, WireFailure::keymap(&err));

        assert!(render_plain(&report).contains("(layer 2)"));
    }

    const MACROS_COMPOUND_GOLDEN: &str = r#"dygma-cli::macros::wire-parse

  × failed to parse macros map: failed to parse macro map data at byte offset 3: expected a
  │ supported action kind; kinds 9 to 13 are not yet supported
   ╭─[--macros argument (excerpt):1:4]
 1 │ 12 300 0 0
   ·    ─┬─
   ·     ╰── parsing stopped here
   ╰────
  help: macros wire data is space-separated u8 values — one byte per action, ending with a `0`
        terminator

Error: 
  × parsing the `--macros` wire data

Error: 
  × something went wrong running the command
"#;

    const MACROS_U8_GOLDEN: &str = r#"dygma-cli::macros::wire-parse

  × invalid token `300` in macros wire data
   ╭─[--macros argument (excerpt):1:3]
 1 │ 6 300 8 11 7 229 8 8 8 28 8 54 8 44 6 229 8 7 7 229 8 28 8 1
   ·   ─┬─
   ·    ╰── out of range for a u8
   ╰────
  help: macros wire data is space-separated u8 values — one byte per action, ending with a `0`
        terminator

Error: 
  × parsing the `--macros` wire data

Error: 
  × failed to parse macros map: failed to parse macro map data at byte offset 2: expected u8

Error: 
  × something went wrong running the command
"#;
    const MACROS_KIND_GOLDEN: &str = r#"dygma-cli::macros::wire-parse

  × failed to parse macros map: failed to parse macro map data at byte offset 3: expected a
  │ supported action kind; kinds 9 to 13 are not yet supported
   ╭─[--macros argument (excerpt):1:4]
 1 │ 12 14 7 18 18 26 0 0 
   ·    ─┬
   ·     ╰── parsing stopped here
   ╰────
  help: macros wire data is space-separated u8 values — one byte per action, ending with a `0`
        terminator

Error: 
  × parsing the `--macros` wire data

Error: 
  × something went wrong running the command
"#;

    /// Renders a keymap JSON file with one typo'd key name and pins the
    /// exact no-color output: the span must pin the offending token
    /// even though serde reports the position of the token after it.
    #[test]
    fn json_failure_renders_the_offending_key() {
        let good = serde_json::to_string_pretty(
            &DefyKeymap::from_str(&format!("{} ", keymap_wire(10))).unwrap(),
        )
        .unwrap();
        let bad = good.replacen("\"Escape\"", "\"CTRl\"", 1);
        let report = json_parse_report::<DefyKeymap>(
            Path::new("keymap.json"),
            bad.as_bytes(),
            serde_json::from_str::<DefyKeymap>(&bad).unwrap_err(),
        );

        assert_eq!(render_plain(&report), JSON_GOLDEN);
    }

    /// Renders a backup captured from another product and pins the
    /// exact no-color output: the span must sit on the product value
    /// the restore-plan validation rejected.
    #[test]
    fn wrong_product_backup_renders_the_product_value() {
        let text = backup_json("Raise", "Silver Raise", &keymap_wire(10));
        let backup: DefyBackup = serde_json::from_str(&text).unwrap();
        let report = backup_validation_report_with_text(
            Path::new("backup.json"),
            Some(&text),
            dygma_cli::devices::defy::DefyKeyboard::backup_restore_plan(&backup).unwrap_err(),
        );

        assert_eq!(render_plain(&report), RAISE_GOLDEN);
    }

    /// Renders a backup carrying the partial-capture marker and pins
    /// the exact no-color output: the span must sit on the marker key,
    /// the message must name the missing commands, and the help must
    /// name the completion and override paths.
    #[test]
    fn partial_capture_refusal_renders_the_marker() {
        let mut text = backup_json("Defy", "Silver Defy", &keymap_wire(10));

        // The interrupted-capture marker, as `backup create` writes it
        // before the closing brace.
        let marker = "  \"partial\": { \"missing\": [\"palette\", \"colormap.map\"] }\n";
        text.insert_str(text.len() - 1, &format!(",\n{marker}"));

        let backup: DefyBackup = serde_json::from_str::<DefyBackup>(&text).unwrap();
        let report = backup_validation_report_with_text(
            Path::new("backup.json"),
            Some(&text),
            dygma_cli::devices::defy::DefyKeyboard::backup_restore_plan(&backup).unwrap_err(),
        );

        let rendered = render_plain(&report);

        // The marker is located, the missing list is the message, and
        // the recovery paths are the help. Pinned exactly below.
        assert_eq!(rendered, PARTIAL_REFUSAL_GOLDEN);
    }

    /// The interrupted-capture report renders as the plain context
    /// chain (a device failure has no source text to span): the saved
    /// path, the captured/missing counts, the missing list, and the
    /// recovery paths, under the capture failure itself.
    #[test]
    fn interrupted_capture_report_carries_the_recovery_story() {
        use dygma_cli::devices::defy::CaptureFailure;
        use dygma_cli::focus_api::RunCommandError;

        let expected: Vec<String> = ["keymap.custom", "palette", "mouse.speed"]
            .iter()
            .map(|command| (*command).to_owned())
            .collect();
        let captured = ["keymap.custom", "palette"]
            .iter()
            .map(|command| {
                dygma_cli::focus_api::parsing::backup::BackupCommand::new(
                    (*command).to_owned(),
                    String::new(),
                )
            })
            .collect();

        let partial =
            dygma_cli::focus_api::parsing::backup::DefyBackup::partial_capture(captured, &expected);

        let report = interrupted_capture_report(
            Path::new("backup.json"),
            &partial,
            CaptureFailure::Command {
                command: "mouse.speed".to_owned(),
                source: RunCommandError::ResponseStreamTerminatedPrematurely,
            },
            "capturing the backup from the Defy".to_owned(),
        );

        let rendered = render_plain(&report);

        // The chain renders wrapped at the pinned width; the content
        // assertions normalize the wrapping (including miette's `│`
        // continuation markers) away — this report is the plain chain
        // (no snippet), so the words are the whole story. The source
        // renders like every source-chained error (the innermost
        // reason as the headline, the full Display at the chain's
        // end); the attachments carry the recovery story.
        let flat = rendered
            .split('│')
            .flat_map(|part| part.split_whitespace())
            .collect::<Vec<_>>()
            .join(" ");

        assert!(
            flat.contains("response stream terminated while waiting for the response to complete")
        );
        assert!(flat.contains(
            "failed to capture the `mouse.speed` response: response stream terminated \
             while waiting for the response to complete"
        ));

        // The caller-supplied context is the run's only context frame:
        // exactly once — a repeated frame would be a doubled headline,
        // and a second, different one would contradict the run.
        assert_eq!(
            flat.matches("capturing the backup from the Defy").count(),
            1
        );
        assert!(flat.contains(
            "a PARTIAL capture was saved to backup.json (2 command(s) captured, 1 missing)"
        ));
        assert!(flat.contains("missing: mouse.speed"));
        assert!(flat.contains("`backup restore` refuses this file unless --allow-partial"));
        assert!(flat.contains("backup create --retry-missing <file>"));
    }

    /// Renders a backup whose `keymap.custom` entry holds nine layers
    /// and pins the exact no-color output: the label must state the
    /// counts.
    #[test]
    fn nine_layer_backup_renders_the_layer_counts() {
        let text = backup_json("Defy", "Silver Defy", &keymap_wire(9));
        let backup: DefyBackup = serde_json::from_str(&text).unwrap();
        let report = backup_validation_report_with_text(
            Path::new("backup.json"),
            Some(&text),
            dygma_cli::devices::defy::DefyKeyboard::backup_restore_plan(&backup).unwrap_err(),
        );

        assert_eq!(render_plain(&report), NINE_GOLDEN);
    }

    /// Renders a Raise backup that `backup convert` cannot target and
    /// pins the exact no-color output: the span must sit on the
    /// product value, and the help must name the supported pairs.
    #[test]
    fn unsupported_conversion_renders_the_product_value() {
        let text = backup_json("Raise", "Silver Raise", &keymap_wire(10));
        let backup: DefyBackup = serde_json::from_str(&text).unwrap();
        let report = conversion_report_with_text(
            Path::new("backup.json"),
            Some(&text),
            dygma_cli::devices::conversions::convert_backup(
                &backup,
                dygma_cli::devices::conversions::Product::Defy,
            )
            .unwrap_err(),
        );

        assert_eq!(render_plain(&report), UNSUPPORTED_CONVERSION_GOLDEN);
    }

    /// Renders a backup claiming to be a Sonsei capture but carrying a
    /// Defy-sized keymap, and pins the exact no-color output: the
    /// label must state the found/expected counts.
    #[test]
    fn conversion_shape_failure_renders_the_counts() {
        // A complete Sonsei-shaped capture except the keymap, which
        // holds Defy-sized (800-token) data: the shape failure is the
        // story, so the colormap and palette must be convertible.
        let palette = std::iter::repeat_n(("1", "2", "3"), 16)
            .flat_map(|(r, g, b)| [r, g, b])
            .collect::<Vec<_>>()
            .join(" ");
        let colormap = std::iter::repeat_n("0", 560).collect::<Vec<_>>().join(" ");
        let text = format!(
            r#"{{
  "neuronID": "7f6a7facc35abff3",
  "neuron": {{
    "id": "7f6a7facc35abff3",
    "name": "Dygman's Sonsei",
    "layers": [],
    "macros": [],
    "superkeys": [],
    "device": {{ "info": {{ "vendor": "Dygma", "product": "Sonsei" }} }}
  }},
  "versions": {{ "bazecor": "v1.0.0-beta.2" }},
  "backup": [
    {{ "command": "keymap.custom", "data": "{}" }},
    {{ "command": "colormap.map", "data": "{}" }},
    {{ "command": "palette", "data": "{}" }}
  ]
}}"#,
            keymap_wire(10),
            colormap,
            palette
        );
        let backup: DefyBackup = serde_json::from_str(&text).unwrap();
        let report = conversion_report_with_text(
            Path::new("backup.json"),
            Some(&text),
            dygma_cli::devices::conversions::convert_backup(
                &backup,
                dygma_cli::devices::conversions::Product::Defy,
            )
            .unwrap_err(),
        );

        assert_eq!(render_plain(&report), CONVERSION_SHAPE_GOLDEN);
    }

    /// A backup's `macros.map` entry with a value that is a valid u16
    /// but not a u8: the entry span sits on the out-of-range token.
    #[test]
    fn backup_entry_out_of_u8_span_sits_on_the_token() {
        let data = MACROS_WIRE.replacen(" 229 8 11", " 300 8 11", 1);
        let source = BackupEntryError::Macros(ParseMacroMapError::MalformedData(
            dygma_cli::focus_api::parsing::macros::ParseMacroMapError {
                offset: 2,
                expected: "u8".to_owned(),
                unsupported_kind: false,
            },
        ));

        let (span, label) = entry_span_and_label(&data, 100, &source);

        assert_eq!(span, (102, 3));
        assert_eq!(label, "not valid wire data: `300`");
    }

    /// A backup's `macros.map` entry carrying an unsupported action
    /// kind: the parser consumes the kind before rejecting it, so the
    /// span marks where parsing stopped instead of inventing an
    /// end-of-data story.
    #[test]
    fn backup_entry_unsupported_kind_marks_the_stop() {
        let data = "12 14 7 18 18 26 0 0";
        let source = BackupEntryError::Macros(ParseMacroMapError::MalformedData(
            dygma_cli::focus_api::parsing::macros::ParseMacroMapError {
                offset: 3,
                expected: "a supported action kind; kinds 9 to 13 are not yet supported".to_owned(),
                unsupported_kind: true,
            },
        ));

        let (span, label) = entry_span_and_label(data, 100, &source);

        assert_eq!(span, (103, 2));
        assert_eq!(label, "parsing stopped here");
    }

    /// A compound backup `macros.map` entry — an unsupported kind
    /// followed by a token that is also not a u8 — must mark the stop,
    /// never attributing the failure to the later token.
    #[test]
    fn backup_entry_compound_rejection_marks_the_stop() {
        let data = "12 300 0 0";
        let source = BackupEntryError::Macros(ParseMacroMapError::MalformedData(
            dygma_cli::focus_api::parsing::macros::ParseMacroMapError {
                offset: 3,
                expected: "a supported action kind; kinds 9 to 13 are not yet supported".to_owned(),
                unsupported_kind: true,
            },
        ));

        let (span, label) = entry_span_and_label(data, 100, &source);

        assert_eq!(span, (103, 3));
        assert_eq!(label, "parsing stopped here");
    }

    /// A backup entry whose parse failure sits at the end of the data
    /// (nothing follows the offset, and no token is the offender): the
    /// span sits at the entry's start and the label says the data
    /// ended where a value was expected — the end case of the shared
    /// decision, reached here without a consumed offender.
    #[test]
    fn backup_entry_failure_at_the_end_says_the_data_ended() {
        let source = BackupEntryError::Macros(ParseMacroMapError::MalformedData(
            dygma_cli::focus_api::parsing::macros::ParseMacroMapError {
                offset: 10,
                expected: "u8".to_owned(),
                unsupported_kind: false,
            },
        ));

        let (span, label) = entry_span_and_label("12 300 0 0", 100, &source);

        assert_eq!(span, (100, 0));
        assert_eq!(label, "the data ended where a value was expected");
    }

    /// serde's column points at the offending character for syntax
    /// errors, so the fallback span sits on that character — pinned for
    /// a trailing comma: serde reports column 7 (1-based), the `]` at
    /// byte 6 of `[1, 2,]`.
    #[test]
    fn json_syntax_failure_span_sits_on_the_offending_char() {
        let err = serde_json::from_str::<Vec<u8>>("[1, 2,]").unwrap_err();

        assert!(matches!(
            err.classify(),
            serde_json::error::Category::Syntax
        ));

        let snippet =
            json_snippet::<DefyBackup>(Path::new("keymap.json"), "[1, 2,]", &err, Vec::new());

        assert_eq!(snippet.span, (6, 1));
        assert_eq!(snippet.label, "trailing comma");
    }

    /// A peek-time type mismatch (a Data error) reports the column
    /// just past the consumed value, so the fallback span backs up one
    /// byte — onto the value's last character for multi-character
    /// values, else the preceding byte. Pinned for an integer where
    /// the backup wants a string.
    #[test]
    fn json_data_failure_span_backs_onto_the_value() {
        let text = r#"{"neuron": {}, "versions": {"bazecor": 5}}"#;
        let err = serde_json::from_str::<DefyBackup>(text).unwrap_err();

        assert!(matches!(err.classify(), serde_json::error::Category::Data));

        let snippet = json_snippet::<DefyBackup>(Path::new("backup.json"), text, &err, Vec::new());

        // The `5` sits at byte 39; the span backs onto byte 38.
        assert_eq!(snippet.span, (38, 1));
    }

    /// `NO_COLOR` follows the no-color.org convention: a non-empty
    /// value selects the plain rendering, an empty value does not, and
    /// a non-terminal stderr is always plain.
    #[test]
    fn no_color_convention() {
        let report = Error.into_report().attach("anything");

        let colored = |theme| render_with_theme(&report, theme).contains("\x1b[");

        assert!(colored(theme_for(None, true)));
        assert!(colored(theme_for(Some(std::ffi::OsStr::new("")), true)));
        assert!(!colored(theme_for(Some(std::ffi::OsStr::new("1")), true)));
        assert!(!colored(theme_for(None, false)));
    }

    /// The no-color fallback for an error without a located snippet:
    /// the error-stack chain stays readable.
    #[test]
    fn unlocated_error_renders_the_context_chain() {
        let report = Error
            .into_report()
            .attach("no names stored for Neuron `8ed7f33db024a5b5`");

        assert_eq!(
            render_plain(&report),
            r"dygma-cli::error

  × no names stored for Neuron `8ed7f33db024a5b5`

Error: 
  × something went wrong running the command
"
        );
    }

    /// The colored theme produces ANSI escapes on the same structure —
    /// the graphical render survives without terminal probing (the
    /// `fancy-no-syscall` trim decision).
    #[test]
    fn colored_render_carries_ansi_escapes() {
        let wire = corrupted_wire();
        let err = wire.parse::<DefyKeymap>().unwrap_err();
        let report = wire_parse_report(WireFormat::Keymap, &wire, WireFailure::keymap(&err));
        let rendered = render_with_theme(&report, GraphicalTheme::unicode());

        assert!(rendered.contains("\x1b["));
        assert!(rendered.contains("invalid token `O` in keymap wire data (layer 2)"));
    }
}
