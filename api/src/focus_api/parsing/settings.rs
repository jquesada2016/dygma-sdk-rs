//! Types for parsing and serializing the identity and settings Focus
//! commands (`version`, `hardware.*`, `keymap.onlyCustom`,
//! `settings.defaultLayer`, `qukeys.*`, `superkeys.*` timings, `mouse.*`,
//! `macros.memory`, `eeprom.contents`, and the LED settings `led.fade`
//! and `idleleds.true_sleep`).
//!
//! These commands all carry trivial single values, but each has its own
//! wire shape: decimal numbers of different widths, `true`/`false` words,
//! the `"1"`/`"0"` of `keymap.onlyCustom`, the `126` "no default"
//! sentinel of `settings.defaultLayer`, and — for `qukeys.holdTimeout`,
//! `mouse.speedDelay`, `mouse.accelDelay`, and `mouse.wheelDelay` — a
//! two-number low-byte-then-high-byte pair on writes.

use std::str::FromStr;

use itertools::Itertools;
use winnow::{
    ModalResult, Parser,
    ascii::{dec_uint, space0},
    combinator::{eof, repeat, terminated},
};

/// The largest layer number a [`DefaultLayer::layer`] can carry.
///
/// The firmware stores the default layer in a 7-bit field where `0x7e`
/// (126) means "no default layer", so layers 126 and above are not
/// representable (`kaleidoscope/plugin/EEPROM-Settings.h`,
/// `IGNORE_HARDCODED_LAYER`).
pub const DEFAULT_LAYER_MAX: u8 = 125;

/// The wire value the firmware uses to signal "no default layer"
/// (`kaleidoscope/plugin/EEPROM-Settings.h`, `IGNORE_HARDCODED_LAYER`).
pub const DEFAULT_LAYER_NONE_SENTINEL: u8 = 126;

/// Error returned when an identity or settings response does not match
/// the wire shape the firmware serves for it.
#[derive(Clone, Debug, Display, Error)]
pub enum ParseSettingsError {
    /// A decimal number response was expected, but the data did not
    /// parse as one.
    #[display("expected a decimal number, got: {_0}")]
    Decimal(#[error(not(source))] String),
    /// A `version` response was expected (a space-separated
    /// `bazecor kaleidoscope firmware` triple), but the data did not
    /// have three tokens.
    #[display("expected a `bazecor kaleidoscope firmware` version triple, got: {_0}")]
    Version(#[error(not(source))] String),
    /// A `hardware.chip_id` response was expected, but the data was
    /// empty.
    #[display("expected a chip id, got an empty response")]
    ChipId,
    /// A `hardware.wireless` response was expected (`true` or `false`),
    /// but the data was something else.
    #[display("expected `true` or `false`, got: {_0}")]
    Wireless(#[error(not(source))] String),
    /// A `hardware.layout` response was expected (`ANSI` or `ISO`), but
    /// the data was something else.
    #[display("expected `ANSI` or `ISO`, got: {_0}")]
    Layout(#[error(not(source))] String),
    /// A `keymap.onlyCustom` response carried a number other than the
    /// `0`/`1` the firmware serves this setting as.
    #[display("expected `1` or `0` on the wire, got: {_0}")]
    OnlyCustom(#[error(not(source))] String),
    /// An `eeprom.contents` response was expected, but the data did not
    /// parse as a stream of byte-sized decimal numbers.
    #[display("expected a stream of decimal bytes, got: {_0}")]
    EepromContents(#[error(not(source))] String),
    /// An on/off setting (`led.fade`, `idleleds.true_sleep`) response
    /// was expected (a decimal `1` or `0`), but the data was something
    /// else.
    #[display("expected `1` or `0`, got: {_0}")]
    OnOff(#[error(not(source))] String),
}

/// The firmware version triple reported by the `version` command.
///
/// The device answers with three space-separated tokens, which Bazecor
/// reads as the Bazecor, Kaleidoscope, and firmware versions
/// respectively (for example
/// `v2.2.0 7622bb53 c9d9b7b-dirty`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FirmwareVersion {
    /// The Bazecor-side firmware version.
    pub bazecor: String,
    /// The Kaleidoscope version the firmware is built on.
    pub kaleidoscope: String,
    /// The firmware version itself.
    pub firmware: String,
}

impl FromStr for FirmwareVersion {
    type Err = ParseSettingsError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut tokens = s.split_whitespace();

        match (tokens.next(), tokens.next(), tokens.next(), tokens.next()) {
            (Some(bazecor), Some(kaleidoscope), Some(firmware), None) => Ok(Self {
                bazecor: bazecor.to_owned(),
                kaleidoscope: kaleidoscope.to_owned(),
                firmware: firmware.to_owned(),
            }),
            _ => Err(ParseSettingsError::Version(s.to_owned())),
        }
    }
}

impl std::fmt::Display for FirmwareVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            bazecor,
            kaleidoscope,
            firmware,
        } = self;

        write!(f, "{bazecor} {kaleidoscope} {firmware}")
    }
}

/// The Neuron identity string reported by the `hardware.chip_id`
/// command.
///
/// Bazecor keys its app-side per-device stores (layer, superkey, and
/// macro names) by this value.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ChipId(String);

impl FromStr for ChipId {
    type Err = ParseSettingsError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let id = s.trim();

        if id.is_empty() {
            return Err(ParseSettingsError::ChipId);
        }

        Ok(Self(id.to_owned()))
    }
}

impl std::fmt::Display for ChipId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl ChipId {
    /// Borrows the inner chip id string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A `true`/`false` word response, as printed by the firmware's
/// `Focus.send(bool)` (`kaleidoscope/plugin/FocusSerial.cpp`,
/// `printBool`).
///
/// Used by `hardware.wireless`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoolWord(pub bool);

impl FromStr for BoolWord {
    type Err = ParseSettingsError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim() {
            "true" => Ok(Self(true)),
            "false" => Ok(Self(false)),
            other => Err(ParseSettingsError::Wireless(other.to_owned())),
        }
    }
}

impl std::fmt::Display for BoolWord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.0 { "true" } else { "false" })
    }
}

