//! Types for parsing and serializing the wireless and battery Focus
//! commands (`wireless.battery.*`, `wireless.rf.*`,
//! `wireless.bluetooth.*`).
//!
//! The Defy's wireless side is served by its wireless Neuron's
//! firmware, whose sketch is NOT in the local Kaleidoscope checkout —
//! so, like WS-B's wireless settings and WS-C's wireless LED commands,
//! every shape here rests on Bazecor's sources (the `Wireless.tsx` and
//! `BatteryStatus.tsx` views, the `BatteryStatusSide` indicator, the
//! in-repo `FOCUS_API.md`) plus the captured responses (the shipped
//! default backups and the app's virtual keyboards, recorded by
//! `tools/bazecor-differential/record-wireless.mjs`). Per-claim
//! evidence is cited on each type.

use std::str::FromStr;

use winnow::{
    ModalResult, Parser,
    ascii::{dec_uint, space0},
    combinator::terminated,
    stream::AsChar,
    token::take_while,
};

/// Error returned when a wireless command response does not match the
/// wire shape the firmware serves for it.
#[derive(Clone, Debug, Display, Error)]
pub enum ParseWirelessError {
    /// A battery level response was expected (a percentage decimal,
    /// 0-100), but the data was something else.
    #[display("expected a battery level percentage (0-100), got: {_0}")]
    BatteryLevel(#[error(not(source))] String),
    /// A battery status response was expected (a decimal status code,
    /// or one of the hex words the firmware is known to serve), but
    /// the data was something else.
    #[display("expected a battery status code, got: {_0}")]
    BatteryStatus(#[error(not(source))] String),
    /// A `wireless.rf.power` response carried a number other than the
    /// 0/1/2 the firmware documents for it.
    #[display("expected an RF power level (`0`, `1`, or `2`), got: {_0}")]
    RfPower(#[error(not(source))] String),
    /// A `wireless.rf.channelHop` response was expected (a decimal
    /// `1`/`0` or a `true`/`false` word — both forms are captured),
    /// but the data was something else.
    #[display("expected `1`, `0`, `true`, or `false`, got: {_0}")]
    RfChannelHop(#[error(not(source))] String),
}

/// Which half of the split keyboard a battery command addresses.
///
/// The side is spelled into the command name on the wire
/// (`wireless.battery.left.level` vs `wireless.battery.right.level`),
/// never sent as an argument.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BatterySide {
    /// The left half.
    Left,
    /// The right half.
    Right,
}

impl std::fmt::Display for BatterySide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Left => "left",
            Self::Right => "right",
        })
    }
}

/// A side's battery charge percentage (`wireless.battery.{left,right}
/// .level`), guaranteed to be in the 0-100 range the firmware
/// documents (`FOCUS_API.md`: "a charge percentage between 0 and
/// 100%").
///
/// The inner value is private so an out-of-range level is
/// unrepresentable; construct values through [`FromStr`] or
/// [`BatteryLevel::new`].
///
/// Evidence: `FOCUS_API.md` documents the 0-100 domain; Bazecor's
/// views parse the response with `parseInt` (Wireless.tsx) and render
/// it as a percentage; the app's virtual wireless Defy answers `80`
/// (recorded in `expectations/wireless.json`). The commands are
/// deny-listed from backups (the `wireless.battery.left`/`right`
/// prefixes in `Backup.ts` `Commands`), so no real capture exists —
/// the domain check enforces the documented shape, and a device that
/// answered something else would fail the read with the value named,
/// not misreport a charge.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct BatteryLevel(u8);

impl BatteryLevel {
    /// The documented wire ceiling (`FOCUS_API.md`).
    pub const MAX: u8 = 100;

    /// Creates a level, returning `None` when the percentage exceeds
    /// [`BatteryLevel::MAX`] and cannot be a real charge reading.
    pub fn new(percentage: u8) -> Option<Self> {
        (percentage <= Self::MAX).then_some(Self(percentage))
    }