/// The physical key layout reported by the `hardware.layout` command.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyboardLayout {
    /// The ANSI (US) layout variant.
    Ansi,
    /// The ISO (EU) layout variant.
    Iso,
}

impl FromStr for KeyboardLayout {
    type Err = ParseSettingsError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim() {
            "ANSI" => Ok(Self::Ansi),
            "ISO" => Ok(Self::Iso),
            other => Err(ParseSettingsError::Layout(other.to_owned())),
        }
    }
}

impl std::fmt::Display for KeyboardLayout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Ansi => "ANSI",
            Self::Iso => "ISO",
        })
    }
}

/// A single unsigned decimal number response, as printed by the
/// firmware's `Focus.send` for a `uint8_t` value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecimalU8(pub u8);

impl FromStr for DecimalU8 {
    type Err = ParseSettingsError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        u8_decimal_parser
            .parse(s)
            .map(Self)
            .map_err(|_| ParseSettingsError::Decimal(s.to_owned()))
    }
}

/// A single unsigned decimal number response, as printed by the
/// firmware's `Focus.send` for a `uint16_t` value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecimalU16(pub u16);

impl FromStr for DecimalU16 {
    type Err = ParseSettingsError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        u16_decimal_parser
            .parse(s)
            .map(Self)
            .map_err(|_| ParseSettingsError::Decimal(s.to_owned()))
    }
}

/// Parses a single decimal number optionally followed by whitespace,
/// and nothing else. The firmware terminates every value it sends with
/// a space separator, so a trailing space is the norm.
///
/// Parsing directly into the target width is what makes overflowing
/// values (a `300` in a `u8` response) fail instead of truncate.
fn u8_decimal_parser(input: &mut &str) -> ModalResult<u8> {
    terminated(dec_uint::<_, u8, _>, space0).parse_next(input)
}

/// The `u16` flavor of [`u8_decimal_parser`].
fn u16_decimal_parser(input: &mut &str) -> ModalResult<u16> {
    terminated(dec_uint::<_, u16, _>, space0).parse_next(input)
}

/// The `keymap.onlyCustom` setting: whether the keyboard ignores its
/// hardcoded default layers and only uses the layers stored in
/// `keymap.custom`.
///
/// On the wire this is `"1"` or `"0"`, not `true`/`false`
/// (`kaleidoscope/plugin/EEPROM-Keymap.cpp`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OnlyCustom(pub bool);

impl FromStr for OnlyCustom {
    type Err = ParseSettingsError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let DecimalU8(value) = DecimalU8::from_str(s)?;

        // The firmware serves this setting as a `0`/`1` bool; anything
        // else on the wire is not a representable `OnlyCustom`.
        match value {
            0 => Ok(Self(false)),
            1 => Ok(Self(true)),
            _ => Err(ParseSettingsError::OnlyCustom(s.to_owned())),
        }
    }
}

/// Returns the command data for writing `keymap.onlyCustom`: `"1"` or
/// `"0"`.
pub fn only_custom_command_data(value: bool) -> &'static str {
    if value { "1" } else { "0" }
}

/// An on/off setting the firmware serves and accepts as the decimals
/// `"1"` and `"0"`: `led.fade` and `idleleds.true_sleep`.
///
/// The first decimal token is the value; anything after the first
/// separator is ignored, exactly like Bazecor's `parseInt` read of the
/// same responses. That tolerance is not incidental: the only real
/// `led.fade` response ever captured from a Dygma LED-side firmware
/// (the shipped Sonsei backup's `led.fade` entry) reads `"1 0"`, two
/// tokens — a strict single-value parse would refuse data the device
/// really serves. Reads of single-value settings with no such capture
/// ([`DecimalU8`]/[`DecimalU16`]) stay strict.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OnOff(pub bool);

impl FromStr for OnOff {
    type Err = ParseSettingsError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Leading whitespace is a separator run like any other; the
        // first token must be a complete decimal 0/1 (a partial token
        // like `10` is a different value, not a tolerated form).
        let first = s.split_whitespace().next().unwrap_or_default();

        match first {
            "1" => Ok(Self(true)),
            "0" => Ok(Self(false)),
            _ => Err(ParseSettingsError::OnOff(s.to_owned())),
        }
    }
}

impl std::fmt::Display for OnOff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.0 { "1" } else { "0" })
    }
}

/// Returns the command data for writing an [`OnOff`] setting: `"1"` or
/// `"0"`.
pub fn on_off_command_data(value: bool) -> &'static str {
    if value { "1" } else { "0" }
}

/// The `settings.defaultLayer` setting.
///
/// The firmware stores the default layer in a 7-bit field and treats
/// `126` (`0x7e`, `IGNORE_HARDCODED_LAYER`) as "no default layer";
/// writes of `126` or above are stored as the sentinel
/// (`kaleidoscope/plugin/EEPROM-Settings.cpp`). Reads of the sentinel,
/// and of the uninitialized EEPROM's `127`, are reported here as
/// [`DefaultLayer::none`] — mirroring the firmware, where any value
/// from the sentinel upwards means no default layer is set.
///
/// The inner value is private so a layer number above
/// [`DEFAULT_LAYER_MAX`] is unrepresentable; construct values through
/// [`DefaultLayer::layer`] or [`FromStr`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DefaultLayer(Option<u8>);

impl DefaultLayer {
    /// Creates a default layer that boots into `number`, returning
    /// `None` when the number exceeds [`DEFAULT_LAYER_MAX`] and thus
    /// cannot be represented on the wire.
    pub fn layer(number: u8) -> Option<Self> {
        if number > DEFAULT_LAYER_MAX {
            return None;
        }

        Some(Self(Some(number)))
    }

    /// Creates the "no default layer" setting.
    pub const fn none() -> Self {
        Self(None)
    }

    /// The layer number, or `None` for the "no default layer" setting.
    pub const fn layer_number(self) -> Option<u8> {
        self.0
    }

    /// Converts the setting into a form suitable for sending to the
    /// keyboard as the data of a `settings.defaultLayer` command.
    pub fn to_command_data(self) -> String {
        match self.0 {
            Some(number) => number.to_string(),
            None => DEFAULT_LAYER_NONE_SENTINEL.to_string(),
        }
    }
}

impl FromStr for DefaultLayer {
    type Err = ParseSettingsError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let DecimalU8(number) = s.parse::<DecimalU8>()?;