    /// The percentage.
    pub const fn get(self) -> u8 {
        self.0
    }
}

impl FromStr for BatteryLevel {
    type Err = ParseWirelessError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let level = battery_level_parser
            .parse(s)
            .map_err(|_| ParseWirelessError::BatteryLevel(s.to_owned()))?;

        Self::new(level).ok_or(ParseWirelessError::BatteryLevel(s.to_owned()))
    }
}

impl std::fmt::Display for BatteryLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A side's battery status (`wireless.battery.{left,right}.status`).
///
/// The five documented codes carry the meanings `FOCUS_API.md` and
/// Bazecor's `BatteryStatusSide` indicator agree on; any other code —
/// including the hex words the firmware is known to serve in the wild
/// (Bazecor's `BatteryStatus.tsx` maps every `0x…` response to its
/// `fatal-error` state) — is preserved as
/// [`BatteryStatus::Unknown`] instead of being rejected, because
/// responses outside the documented set are a real wire state the
/// app's poll demonstrably meets (its dedicated `0x…` branch exists
/// for exactly that traffic). This is the wire layer's deliberate
/// totality: convert into domain decisions only after matching.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatteryStatus {
    /// `0`: the battery is in normal/drain mode, being actively
    /// discharged.
    Discharging,
    /// `1`: the battery is charging.
    Charging,
    /// `2`: the battery is fully charged.
    Full,
    /// `3`: the battery is faulty or has a reading error.
    Fault,
    /// `4`: the battery is physically disconnected from the RF board.
    /// (Bazecor's poll retries while a side reads this, waiting for
    /// the sides to answer.)
    Disconnected,
    /// Any other code the firmware serves: the decimal value of a hex
    /// response (`0x…`), or an undocumented decimal. Bazecor's UI
    /// collapses all of these into one "fatal error" state (hex maps
    /// to 255, anything else falls into the same switch arm); the raw
    /// value is kept here so a real reading is never invented away.
    Unknown(u16),
}

impl BatteryStatus {
    /// The documented codes, in wire order 0-4.
    const KNOWN: [Self; 5] = [
        Self::Discharging,
        Self::Charging,
        Self::Full,
        Self::Fault,
        Self::Disconnected,
    ];

    /// The human-readable description of this status: short forms of
    /// the meanings `FOCUS_API.md` documents for the documented codes,
    /// and the raw value named for the undocumented ones. (Bazecor's
    /// indicator maps the codes to visual states, rendering words only
    /// for the fault, disconnected, and fatal-error cases — the words
    /// here are the SDK's own.)
    pub fn description(self) -> String {
        match self {
            Self::Discharging => "discharging".to_owned(),
            Self::Charging => "charging".to_owned(),
            Self::Full => "fully charged".to_owned(),
            Self::Fault => "fault".to_owned(),
            Self::Disconnected => "disconnected".to_owned(),
            Self::Unknown(value) => format!("unknown status ({value})"),
        }
    }
}

impl FromStr for BatteryStatus {
    type Err = ParseWirelessError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let token = leading_alphanumeric_token
            .parse(s)
            .map_err(|_| ParseWirelessError::BatteryStatus(s.to_owned()))?;

        // A hex word: the response form Bazecor's `includes("0x")`
        // check exists for. Every hex response lands in Unknown with
        // its parsed value.
        if let Some(hex) = token
            .strip_prefix("0x")
            .or_else(|| token.strip_prefix("0X"))
        {
            let value = u16::from_str_radix(hex, 16)
                .map_err(|_| ParseWirelessError::BatteryStatus(s.to_owned()))?;

            return Ok(Self::Unknown(value));
        }

        let value = status_decimal_parser
            .parse(token)
            .map_err(|_| ParseWirelessError::BatteryStatus(s.to_owned()))?;

        Ok(Self::KNOWN
            .get(value as usize)
            .copied()
            .unwrap_or(Self::Unknown(value)))
    }
}