        // Values from the sentinel upwards mean "no default": the
        // firmware stores writes of 126+ as the 126 sentinel, and an
        // uninitialized EEPROM reads back 127.
        if number >= DEFAULT_LAYER_NONE_SENTINEL {
            return Ok(Self::none());
        }

        Ok(Self(Some(number)))
    }
}

impl std::fmt::Display for DefaultLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Some(number) => write!(f, "{number}"),
            None => f.write_str("none"),
        }
    }
}

/// Returns the command data for writing a `uint16_t` setting whose
/// firmware write path reads a low-byte-then-high-byte decimal pair
/// and composes the value as `(high << 8) | low`
/// (`kaleidoscope/plugin/Qukeys.cpp` `holdTimeout`,
/// `kaleidoscope/plugin/MouseKeys.cpp` `speedDelay`/`accelDelay`/
/// `wheelDelay`).
///
/// The firmware never receives a value larger than 255 from a single
/// decimal write (it parses into a `uint8_t`), so sending both numbers
/// is the only way to address the full `u16` range.
pub fn split_u16_command_data(value: u16) -> String {
    format!("{} {}", value & 0xFF, value >> 8)
}

/// The `macros.memory` value reported by the device, guaranteed to be
/// at least [`MacroMemorySize::MIN`].
///
/// Older firmwares do not serve `macros.memory` at all, and the
/// firmware sizes its macro storage per build, so the value must be
/// probed per device. Following Bazecor, a missing or suspiciously
/// small probe falls back to [`MacroMemorySize::FALLBACK`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MacroMemorySize(u16);

impl MacroMemorySize {
    /// The smallest value that is trusted as a real probe result.
    /// Anything below this is treated as missing
    /// (`bazecor/src/renderer/views/MacroEditor.tsx`).
    pub const MIN: u16 = 100;

    /// The fallback used when the probe is missing or below
    /// [`MacroMemorySize::MIN`]. This is also what `FOCUS_API.md`
    /// documents as the stock macro memory.
    pub const FALLBACK: u16 = 2048;

    /// Creates a size, returning `None` when the value is below
    /// [`MacroMemorySize::MIN`].
    pub fn new(value: u16) -> Option<Self> {
        if value < Self::MIN {
            return None;
        }

        Some(Self(value))
    }

    /// Sanitizes a probe result, falling back to
    /// [`MacroMemorySize::FALLBACK`] when the probe is missing
    /// (`None`, e.g. the command is not served by the firmware) or
    /// below [`MacroMemorySize::MIN`].
    pub fn sanitize(value: Option<u16>) -> Self {
        value.and_then(Self::new).unwrap_or(Self(Self::FALLBACK))
    }

    /// The size in bytes.
    pub fn get(self) -> u16 {
        self.0
    }
}

impl From<MacroMemorySize> for usize {
    fn from(value: MacroMemorySize) -> Self {
        usize::from(value.0)
    }
}

impl std::fmt::Display for MacroMemorySize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The contents of the device's EEPROM, as reported by the
/// `eeprom.contents` command: one decimal number per stored byte.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EepromContents(pub Vec<u8>);

impl FromStr for EepromContents {
    type Err = ParseSettingsError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let contents = eeprom_contents_parser
            .parse(s)
            .map_err(|_| ParseSettingsError::EepromContents(s.to_owned()))?;

        Ok(Self(contents))
    }
}

/// Error returned when an erase payload cannot be built for
/// [`EepromContents::erase_command_data`].
#[derive(Clone, Copy, Debug, Display, Error)]
#[display(
    "the device reported an empty EEPROM, so there is nothing to erase; \
    writing an empty payload would be treated as a read by the firmware"
)]
pub struct EmptyEepromError;

impl EepromContents {
    /// The number of bytes the device stores.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether the device stores no bytes at all.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Converts the contents into an erase payload for the
    /// `eeprom.contents` command: the same number of bytes the device
    /// reported, all set to `255` (the erased state). Sending this
    /// back wipes every plugin setting stored in the EEPROM.
    ///
    /// Fails with [`EmptyEepromError`] when the device reported no
    /// bytes at all: there would be nothing to erase, and the empty
    /// write would hit the firmware's `isEOL` and be answered as a
    /// read instead, silently erasing nothing.
    pub fn erase_command_data(&self) -> Result<String, EmptyEepromError> {
        if self.0.is_empty() {
            return Err(EmptyEepromError);
        }

        Ok(self.0.iter().map(|_| "255").join(" "))
    }
}