impl std::fmt::Display for BatteryStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Discharging => f.write_str("0"),
            Self::Charging => f.write_str("1"),
            Self::Full => f.write_str("2"),
            Self::Fault => f.write_str("3"),
            Self::Disconnected => f.write_str("4"),
            Self::Unknown(value) => write!(f, "{value}"),
        }
    }
}

/// Parses a battery percentage optionally followed by whitespace, and
/// nothing else — `.parse` demands the whole input, so a second token
/// is a failure. The firmware terminates every value it sends with a
/// space separator, so the trailing space is the norm.
fn battery_level_parser(input: &mut &str) -> ModalResult<u8> {
    terminated(dec_uint::<_, u8, _>, space0).parse_next(input)
}

/// The `u16` RF power flavor of [`battery_level_parser`].
fn rf_power_parser(input: &mut &str) -> ModalResult<u16> {
    terminated(dec_uint::<_, u16, _>, space0).parse_next(input)
}

/// A complete decimal status value (the whole input, no trailing
/// anything) — the decimal form of a battery status token.
fn status_decimal_parser(input: &mut &str) -> ModalResult<u16> {
    dec_uint.parse_next(input)
}

/// The first whitespace-delimited run of alphanumeric characters of a
/// response, optionally followed by whitespace, and nothing else —
/// the token form every parsed wireless response value takes
/// (decimals, `0x…` hex words, `true`/`false`).
fn leading_alphanumeric_token<'a>(input: &mut &'a str) -> ModalResult<&'a str> {
    terminated(take_while(1.., AsChar::is_alphanum), space0).parse_next(input)
}

/// The RF signal strength between the Neuron and the sides
/// (`wireless.rf.power`).
///
/// The three documented levels are the whole domain (`FOCUS_API.md`
/// documents 0 as low signal power, 1 as medium, and 2 as high;
/// Bazecor's `RFSettings.tsx` offers the same three as
/// Low/Medium/High), so a response outside them fails the read with
/// the value named instead of inventing a level.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RfPower {
    /// `0`: low signal power — the least range, the best battery
    /// life.
    Low,
    /// `1`: medium signal power.
    Medium,
    /// `2`: high signal power — the most range, the worst battery
    /// life.
    High,
}

impl RfPower {
    /// The level name Bazecor's `RFSettings.tsx` renders for this
    /// power level, lowercased.
    pub fn description(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }

    /// Converts the level into a form suitable for sending to the
    /// keyboard as the data of a `wireless.rf.power` write.
    pub fn to_command_data(self) -> &'static str {
        match self {
            Self::Low => "0",
            Self::Medium => "1",
            Self::High => "2",
        }
    }
}

impl FromStr for RfPower {
    type Err = ParseWirelessError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let value = rf_power_parser
            .parse(s)
            .map_err(|_| ParseWirelessError::RfPower(s.to_owned()))?;

        match value {
            0 => Ok(Self::Low),
            1 => Ok(Self::Medium),
            2 => Ok(Self::High),
            _ => Err(ParseWirelessError::RfPower(s.to_owned())),
        }
    }
}

impl std::fmt::Display for RfPower {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.to_command_data())
    }
}

/// Whether the RF link hops channels (`wireless.rf.channelHop`).
///
/// The wire is a decimal `1`/`0` on the real devices (the shipped
/// Defy backups capture `0`), while Bazecor's virtual wireless Defy
/// answers with a bool word (`false`, the one it serves) — both forms
/// parse, because the rule for captures is to accept the union of the
/// shapes the sources actually carry. Writes always send the decimal
/// the real devices read back (`Wireless.tsx` writes the number it
/// read).
///
/// The command is absent from `FOCUS_API.md` and from this Bazecor's
/// wireless UI (the view only reads it and writes the value back
/// unchanged), so the setting's observable semantics — channel
/// hopping on the RF link — rest on the command's name alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RfChannelHop(pub bool);