fn eeprom_contents_parser(input: &mut &str) -> ModalResult<Vec<u8>> {
    let contents = repeat(0.., terminated(dec_uint::<_, u8, _>, space0)).parse_next(input)?;
    let _ = eof.parse_next(input)?;

    Ok(contents)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_parses_real_device_response() {
        // The real `version` response of Bazecor's virtual wireless
        // Defy (`src/api/hardware-virtual/DefyWireless.ts`), with the
        // trailing separator space the firmware appends.
        let version = "v2.2.0 7622bb53 c9d9b7b-dirty "
            .parse::<FirmwareVersion>()
            .unwrap();

        assert_eq!(version.bazecor, "v2.2.0");
        assert_eq!(version.kaleidoscope, "7622bb53");
        assert_eq!(version.firmware, "c9d9b7b-dirty");

        // Without a trailing space also parses (device responses may
        // or may not carry the separator before the terminator).
        let version = "v2.2.0 7622bb53 c9d9b7b-dirty"
            .parse::<FirmwareVersion>()
            .unwrap();

        assert_eq!(version.bazecor, "v2.2.0");
    }

    #[test]
    fn version_display_round_trips() {
        let version = "v2.2.0 7622bb53 c9d9b7b-dirty"
            .parse::<FirmwareVersion>()
            .unwrap();

        assert_eq!(version.to_string(), "v2.2.0 7622bb53 c9d9b7b-dirty");
    }

    #[test]
    fn version_fails_on_wrong_token_count() {
        assert!(matches!(
            "v2.2.0 7622bb53".parse::<FirmwareVersion>(),
            Err(ParseSettingsError::Version(_))
        ));

        assert!(matches!(
            "v2.2.0 7622bb53 c9d9b7b extra".parse::<FirmwareVersion>(),
            Err(ParseSettingsError::Version(_))
        ));

        assert!(matches!(
            "".parse::<FirmwareVersion>(),
            Err(ParseSettingsError::Version(_))
        ));
    }

    #[test]
    fn chip_id_parses_and_trims() {
        // The real response of Bazecor's virtual wireless Defy.
        let id = "VirtualDefyWirelessVirtualDefyWireless "
            .parse::<ChipId>()
            .unwrap();

        assert_eq!(id.as_str(), "VirtualDefyWirelessVirtualDefyWireless");
        assert_eq!(id.to_string(), "VirtualDefyWirelessVirtualDefyWireless");

        // Bazecor's serial unit test fixture.
        assert_eq!("chip123".parse::<ChipId>().unwrap().as_str(), "chip123");

        assert!(matches!(
            " ".parse::<ChipId>(),
            Err(ParseSettingsError::ChipId)
        ));
        assert!(matches!(
            "".parse::<ChipId>(),
            Err(ParseSettingsError::ChipId)
        ));
    }

    #[test]
    fn bool_word_parses() {
        assert_eq!("true ".parse::<BoolWord>().unwrap(), BoolWord(true));
        assert_eq!("false".parse::<BoolWord>().unwrap(), BoolWord(false));
        assert_eq!(BoolWord(true).to_string(), "true");

        assert!(matches!(
            "1 ".parse::<BoolWord>(),
            Err(ParseSettingsError::Wireless(_))
        ));
    }

    #[test]
    fn keyboard_layout_parses() {
        // The real responses of Bazecor's virtual Defies.
        assert_eq!(
            "ISO ".parse::<KeyboardLayout>().unwrap(),
            KeyboardLayout::Iso
        );
        assert_eq!(
            "ANSI ".parse::<KeyboardLayout>().unwrap(),
            KeyboardLayout::Ansi
        );
        assert_eq!(KeyboardLayout::Iso.to_string(), "ISO");

        assert!(matches!(
            "ansi".parse::<KeyboardLayout>(),
            Err(ParseSettingsError::Layout(_))
        ));
    }

    #[test]
    fn decimals_parse_device_responses() {
        // Real-shaped responses: value + separator space, or bare value.
        assert_eq!("250 ".parse::<DecimalU8>().unwrap(), DecimalU8(250));
        assert_eq!("127".parse::<DecimalU8>().unwrap(), DecimalU8(127));
        assert_eq!("2048 ".parse::<DecimalU16>().unwrap(), DecimalU16(2048));

        // Overflowing the width fails.
        assert!(matches!(
            "300 ".parse::<DecimalU8>(),
            Err(ParseSettingsError::Decimal(_))
        ));

        // Trailing garbage fails.
        assert!(matches!(
            "250 1 ".parse::<DecimalU8>(),
            Err(ParseSettingsError::Decimal(_))
        ));
        assert!(matches!(
            "garbage".parse::<DecimalU8>(),
            Err(ParseSettingsError::Decimal(_))
        ));
        assert!(matches!(
            "-1".parse::<DecimalU8>(),
            Err(ParseSettingsError::Decimal(_))
        ));
    }

    #[test]
    fn only_custom_parses_one_and_zero() {
        // The wire is "1"/"0", not true/false. Real response of the
        // virtual Defy: `1 `.
        assert_eq!("1 ".parse::<OnlyCustom>().unwrap(), OnlyCustom(true));
        assert_eq!("0".parse::<OnlyCustom>().unwrap(), OnlyCustom(false));

        // A bool cannot represent other wire values, so they fail with
        // the dedicated variant: the input IS a decimal, it just isn't
        // a `0`/`1`.
        assert!(matches!(
            "2 ".parse::<OnlyCustom>(),
            Err(ParseSettingsError::OnlyCustom(_))
        ));
        assert!(matches!(
            "255".parse::<OnlyCustom>(),
            Err(ParseSettingsError::OnlyCustom(_))
        ));

        assert_eq!(only_custom_command_data(true), "1");
        assert_eq!(only_custom_command_data(false), "0");
    }

    #[test]
    fn default_layer_parses_sentinels_as_none() {
        assert_eq!(
            "0 ".parse::<DefaultLayer>().unwrap(),
            DefaultLayer::layer(0).unwrap()
        );
        assert_eq!(
            "5 ".parse::<DefaultLayer>().unwrap(),
            DefaultLayer::layer(5).unwrap()
        );
        // 126 is the "no default" sentinel; 127 is the uninitialized
        // EEPROM's read. The firmware clamps every write of 126+ to
        // the sentinel, so both mean none.
        assert_eq!(
            "126 ".parse::<DefaultLayer>().unwrap(),
            DefaultLayer::none()
        );
        assert_eq!("127".parse::<DefaultLayer>().unwrap(), DefaultLayer::none());
        assert_eq!("255".parse::<DefaultLayer>().unwrap(), DefaultLayer::none());

        assert!(matches!(
            "300".parse::<DefaultLayer>(),
            Err(ParseSettingsError::Decimal(_))
        ));
    }

    #[test]
    fn default_layer_serializes() {
        assert_eq!(DefaultLayer::layer(5).unwrap().to_command_data(), "5");
        assert_eq!(DefaultLayer::none().to_command_data(), "126");

        // Layer numbers above the maximum are unrepresentable.
        assert!(DefaultLayer::layer(DEFAULT_LAYER_MAX).is_some());
        assert!(DefaultLayer::layer(126).is_none());
        assert!(DefaultLayer::layer(u8::MAX).is_none());

        // The human display is not the wire form.
        assert_eq!(DefaultLayer::none().to_string(), "none");
        assert_eq!(DefaultLayer::layer(9).unwrap().to_string(), "9");

        // Parsing the wire form round-trips.
        for layer in [
            DefaultLayer::layer(0).unwrap(),
            DefaultLayer::layer(125).unwrap(),
            DefaultLayer::none(),
        ] {
            assert_eq!(
                layer.to_command_data().parse::<DefaultLayer>().unwrap(),
                layer
            );
        }
    }

    #[test]
    fn split_u16_serializes_low_byte_first() {
        // The firmware composes the value as (high << 8) | low from a
        // low-byte-then-high-byte decimal pair.
        assert_eq!(split_u16_command_data(1000), "232 3");
        assert_eq!(split_u16_command_data(0), "0 0");
        assert_eq!(split_u16_command_data(255), "255 0");
        assert_eq!(split_u16_command_data(65_535), "255 255");
    }

    #[test]
    fn on_off_reads_the_first_decimal_token() {
        // The real `led.fade` response of the shipped Sonsei backup:
        // TWO tokens. Bazecor's `parseInt` reads the first; so does
        // this type, or that real capture could not be read.
        assert_eq!("1 0".parse::<OnOff>().unwrap(), OnOff(true));
        assert_eq!("0 ".parse::<OnOff>().unwrap(), OnOff(false));
        assert_eq!("1".parse::<OnOff>().unwrap(), OnOff(true));

        // A partial token is a different value, not a tolerated form.
        assert!(matches!(
            "10".parse::<OnOff>(),
            Err(ParseSettingsError::OnOff(_))
        ));
        assert!(matches!(
            "true".parse::<OnOff>(),
            Err(ParseSettingsError::OnOff(_))
        ));
        assert!(matches!(
            "".parse::<OnOff>(),
            Err(ParseSettingsError::OnOff(_))
        ));

        // The wire form and display agree: 1/0.
        assert_eq!(OnOff(true).to_string(), "1");
        assert_eq!(on_off_command_data(false), "0");
    }

    #[test]
    fn macro_memory_size_sanitizes_like_bazecor() {
        // A healthy probe passes through.
        assert_eq!(
            MacroMemorySize::sanitize(Some(2048)).get(),
            MacroMemorySize::FALLBACK
        );
        assert_eq!(MacroMemorySize::sanitize(Some(1024)).get(), 1024);
        // Exactly the minimum is trusted.
        assert_eq!(MacroMemorySize::sanitize(Some(100)).get(), 100);

        // Below the minimum, or a missing probe (older firmware that
        // does not serve `macros.memory`), falls back to 2048.
        assert_eq!(MacroMemorySize::sanitize(Some(99)).get(), 2048);
        assert_eq!(MacroMemorySize::sanitize(Some(0)).get(), 2048);
        assert_eq!(MacroMemorySize::sanitize(None).get(), 2048);

        assert!(MacroMemorySize::new(99).is_none());
        assert_eq!(usize::from(MacroMemorySize::sanitize(Some(512))), 512);
    }

    #[test]
    fn eeprom_contents_parses_and_erases() {
        let contents = "255 0 32 ".parse::<EepromContents>().unwrap();

        assert_eq!(contents.0, vec![255, 0, 32]);
        assert_eq!(contents.len(), 3);
        assert!(!contents.is_empty());

        // The erase payload keeps the byte count, all 255s.
        assert_eq!(contents.erase_command_data().unwrap(), "255 255 255");

        assert!(matches!(
            "255 x 32".parse::<EepromContents>(),
            Err(ParseSettingsError::EepromContents(_))
        ));
        assert!(matches!(
            "300".parse::<EepromContents>(),
            Err(ParseSettingsError::EepromContents(_))
        ));
    }

    #[test]
    fn empty_eeprom_contents_refuses_to_erase() {
        // An empty dump is a real wire state (Bazecor's virtual
        // devices report ""), so it parses — but an empty erase
        // payload would hit the firmware's `isEOL` and be answered as
        // a read, silently erasing nothing. It must be a typed error.
        let contents = "".parse::<EepromContents>().unwrap();

        assert!(contents.is_empty());

        assert!(matches!(
            contents.erase_command_data(),
            Err(EmptyEepromError)
        ));
        assert!(matches!(
            EepromContents(Vec::new()).erase_command_data(),
            Err(EmptyEepromError)
        ));
    }

    #[test]
    fn eeprom_contents_parses_large_real_shaped_dump() {
        // The Defy's EEPROM holds thousands of bytes; a dump is one
        // long line of decimals. 300 bytes is plenty to prove the
        // shape.
        let data = std::iter::repeat_n("7 ", 300).collect::<String>();

        let contents = data.parse::<EepromContents>().unwrap();

        assert_eq!(contents.len(), 300);
        assert!(contents.0.iter().all(|&byte| byte == 7));
    }
}