impl FromStr for RfChannelHop {
    type Err = ParseWirelessError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let token = leading_alphanumeric_token
            .parse(s)
            .map_err(|_| ParseWirelessError::RfChannelHop(s.to_owned()))?;

        match token {
            "1" | "true" => Ok(Self(true)),
            "0" | "false" => Ok(Self(false)),
            _ => Err(ParseWirelessError::RfChannelHop(s.to_owned())),
        }
    }
}

impl std::fmt::Display for RfChannelHop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(rf_channel_hop_command_data(self.0))
    }
}

/// Returns the command data for writing an [`RfChannelHop`]: `"1"` or
/// `"0"`. The single source of the decimal form — the [`Display`]
/// implementation delegates here, so the value a write sends and the
/// value a read prints can't drift apart.
pub fn rf_channel_hop_command_data(value: bool) -> &'static str {
    if value { "1" } else { "0" }
}

/// The keyboard's Bluetooth device name (`wireless.bluetooth.deviceName`).
///
/// The response is the name verbatim, possibly empty — every shipped
/// Defy backup captures an empty one (the factory state) — so no wire
/// shape is imposed beyond trimming the response's separator
/// whitespace. The name travels on the wire unquoted, exactly as
/// Bazecor sends it (`Wireless.tsx` writes the raw string; `FOCUS_API.md`
/// does not document this command, so its write grammar — including
/// how the firmware reads multi-word names back — rests on Bazecor's
/// behavior alone).
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct DeviceName(String);

impl DeviceName {
    /// Creates a device name from a raw string, trimming the
    /// surrounding whitespace a response carries as separators. Empty
    /// names are valid: that is the factory state every shipped Defy
    /// backup captures.
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into().trim().to_owned())
    }

    /// Borrows the inner name string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Converts the name into a form suitable for sending to the
    /// keyboard as the data of a `wireless.bluetooth.deviceName`
    /// write: the name verbatim, unquoted.
    ///
    /// An empty name writes an empty payload, which the firmware
    /// answers as a read (the same isEOL behavior every Focus command
    /// shares) — a silent no-op, not a blank name.
    pub fn to_command_data(&self) -> String {
        self.0.clone()
    }
}

impl std::fmt::Display for DeviceName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// One of the five Bluetooth info channels the
/// `wireless.bluetooth.infoChannel <1..5>` command serves.
///
/// The inner value is private so a channel outside the documented
/// 1-5 range is unrepresentable; construct values through
/// [`BluetoothChannelId::new`] or [`BluetoothChannelId::ALL`].
///
/// Evidence: Bazecor reads channels 1-5 and nothing else
/// (`Wireless.tsx`, `Preferences.tsx`), stores each response as a raw
/// string, and never renders or interprets it — no capture of any
/// response exists (the commands are not in the shipped backups or
/// the virtual keyboards, and `FOCUS_API.md` does not document them),
/// so the SDK serves the raw strings the device answers with and
/// claims nothing about their content.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BluetoothChannelId(u8);

impl BluetoothChannelId {
    /// The lowest channel the command serves.
    pub const MIN: u8 = 1;

    /// The highest channel the command serves.
    pub const MAX: u8 = 5;

    /// All five channels, in order.
    pub const ALL: [Self; 5] = [Self(1), Self(2), Self(3), Self(4), Self(5)];

    /// Creates a channel id, returning `None` when the number is
    /// outside the 1-5 range the command serves.
    pub fn new(number: u8) -> Option<Self> {
        ((Self::MIN..=Self::MAX).contains(&number)).then_some(Self(number))
    }

    /// The channel number.
    pub const fn get(self) -> u8 {
        self.0
    }

    /// The argument the `wireless.bluetooth.infoChannel` write carries:
    /// the channel number as a decimal.
    pub fn command_data(self) -> String {
        self.0.to_string()
    }
}

impl std::fmt::Display for BluetoothChannelId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The recorded wireless responses of Bazecor's data sources.
    #[derive(serde::Deserialize)]
    struct RecordedWireless {
        bazecor_commit: String,
        /// The WS-D scope commands the recording covers.
        #[serde(rename = "scope_commands")]
        scope_commands: Vec<String>,
        /// Virtual keyboard and shipped-backup captures, keyed by
        /// source then command. Sources also carry wireless commands
        /// OUTSIDE the WS-D scope (the `wireless.energy.*` family,
        /// `wireless.rf.stability`, the `wireless.bluetooth` store
        /// management commands) — recorded as the evidence map, not
        /// parsed here: issue #4's command list does not include them,
        /// and this unit builds none; they are tracked for a scope
        /// decision in their own parity issue (#23).
        sources: std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>>,
        /// Commands whose captured forms disagree between sources,
        /// recorded as notes instead of equality checks, and the
        /// scope commands no source records at all.
        notes: Vec<String>,
    }

    /// The recorded expectations.
    fn recorded_wireless() -> RecordedWireless {
        serde_json::from_str(include_str!(
            "../../../../tools/bazecor-differential/expectations/wireless.json"
        ))
        .expect("the recorded wireless expectation file parses")
    }

    /// The panic-with-context failure for a recorded response that
    /// does not parse.
    fn parse_failure<'a, E: std::fmt::Debug, T>(
        source: &'a str,
        command: &'a str,
        data: &'a str,
    ) -> impl Fn(E) -> T + 'a {
        move |err| panic!("{source} `{command}` => `{data}`: {err:?}")
    }

    /// Checks one recorded response with the SDK type its command
    /// maps to, exactly as the source records it.
    fn assert_recorded_response_parses(source: &str, command: &str, data: &str) {
        match command {
            "wireless.battery.left.level" | "wireless.battery.right.level" => {
                let level = data
                    .parse::<BatteryLevel>()
                    .unwrap_or_else(parse_failure(source, command, data));

                assert_eq!(level.to_string(), data, "{source} `{command}`");
            }
            "wireless.battery.left.status" | "wireless.battery.right.status" => {
                let status = data
                    .parse::<BatteryStatus>()
                    .unwrap_or_else(parse_failure(source, command, data));

                assert_eq!(status.to_string(), data, "{source} `{command}`");
                assert_eq!(
                    status,
                    BatteryStatus::Charging,
                    "{source} `{command}`: every recording source (four \
                     virtual keyboards, the Raise2 backup pair) answers 1"
                );
            }
            "wireless.battery.savingMode" => {
                let mode = data
                    .parse::<crate::focus_api::parsing::settings::OnOff>()
                    .unwrap_or_else(parse_failure(source, command, data));

                assert!(!mode.0, "{source} `{command}`: every capture reads off");
            }
            "wireless.rf.power" => {
                let power = data
                    .parse::<RfPower>()
                    .unwrap_or_else(parse_failure(source, command, data));

                assert_eq!(power, RfPower::Low, "{source} `{command}`");
                assert_eq!(power.to_string(), data, "{source} `{command}`");
            }
            "wireless.rf.channelHop" => {
                // Both captured forms — the backups' decimal and the
                // virtual keyboards' bool word — are the same Off,
                // and both round-trip to the decimal the real devices
                // read back.
                let hop = data
                    .parse::<RfChannelHop>()
                    .unwrap_or_else(parse_failure(source, command, data));

                assert!(!hop.0, "{source} `{command}`");
                assert_eq!(hop.to_string(), "0", "{source} `{command}`");
            }
            "wireless.bluetooth.deviceName" => {
                let name = DeviceName::new(data);

                assert_eq!(name.as_str(), data.trim(), "{source} `{command}`");
                assert_eq!(name.to_command_data(), data.trim(), "{source} `{command}`");
            }
            "wireless.rf.syncPairing" => {
                // An action command: every capture holds the empty
                // response (the command produces no output), which is
                // data, not a parse shape.
                assert_eq!(data.trim(), "", "{source} `{command}`");
            }
            other => panic!("unmapped scope command `{other}` — extend the match"),
        }
    }

    #[test]
    fn differential_recording_is_pinned_to_the_audited_bazecor_commit() {
        assert_eq!(
            recorded_wireless().bazecor_commit,
            "ff9e10b35f9606479c63d01da06c40b7c074f8bb"
        );
    }

    #[test]
    fn our_parses_agree_with_every_recorded_response() {
        // Every recorded WS-D response parses with the SDK type its
        // command maps to. This is the WS-D differential: the shapes
        // are the app's own data, machine-recorded, not transcribed.
        let recorded = recorded_wireless();

        for (source, commands) in &recorded.sources {
            for command in &recorded.scope_commands {
                let Some(data) = commands.get(command) else {
                    continue;
                };

                assert_recorded_response_parses(source, command, data);
            }
        }
    }

    #[test]
    fn the_recording_covers_every_bazecor_source_that_carries_wireless_commands() {
        // The differential iterates whatever sources the recording
        // carries, so a source silently dropped from the harness would
        // shrink the evidence map without failing anything. This pins
        // the source set itself: the four wireless virtual keyboards
        // and the five shipped backups Bazecor ships (the wired
        // Raise/Raise-1 sources and DefyWired carry no wireless
        // commands — verified at the pinned commit).
        let recorded = recorded_wireless();
        let mut expected = [
            "virtual DefyWireless",
            "virtual SonseiWireless",
            "virtual Raise2ANSI",
            "virtual Raise2ISO",
            "backup Defy-wireless",
            "backup Defy-wired",
            "backup Sonsei",
            "backup Raise2ANSI",
            "backup Raise2ISO",
        ];
        expected.sort_unstable();

        let mut recorded_sources = recorded.sources.keys().cloned().collect::<Vec<_>>();
        recorded_sources.sort_unstable();

        assert_eq!(recorded_sources, expected);
    }

    #[test]
    fn the_uncaptured_scope_commands_are_recorded_as_absent() {
        // `wireless.battery.forceRead` and the five `infoChannel`
        // reads are captured by NO source (not the virtual keyboards,
        // not the shipped backups) — the thin-evidence end of the
        // WS-D map. The recording must keep saying so, so the SDK's
        // docs cannot quietly drift into claiming captures that do
        // not exist.
        let recorded = recorded_wireless();

        let uncaptured = ["wireless.battery.forceRead"]
            .map(ToOwned::to_owned)
            .into_iter()
            .chain((1..=5).map(|channel| format!("wireless.bluetooth.infoChannel {channel}")));

        for command in uncaptured {
            assert!(
                !recorded
                    .sources
                    .values()
                    .any(|commands| commands.contains_key(&command)),
                "`{command}` gained a capture — re-adjudicate its shape"
            );
            assert!(
                recorded.notes.iter().any(|note| note.contains(&command)),
                "`{command}` must stay recorded as absent"
            );
        }
    }

    #[test]
    fn the_recorded_channel_hop_disagreement_is_a_known_note() {
        // The virtual Defy answers `false` where the real devices
        // answer `0`; the recording keeps both and notes the
        // disagreement instead of papering over it.
        let recorded = recorded_wireless();

        assert!(
            recorded
                .notes
                .iter()
                .any(|note| note.contains("channelHop")),
            "the channelHop outlier must stay recorded"
        );

        assert_eq!(
            "false".parse::<RfChannelHop>().unwrap(),
            RfChannelHop(false)
        );
        assert_eq!("0".parse::<RfChannelHop>().unwrap(), RfChannelHop(false));
    }

    #[test]
    fn battery_level_parses_documented_domain() {
        assert_eq!("80 ".parse::<BatteryLevel>().unwrap(), BatteryLevel(80));
        assert_eq!("0".parse::<BatteryLevel>().unwrap(), BatteryLevel(0));
        assert_eq!("100".parse::<BatteryLevel>().unwrap(), BatteryLevel(100));
        assert_eq!(BatteryLevel(80).to_string(), "80");
        assert_eq!(BatteryLevel::new(100).unwrap().get(), 100);
        assert!(BatteryLevel::new(101).is_none());

        // Outside the documented 0-100 domain, or not a decimal: a
        // typed failure naming the value, never a made-up charge.
        assert!(matches!(
            "101 ".parse::<BatteryLevel>(),
            Err(ParseWirelessError::BatteryLevel(_))
        ));
        assert!(matches!(
            "255".parse::<BatteryLevel>(),
            Err(ParseWirelessError::BatteryLevel(_))
        ));
        assert!(matches!(
            "".parse::<BatteryLevel>(),
            Err(ParseWirelessError::BatteryLevel(_))
        ));
        assert!(matches!(
            "abc".parse::<BatteryLevel>(),
            Err(ParseWirelessError::BatteryLevel(_))
        ));
        assert!(matches!(
            "80 40".parse::<BatteryLevel>(),
            Err(ParseWirelessError::BatteryLevel(_))
        ));
    }

    #[test]
    fn battery_status_parses_documented_codes_and_hex_words() {
        assert_eq!(
            "1 ".parse::<BatteryStatus>().unwrap(),
            BatteryStatus::Charging
        );
        assert_eq!(
            "0".parse::<BatteryStatus>().unwrap(),
            BatteryStatus::Discharging
        );
        assert_eq!("2".parse::<BatteryStatus>().unwrap(), BatteryStatus::Full);
        assert_eq!("3".parse::<BatteryStatus>().unwrap(), BatteryStatus::Fault);
        assert_eq!(
            "4".parse::<BatteryStatus>().unwrap(),
            BatteryStatus::Disconnected
        );

        // The in-the-wild hex form: kept as its parsed value, where
        // Bazecor's UI collapses it to one fatal-error state.
        assert_eq!(
            "0xFF ".parse::<BatteryStatus>().unwrap(),
            BatteryStatus::Unknown(255)
        );
        assert_eq!(
            "0x0A".parse::<BatteryStatus>().unwrap(),
            BatteryStatus::Unknown(10)
        );
        assert_eq!(
            "0X2".parse::<BatteryStatus>().unwrap(),
            BatteryStatus::Unknown(2)
        );

        // Undocumented decimals are preserved, not rejected: the
        // wire layer is total.
        assert_eq!(
            "9 ".parse::<BatteryStatus>().unwrap(),
            BatteryStatus::Unknown(9)
        );
        assert_eq!(BatteryStatus::Unknown(255).to_string(), "255");
        assert_eq!(BatteryStatus::Full.to_string(), "2");

        // The descriptions are short forms of FOCUS_API.md's
        // documented meanings — the SDK's own words, not Bazecor's
        // (its indicator renders words only for fault/disconnected/
        // fatal-error).
        assert_eq!(BatteryStatus::Discharging.description(), "discharging");
        assert_eq!(BatteryStatus::Full.description(), "fully charged");
        assert_eq!(
            BatteryStatus::Unknown(255).description(),
            "unknown status (255)"
        );

        // Not a status: a typed failure naming the value.
        assert!(matches!(
            "".parse::<BatteryStatus>(),
            Err(ParseWirelessError::BatteryStatus(_))
        ));
        assert!(matches!(
            "charging".parse::<BatteryStatus>(),
            Err(ParseWirelessError::BatteryStatus(_))
        ));
        assert!(matches!(
            "0xZZ".parse::<BatteryStatus>(),
            Err(ParseWirelessError::BatteryStatus(_))
        ));
        assert!(matches!(
            "1 0".parse::<BatteryStatus>(),
            Err(ParseWirelessError::BatteryStatus(_))
        ));
        assert!(matches!(
            "-1".parse::<BatteryStatus>(),
            Err(ParseWirelessError::BatteryStatus(_))
        ));
    }

    #[test]
    fn rf_power_parses_documented_levels() {
        assert_eq!("0 ".parse::<RfPower>().unwrap(), RfPower::Low);
        assert_eq!("1".parse::<RfPower>().unwrap(), RfPower::Medium);
        assert_eq!("2".parse::<RfPower>().unwrap(), RfPower::High);

        assert_eq!(RfPower::Low.to_string(), "0");
        assert_eq!(RfPower::Medium.to_command_data(), "1");
        assert_eq!(RfPower::High.description(), "high");

        // Outside the three documented levels: a typed failure.
        assert!(matches!(
            "3 ".parse::<RfPower>(),
            Err(ParseWirelessError::RfPower(_))
        ));
        assert!(matches!(
            "255".parse::<RfPower>(),
            Err(ParseWirelessError::RfPower(_))
        ));
        assert!(matches!(
            "high".parse::<RfPower>(),
            Err(ParseWirelessError::RfPower(_))
        ));
        assert!(matches!(
            "".parse::<RfPower>(),
            Err(ParseWirelessError::RfPower(_))
        ));
    }

    #[test]
    fn rf_channel_hop_parses_both_captured_forms() {
        assert_eq!("1 ".parse::<RfChannelHop>().unwrap(), RfChannelHop(true));
        assert_eq!("0".parse::<RfChannelHop>().unwrap(), RfChannelHop(false));
        assert_eq!("true".parse::<RfChannelHop>().unwrap(), RfChannelHop(true));
        assert_eq!(
            "false ".parse::<RfChannelHop>().unwrap(),
            RfChannelHop(false)
        );

        // Writes are the decimals the real devices read back.
        assert_eq!(RfChannelHop(true).to_string(), "1");
        assert_eq!(rf_channel_hop_command_data(false), "0");

        assert!(matches!(
            "2 ".parse::<RfChannelHop>(),
            Err(ParseWirelessError::RfChannelHop(_))
        ));
        assert!(matches!(
            "on".parse::<RfChannelHop>(),
            Err(ParseWirelessError::RfChannelHop(_))
        ));
        assert!(matches!(
            "".parse::<RfChannelHop>(),
            Err(ParseWirelessError::RfChannelHop(_))
        ));
    }

    #[test]
    fn device_name_trims_and_allows_empty() {
        assert_eq!(DeviceName::new("My Defy").as_str(), "My Defy");
        assert_eq!(DeviceName::new("My Defy ").as_str(), "My Defy");
        assert_eq!(DeviceName::new("").as_str(), "");
        assert_eq!(DeviceName::default().as_str(), "");

        // The factory state: every shipped Defy backup captures an
        // empty name.
        let captured = DeviceName::new("");

        assert_eq!(captured.to_command_data(), "");
        assert_eq!(captured.to_string(), "");
    }

    #[test]
    fn bluetooth_channel_ids_are_exactly_one_through_five() {
        assert_eq!(
            BluetoothChannelId::ALL
                .iter()
                .map(|channel| channel.get())
                .collect::<Vec<_>>(),
            vec![1, 2, 3, 4, 5]
        );

        assert!(BluetoothChannelId::new(1).is_some());
        assert!(BluetoothChannelId::new(5).is_some());
        assert!(BluetoothChannelId::new(0).is_none());
        assert!(BluetoothChannelId::new(6).is_none());
        assert!(BluetoothChannelId::new(255).is_none());

        assert_eq!(BluetoothChannelId::new(3).unwrap().to_string(), "3");
        assert_eq!(BluetoothChannelId::new(5).unwrap().command_data(), "5");
    }

    #[test]
    fn battery_sides_display() {
        assert_eq!(BatterySide::Left.to_string(), "left");
        assert_eq!(BatterySide::Right.to_string(), "right");
    }
}
