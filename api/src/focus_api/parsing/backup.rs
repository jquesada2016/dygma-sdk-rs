//! The Bazecor-compatible device backup file format, and the
//! `help`-driven list of commands a backup captures.
//!
//! The file format is the one Bazecor's `DoBackup` produces and its
//! restore flow consumes (`bazecor/src/api/backup/Backup.ts`, with the
//! shipped examples in `bazecor/src/defaultBackups/*.json`):
//! `{neuronID, neuron: {id, name, layers, macros, superkeys, device},
//! versions, backup: [{command, data}]}`. The `backup` array is the
//! actual device state: one Focus command name plus its verbatim
//! response per entry, in capture order.
//!
//! Interop notes, byte-verified against the shipped Bazecor backups:
//!
//! - `superkeys.map` and `macros.map` entries store only the live
//!   bytes — no `65535` padding and no trailing space. The parsers in
//!   this crate accept both those and the padded device form.
//! - After saving a backup, Bazecor's firmware-upgrade flow
//!   (`renderer/controller/DeviceChecks/actions.ts`) rewrites the
//!   device's first `keymap.custom` code to `41` — a quirk of that
//!   flow, not device behavior, and nothing a backup file carries by
//!   construction. It is **not** replicated here: backups created by
//!   this crate preserve the captured data verbatim.
//! - The deny list below filters with substring semantics exactly like
//!   Bazecor's `Backup.Commands` (case-sensitive `contains`), so
//!   `settings.defaultLayer` survives (the deny entry is lowercase
//!   `layer`) while `layer.activate` does not. One deviation: empty
//!   lines are dropped from the capture list instead of being captured
//!   as empty commands — Bazecor captures them, and its own shipped
//!   Sonsei backup carries the resulting `{"command": "", "data": ""}`
//!   artifact. Foreign files carrying that artifact still parse, and
//!   the restore sequence drops such entries (an empty command carries
//!   no state to restore).
//! - A backup saved from a capture that failed partway carries a
//!   `partial` marker field ([`PartialCapture`]) that Bazecor's own
//!   files never have. It is absent on complete captures. Bazecor's
//!   import is inert to the key (plain `JSON.parse`, no schema
//!   validation), but its restore is not: it replaces Bazecor's own
//!   persisted neuron entry for the connected neuron with the
//!   file's name-less neuron, then — a partial file's neuron
//!   carries no `device` — throws a TypeError before anything
//!   reaches the device. The device is never touched; most of
//!   Bazecor's flows swallow the error silently, and only the flows
//!   calling its restore directly surface it (one toasts it, one
//!   logs it). This crate's own tooling refuses such files up
//!   front, naming the missing commands.

/// The commands a backup does **not** capture, as substring patterns
/// over the device's `help` output.
///
/// These are Bazecor's non-persistent or otherwise useless-for-restore
/// commands (`bazecor/src/api/backup/Backup.ts`, `Backup.Commands`),
/// mirrored exactly: identity probes (`version`, `hardware.*`,
/// `eeprom*`, `help`), volatile state (`layer.*`, `led.mode`/`theme`/
/// `at`/bulk setters, `macros.trigger`, `wireless.battery.left`/
/// `right`), firmware diagnostics (`settings.valid?`/`version`/`crc`/
/// `printConfig`/`aliveInterval`/`spiSpeed`/`cpuSpeed`/`ledDriver*`/
/// `underGlow`), and the firmware updater (`upgrade`).
pub const BACKUP_COMMAND_DENY_LIST: &[&str] = &[
    "eeprom",
    "hardware",
    "settings.valid?",
    "settings.version",
    "settings.crc",
    "keymap.default",
    "layer",
    "help",
    "version",
    "led.mode",
    "led.theme",
    "led.at",
    "led.setMultiple",
    "led.getMultiple",
    "led.setAll",
    "macros.trigger",
    "macros.memory",
    "upgrade",
    "settings.printConfig",
    "settings.aliveInterval",
    "settings.spiSpeed",
    "settings.cpuSpeed",
    "settings.ledDriverPullUp",
    "settings.underGlow",
    "settings.ledDriver",
    "wireless.battery.left",
    "wireless.battery.right",
];

/// The command Bazecor force-adds to every capture list even when the
/// device's `help` output does not mention it
/// (`led.brightnessUG`, the underglow brightness), because some
/// firmwares serve it without listing it.
pub const BACKUP_FORCED_COMMAND: &str = "led.brightnessUG";

/// Computes the list of commands a backup captures from the device's
/// `help` output: every listed command that does not contain an entry
/// of [`BACKUP_COMMAND_DENY_LIST`] as a substring, in `help` order,
/// plus [`BACKUP_FORCED_COMMAND`] appended if `help` does not list it.
///
/// Empty lines are dropped (see the [module docs](self) for the
/// deviation from Bazecor this implies). Duplicates in the `help`
/// output are captured twice, mirroring Bazecor.
pub fn backup_capture_commands(help_commands: &[String]) -> Vec<String> {
    let mut capture_list: Vec<String> = help_commands
        .iter()
        .filter(|command| !command.is_empty())
        .filter(|command| {
            !BACKUP_COMMAND_DENY_LIST
                .iter()
                .any(|denied| command.contains(denied))
        })
        .cloned()
        .collect();

    if !capture_list
        .iter()
        .any(|command| command == BACKUP_FORCED_COMMAND)
    {
        capture_list.push(BACKUP_FORCED_COMMAND.to_owned());
    }

    capture_list
}

/// One captured Focus command: the command name and the device's
/// verbatim response as its `data`.
///
/// `data` is preserved exactly as captured — including the trailing
/// separator space the firmware usually sends — so serializing and
/// re-sending it reproduces the device state byte for byte.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupCommand {
    /// The Focus command name (for example `keymap.custom`).
    pub command: String,
    /// The verbatim response the device sent for the command.
    pub data: String,
}

impl BackupCommand {
    /// Creates a captured-command entry.
    pub fn new(command: String, data: String) -> Self {
        Self { command, data }
    }
}

/// The partial-capture marker of a backup file that was saved from a
/// capture that failed partway
/// ([`DefyKeyboard::create_backup`]'s failure path), serialized as the
/// backup file's `partial` field.
///
/// The field is **present only on partial captures** — a complete
/// capture serializes without it, so the file stays byte-identical to
/// what earlier versions of this crate (and Bazecor) write, and a
/// `partial` field is an unmistakable "this file is incomplete" bit
/// everywhere the format travels. Bazecor's import is inert to the
/// key: its files are read through plain `JSON.parse` with no schema
/// validation (`Backup.ts` `getLatestBackup`,
/// `renderer/modules/Settings/BackupSettings.tsx` `GetBackup`) and
/// its import gate is the presence of the `backup` array
/// (`isBackupType` is `"backup" in backup`). Its restore, though, is
/// not inert: it replaces Bazecor's own persisted neuron entry for
/// the connected neuron with the file's neuron, then — a partial
/// file's neuron carries no `device` — throws a TypeError at the
/// restore's first read of that descriptor, before anything reaches
/// the device. The device is never touched; most of Bazecor's flows
/// swallow the error silently, and only the flows calling its
/// restore directly surface it (one toasts it, one logs it).
///
/// [`DefyKeyboard::create_backup`]:
///     crate::devices::defy::DefyKeyboard::create_backup
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartialCapture {
    /// The commands of the capture list this file does not carry, in
    /// capture-list order: what a successful capture would have
    /// captured in addition to the entries the file holds.
    ///
    /// Empty only in files saved by earlier versions of this crate,
    /// whose captures could still be interrupted after every command
    /// was captured but before the identity probes ran (they are
    /// best-effort now, so they cannot interrupt a capture) — the
    /// marker alone still says the file came from a failed capture.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing: Vec<String>,
}

impl PartialCapture {
    /// Builds the marker for a capture that holds `captured` entries
    /// of the `expected` capture list: the missing-command list of
    /// [`missing_capture_commands`].
    pub fn from_capture(expected: &[String], captured: &[BackupCommand]) -> Self {
        Self {
            missing: missing_capture_commands(expected, captured),
        }
    }

    /// Whether the marker names any missing commands.
    pub fn has_missing(&self) -> bool {
        !self.missing.is_empty()
    }
}

/// Computes the commands of the `expected` capture list that
/// `captured` does not hold, in `expected` order.
///
/// The comparison is a multiset difference: a command the `help`
/// output lists twice is captured twice (Bazecor's behavior), so one
/// captured occurrence satisfies only one expected one. Entries of
/// `captured` the expected list does not mention (a foreign file's
/// extra commands) count against nothing.
pub fn missing_capture_commands(expected: &[String], captured: &[BackupCommand]) -> Vec<String> {
    use std::collections::HashMap;

    let mut uncaptured: HashMap<&str, usize> = HashMap::new();

    for entry in captured {
        *uncaptured.entry(entry.command.as_str()).or_default() += 1;
    }

    expected
        .iter()
        .filter(|command| match uncaptured.get_mut(command.as_str()) {
            Some(remaining @ 1..) => {
                *remaining -= 1;
                false
            }
            _ => true,
        })
        .cloned()
        .collect()
}

/// Computes the command sequence a restore sends for a backup's
/// captured entries, in Bazecor's restore order
/// (`bazecor/src/api/backup/Backup.ts`, `restoreBackup`):
///
/// - Entries with an empty command are dropped. They carry no state to
///   restore — they are the empty-`help`-line capture artifact Bazecor's
///   own shipped Sonsei backup carries (see the [module docs](self)).
/// - Every `superkeys.map` entry that comes *after* the first
///   `keymap.custom` entry is moved directly before it, keeping the
///   entries' relative order and the order of everything else. Bazecor
///   does this so the superkeys are in place before a keymap that may
///   reference them; the match is its `^superkeys?\.map$` regular
///   expression (case-insensitive, singular `superkey` allowed). The
///   `keymap.custom` match itself is exact and case-sensitive, like
///   Bazecor's.
///
/// Everything else is passed through verbatim, including entries this
/// crate has no wire parser for yet. The trailing `led.mode 0` Bazecor
/// sends after the sequence is **not** part of this function's output;
/// the restore seam appends it (see
/// [`DefyKeyboard::restore_backup`]).
///
/// [`DefyKeyboard::restore_backup`]:
///     crate::devices::defy::DefyKeyboard::restore_backup
pub fn restore_command_sequence(entries: &[BackupCommand]) -> Vec<BackupCommand> {
    let entries: Vec<&BackupCommand> = entries
        .iter()
        .filter(|entry| !entry.command.is_empty())
        .collect();

    let rebuilt = entries
        .iter()
        .position(|entry| entry.command == "keymap.custom")
        .map(|keymap_index| rebuild_with_superkeys_first(&entries, keymap_index));

    rebuilt.unwrap_or_else(|| entries.iter().map(|entry| (*entry).clone()).collect())
}

/// Bazecor's reorder (`Backup.ts` `restoreBackup`): the entries after
/// `keymap_index` whose command is a `superkeys.map` (case-insensitive,
/// singular allowed) move directly before it, in their own order;
/// everything else keeps its order.
fn rebuild_with_superkeys_first(
    entries: &[&BackupCommand],
    keymap_index: usize,
) -> Vec<BackupCommand> {
    let to_move: Vec<usize> = (keymap_index + 1..entries.len())
        .filter(|&index| is_superkeys_map_command(&entries[index].command))
        .collect();

    let mut rebuilt = Vec::with_capacity(entries.len());

    for (index, entry) in entries.iter().enumerate() {
        // The moved entries land exactly at the keymap's position, so
        // they are sent before `keymap.custom` itself (which is never
        // in `to_move`: its index precedes the scanned range).
        if index == keymap_index {
            rebuilt.extend(to_move.iter().map(|&moved| entries[moved].clone()));
        }

        if !to_move.contains(&index) {
            rebuilt.push((*entry).clone());
        }
    }

    rebuilt
}

/// Whether a captured command is a superkeys map entry in the sense of
/// Bazecor's restore reorder: its `^superkeys?\.map$` regular
/// expression, case-insensitive.
fn is_superkeys_map_command(command: &str) -> bool {
    command.eq_ignore_ascii_case("superkeys.map") || command.eq_ignore_ascii_case("superkey.map")
}

/// The `versions` object of a backup file: the tokens of the device's
/// `version` response, keyed the way Bazecor keys them.
///
/// A token the device did not send is `None` and omitted when
/// serializing, matching the shipped backups — the Defy examples carry
/// only `bazecor`, the Raise ones all three tokens.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupVersions {
    /// The Bazecor-side firmware version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bazecor: Option<String>,
    /// The Kaleidoscope version the firmware is built on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kaleidoscope: Option<String>,
    /// The firmware version itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub firmware: Option<String>,
}

impl BackupVersions {
    /// Splits a `version` command response into the versions object,
    /// the way Bazecor's `DoBackup` does: the first three whitespace
    /// tokens are the Bazecor, Kaleidoscope, and firmware versions.
    /// Missing tokens become `None`.
    pub fn from_version_response(response: &str) -> Self {
        let mut tokens = response.split_whitespace().map(ToOwned::to_owned);

        Self {
            bazecor: tokens.next(),
            kaleidoscope: tokens.next(),
            firmware: tokens.next(),
        }
    }
}

/// The `neuron` object of a backup file: Bazecor's app-side, per-device
/// store entry keyed by the Neuron's `hardware.chip_id`.
///
/// The device stores numbers only — layer, macro, and superkey *names*
/// live app-side in this object, so a backup created from scratch by
/// this crate carries the empty store shape (empty strings and arrays,
/// exactly what Bazecor's `DoBackup` writes when it has no stored
/// neuron). Parsing keeps any real store content verbatim: the store
/// entries are opaque [`serde_json::Value`]s here because their shape
/// is Bazecor-internal and evolved across releases (the shipped Sonsei
/// backup's superkey entries even carry an extra `superkey` field).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BackupNeuron {
    /// The Neuron's `hardware.chip_id`, matching the backup's
    /// `neuronID`.
    #[serde(default)]
    pub id: String,
    /// The user-chosen name of the keyboard, or an empty string when
    /// unknown.
    #[serde(default)]
    pub name: String,
    /// The stored layer name entries, preserved verbatim.
    #[serde(default)]
    pub layers: Vec<serde_json::Value>,
    /// The stored macro name entries, preserved verbatim.
    #[serde(default)]
    pub macros: Vec<serde_json::Value>,
    /// The stored superkey name entries, preserved verbatim.
    #[serde(default)]
    pub superkeys: Vec<serde_json::Value>,
    /// The device descriptor the backup was captured with, preserved
    /// verbatim (Bazecor's restore flow reads the product out of it).
    /// Absent when unknown, which matches the shape Bazecor's own
    /// no-neuron fallback serializes to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device: Option<serde_json::Value>,
}

impl BackupNeuron {
    /// Whether this store entry carries any user-visible names: a
    /// non-empty keyboard name, or any layer, macro, or superkey name
    /// entries.
    ///
    /// The names store and the restore path use this to detect the
    /// "name-less backup would replace a named neuron" case — a backup
    /// captured without a store carries the empty neuron shape — which
    /// the CLI warns about before applying names.
    pub fn has_names(&self) -> bool {
        !self.name.is_empty()
            || !self.layers.is_empty()
            || !self.macros.is_empty()
            || !self.superkeys.is_empty()
    }
}

/// A Bazecor-compatible device backup file.
///
/// Parse it from JSON with serde (the CLI reads these files through
/// [`DefyBackup`]'s `Deserialize`); create one from a connected device
/// with [`DefyKeyboard::create_backup`].
///
/// [`DefyKeyboard::create_backup`]:
///     crate::devices::defy::DefyKeyboard::create_backup
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DefyBackup {
    /// The Neuron's `hardware.chip_id` the backup was captured from.
    #[serde(rename = "neuronID")]
    pub neuron_id: String,
    /// The app-side neuron store entry (names; see [`BackupNeuron`]).
    pub neuron: BackupNeuron,
    /// The firmware versions the device reported.
    #[serde(default)]
    pub versions: BackupVersions,
    /// The captured device state: `{command, data}` pairs in capture
    /// order.
    pub backup: Vec<BackupCommand>,
    /// The partial-capture marker, present only when this file was
    /// saved from a capture that failed partway (see
    /// [`PartialCapture`]). Absent — and omitted when serializing — on
    /// every complete capture, so complete files keep their
    /// Bazecor-identical shape. A foreign file that carries a `partial`
    /// field parses into it, and it round-trips.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partial: Option<PartialCapture>,
}

impl DefyBackup {
    /// Whether this file was saved from a capture that failed partway:
    /// it carries the [`PartialCapture`] marker. Restore validation
    /// refuses such files by default.
    pub fn is_partial(&self) -> bool {
        self.partial.is_some()
    }

    /// The missing commands the partial-capture marker names, or an
    /// empty slice for a complete capture.
    pub fn missing_commands(&self) -> &[String] {
        self.partial
            .as_ref()
            .map(|partial| partial.missing.as_slice())
            .unwrap_or(&[])
    }

    /// Assembles a backup file of the shape a fresh capture produces:
    /// the neuron store entry carries the chip id but no names yet
    /// (names are app-side; there is no store in this crate yet), and
    /// the device descriptor comes from the connected device.
    pub fn capture(
        neuron_id: &str,
        versions: BackupVersions,
        backup: Vec<BackupCommand>,
        device: serde_json::Value,
    ) -> Self {
        Self {
            neuron_id: neuron_id.to_owned(),
            neuron: BackupNeuron {
                id: neuron_id.to_owned(),
                device: Some(device),
                ..BackupNeuron::default()
            },
            versions,
            backup,
            partial: None,
        }
    }

    /// Assembles the backup file a failed capture saves: whatever was
    /// captured before the failure, with the neuron object left at the
    /// empty store shape (no chip id, and no device descriptor — the
    /// probes that fill them never ran) and the [`PartialCapture`]
    /// marker set.
    ///
    /// The marker is set even when `missing` comes out empty — which,
    /// since the identity probes became best-effort, only happens when
    /// a caller assembles it that way (files saved by earlier versions
    /// of this crate carry such markers, from captures interrupted at
    /// the probes): the marker is the file's "this capture failed"
    /// bit regardless of the list's contents.
    pub fn partial_capture(captured: Vec<BackupCommand>, expected: &[String]) -> Self {
        let partial = PartialCapture::from_capture(expected, &captured);

        Self {
            neuron_id: String::new(),
            neuron: BackupNeuron::default(),
            versions: BackupVersions::default(),
            backup: captured,
            partial: Some(partial),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The real `help` response shape of a Defy, from Bazecor's virtual
    /// wireless Defy (`src/api/hardware-virtual/DefyWireless.ts`), with
    /// the line endings the response parser normalizes away already
    /// stripped (this is exactly what
    /// `FocusApiConnection::available_commands` produces).
    const DEFY_HELP_COMMANDS: &[&str] = &[
        "version",
        "keymap.custom",
        "keymap.default",
        "keymap.onlyCustom",
        "settings.defaultLayer",
        "settings.valid?",
        "settings.version",
        "settings.crc",
        "eeprom.contents",
        "eeprom.free",
        "led.at",
        "led.setAll",
        "led.mode",
        "led.brightness",
        "led.theme",
        "palette",
        "colormap.map",
        "idleleds.time_limit",
        "hardware.version",
        "hardware.side_power",
        "hardware.side_ver",
        "hardware.sled_ver",
        "hardware.sled_current",
        "hardware.layout",
        "hardware.joint",
        "hardware.keyscan",
        "hardware.crc_errors",
        "hardware.firmware",
        "hardware.chip_id",
        "qukeys.holdTimeout",
        "qukeys.overlapThreshold",
        "superkeys.map",
        "superkeys.waitfor",
        "superkeys.timeout",
        "superkeys.repeat",
        "superkeys.holdstart",
        "superkeys.overlap",
        "macros.map",
        "macros.trigger",
        "help",
        "mouse.speed",
        "mouse.speedDelay",
        "mouse.accelSpeed",
        "mouse.accelDelay",
        "mouse.wheelSpeed",
        "mouse.wheelDelay",
        "mouse.speedLimit",
        "layer.activate",
        "layer.deactivate",
        "layer.isActive",
        "layer.moveTo",
        "layer.state",
    ];

    fn help_commands(commands: &[&str]) -> Vec<String> {
        commands
            .iter()
            .map(|command| (*command).to_owned())
            .collect()
    }

    #[test]
    fn capture_list_filters_real_help_output_like_bazecor() {
        // The exact expected list: help order, deny-listed commands
        // removed, `led.brightnessUG` force-appended (this help output
        // does not list it — the real Defy backups carry it precisely
        // because of this force-add when the firmware omits it).
        let expected = [
            "keymap.custom",
            "keymap.onlyCustom",
            "settings.defaultLayer",
            "led.brightness",
            "palette",
            "colormap.map",
            "idleleds.time_limit",
            "qukeys.holdTimeout",
            "qukeys.overlapThreshold",
            "superkeys.map",
            "superkeys.waitfor",
            "superkeys.timeout",
            "superkeys.repeat",
            "superkeys.holdstart",
            "superkeys.overlap",
            "macros.map",
            "mouse.speed",
            "mouse.speedDelay",
            "mouse.accelSpeed",
            "mouse.accelDelay",
            "mouse.wheelSpeed",
            "mouse.wheelDelay",
            "mouse.speedLimit",
            "led.brightnessUG",
        ];

        assert_eq!(
            backup_capture_commands(&help_commands(DEFY_HELP_COMMANDS)),
            help_commands(&expected)
        );
    }

    #[test]
    fn capture_list_keeps_listed_forced_command_in_place() {
        // A firmware whose help already lists `led.brightnessUG` (like
        // the real Defy backups show) must not get a duplicate: the
        // command stays at its listed position.
        let mut commands = help_commands(DEFY_HELP_COMMANDS);

        commands.insert(13, "led.brightnessUG".to_owned());

        let capture_list = backup_capture_commands(&commands);
        let forced_position = capture_list
            .iter()
            .position(|command| command == "led.brightnessUG")
            .unwrap();
        let brightness_position = capture_list
            .iter()
            .position(|command| command == "led.brightness")
            .unwrap();

        assert_eq!(forced_position + 1, brightness_position);
        assert_eq!(
            capture_list
                .iter()
                .filter(|command| *command == "led.brightnessUG")
                .count(),
            1
        );
    }

    #[test]
    fn capture_list_drops_empty_lines() {
        let capture_list = backup_capture_commands(&help_commands(&[
            "keymap.custom",
            "",
            "help",
            "superkeys.map",
        ]));

        // `led.brightnessUG` is the force-add at the end.
        assert_eq!(
            capture_list,
            help_commands(&["keymap.custom", "superkeys.map", "led.brightnessUG"])
        );
    }

    #[test]
    fn capture_list_is_substring_semantics_case_sensitive() {
        // Mirrors Bazecor's case-sensitive `includes`: lowercase
        // `layer` denies `layer.activate` but not
        // `settings.defaultLayer` (capital L), and `version` denies
        // `version` but not `settings.defaultLayer`.
        let capture_list = backup_capture_commands(&help_commands(&[
            "settings.defaultLayer",
            "layer.activate",
            "version",
            "keymap.default",
        ]));

        assert_eq!(
            capture_list,
            help_commands(&["settings.defaultLayer", "led.brightnessUG"])
        );
    }

    #[test]
    fn versions_split_a_version_response_like_bazecor() {
        // The real `version` response of Bazecor's virtual wireless
        // Defy, with the firmware's trailing separator space.
        let versions = BackupVersions::from_version_response("v2.2.0 7622bb53 c9d9b7b-dirty ");

        assert_eq!(
            versions,
            BackupVersions {
                bazecor: Some("v2.2.0".to_owned()),
                kaleidoscope: Some("7622bb53".to_owned()),
                firmware: Some("c9d9b7b-dirty".to_owned()),
            }
        );

        // A single-token response (what the shipped Defy backups were
        // captured from) leaves the other tokens out.
        let versions = BackupVersions::from_version_response("v1.1.0");

        assert_eq!(versions.bazecor.as_deref(), Some("v1.1.0"));
        assert_eq!(versions.kaleidoscope, None);
        assert_eq!(versions.firmware, None);

        // No response at all is the empty object.
        assert_eq!(
            BackupVersions::from_version_response(""),
            BackupVersions::default()
        );
    }

    #[test]
    fn versions_omit_missing_tokens_when_serialized() {
        // The shipped Defy backups carry `{"bazecor": "v1.1.0"}` only;
        // the Raise ones all three tokens.
        let versions = BackupVersions::from_version_response("v1.1.0");

        assert_eq!(
            serde_json::to_string(&versions).unwrap(),
            r#"{"bazecor":"v1.1.0"}"#
        );

        let versions = BackupVersions::from_version_response("v1.0.11-beta 89deeffe 8267568-dirty");

        assert_eq!(
            serde_json::to_string(&versions).unwrap(),
            r#"{"bazecor":"v1.0.11-beta","kaleidoscope":"89deeffe","firmware":"8267568-dirty"}"#
        );
    }

    /// A trimmed copy of the shipped Defy wireless backup
    /// (`bazecor/src/defaultBackups/Defy-wireless-backup.json`): all
    /// real values, except that the multi-kilobyte `keymap.custom`,
    /// `palette`, and `colormap.map` data strings are cut down to
    /// their real heads, and the neuron store is shortened to one
    /// entry each.
    const DEFY_WIRELESS_BACKUP_JSON: &str = r#"{
  "neuronID": "8ed7f33db024a5b5",
  "neuron": {
    "id": "8ed7f33db024a5b5",
    "name": "Silver Defy",
    "layers": [
      { "id": 0, "name": "Base" },
      { "id": 1, "name": "Navigation" }
    ],
    "macros": [
      {
        "actions": [
          { "type": 6, "keyCode": 229 },
          { "type": 8, "keyCode": 11 },
          { "type": 7, "keyCode": 229 }
        ],
        "name": "Dygmate",
        "macro": "SHIFT H SHIFT E Y , SPACE SHIFT D SHIFT Y G M A T E",
        "id": 0
      }
    ],
    "superkeys": [
      { "actions": [9, 224, 1, 1, 1], "name": "F", "id": 0 }
    ],
    "device": {
      "info": {
        "vendor": "Dygma",
        "product": "Defy",
        "keyboardType": "wireless",
        "displayName": "Dygma Defy wireless",
        "urls": [{ "name": "Homepage", "url": "https://www.dygma.com/defy/" }]
      },
      "usb": { "vendorId": 13807, "productId": 18 },
      "keyboard": {
        "rows": 5,
        "columns": 16,
        "left": [[0, 1, 2, 3, 4, 5, 6], [16, 17, 18, 19, 20, 21, 22]],
        "right": [[9, 10, 11, 12, 13, 14, 15], [25, 26, 27, 28, 29, 30, 31]],
        "ledsLeft": [0, 1, 2, 3],
        "ledsRight": [35, 36, 37, 38]
      },
      "keyboardUnderglow": {
        "rows": 2,
        "columns": 89,
        "ledsLeft": [70, 71, 72],
        "ledsRight": [123, 124, 125]
      },
      "RGBWMode": true,
      "components": {},
      "instructions": {
        "en": {
          "updateInstructions": "To update the firmware, the keyboard needs a special reset."
        }
      },
      "wireless": true,
      "chipId": "8ed7f33db024a5b5"
    }
  },
  "versions": { "bazecor": "v1.1.0" },
  "backup": [
    { "command": "keymap.custom", "data": "41 30 31 32 33 34 45 0 0 46 35 36 37 38 39 42 43 20 26 8 21 23 47 0 0 48 28 24" },
    { "command": "keymap.onlyCustom", "data": "1" },
    { "command": "settings.defaultLayer", "data": "0" },
    { "command": "led.brightness", "data": "153" },
    { "command": "led.brightnessUG", "data": "255" },
    { "command": "led.brightness.wireless", "data": "125" },
    { "command": "led.brightnessUG.wireless", "data": "125" },
    { "command": "palette", "data": "255 196 0 0 0 254 24 0 0 0 0 255 231 255 0 0" },
    { "command": "colormap.map", "data": "10 4 4 4 4 4 7 10 4 4 4 4 4 7 2 4 4 4 4 4 0 2 4 4 4 4 4 10 3" },
    { "command": "idleleds.time_limit", "data": "300" },
    { "command": "idleleds.wireless", "data": "300" },
    { "command": "qukeys.holdTimeout", "data": "250" },
    { "command": "qukeys.overlapThreshold", "data": "80" },
    { "command": "superkeys.map", "data": "262 281 2329 283 1 0 0" },
    { "command": "superkeys.waitfor", "data": "500" },
    { "command": "superkeys.timeout", "data": "250" },
    { "command": "superkeys.repeat", "data": "20" },
    { "command": "superkeys.holdstart", "data": "200" },
    { "command": "superkeys.overlap", "data": "20" },
    { "command": "macros.map", "data": "6 229 8 11 7 229 8 8 8 28 8 54 8 44 6 229 8 7 7 229 8 28 8 10 8 16 8 4 8 23 8 8 0 0" },
    { "command": "mouse.speed", "data": "20" },
    { "command": "mouse.speedDelay", "data": "2" },
    { "command": "mouse.accelSpeed", "data": "20" },
    { "command": "mouse.accelDelay", "data": "2" },
    { "command": "mouse.wheelSpeed", "data": "2" },
    { "command": "mouse.wheelDelay", "data": "100" },
    { "command": "mouse.speedLimit", "data": "127" },
    { "command": "wireless.battery.savingMode", "data": "0" },
    { "command": "wireless.bluetooth.macs", "data": "" },
    { "command": "wireless.bluetooth.peer_ids", "data": "" },
    { "command": "wireless.bluetooth.remove", "data": "" },
    { "command": "wireless.bluetooth.deviceName", "data": "" },
    { "command": "wireless.bluetooth.list", "data": "" },
    { "command": "wireless.rf.power", "data": "0" },
    { "command": "wireless.rf.channelHop", "data": "0" },
    { "command": "wireless.rf.syncPairing", "data": "" }
  ]
}"#;

    /// A trimmed copy of the shipped Sonsei backup
    /// (`bazecor/src/defaultBackups/Sonsei.json`), kept as a second
    /// interop fixture because it carries two quirks the Defy files do
    /// not: a captured `{"command": "", "data": ""}` entry (Bazecor
    /// captures empty help lines), and qukeys settings the Defy files
    /// predate. Real values, large data strings cut to their heads,
    /// neuron store shortened.
    const SONSEI_BACKUP_JSON: &str = r#"{
  "neuronID": "7f6a7facc35abff3",
  "neuron": {
    "id": "7f6a7facc35abff3",
    "name": "Dygman's Sonsei",
    "layers": [
      { "id": 0, "name": "Base" },
      { "id": 1, "name": "Nav&Symbols" }
    ],
    "macros": [],
    "superkeys": [
      { "actions": [42, 17452, 1, 1, 1], "name": "⌫/L3", "id": 0 },
      { "actions": [57, 17452, 0, 0, 0], "name": "⏏️/L3", "id": 3, "superkey": "" }
    ],
    "device": {
      "info": {
        "vendor": "Dygma",
        "product": "Sonsei",
        "keyboardType": "wireless",
        "displayName": "Dygma Sonsei",
        "urls": [{ "name": "Homepage", "url": "https://www.dygma.com/sonsei/" }]
      },
      "usb": { "vendorId": 13807, "productId": 49 },
      "keyboard": { "rows": 5, "columns": 12 },
      "RGBWMode": false,
      "components": {},
      "wireless": true,
      "chipId": "7f6a7facc35abff3"
    }
  },
  "versions": { "bazecor": "v1.0.0-beta.2" },
  "backup": [
    { "command": "keymap.custom", "data": "41 30 31 32 33 34 35 36 37 38 39 42 43 20 26 8 21 23 28 24 1" },
    { "command": "keymap.onlyCustom", "data": "1" },
    { "command": "qukeys.holdTimeout", "data": "250" },
    { "command": "qukeys.overlapThreshold", "data": "70" },
    { "command": "qukeys.minimumHoldTime", "data": "71" },
    { "command": "qukeys.minimumPriorInterval", "data": "75" },
    { "command": "superkeys.map", "data": "42 17452 1 1 1 0 40 17451 1 1 1 0 17154 230 1 1 1 0 57 17452 1 1 1 0 0" },
    { "command": "macros.map", "data": "6 225 8 11 7 225 8 8 8 15 8 15 8 18 0 6 225 8 11 7 225 8 18 8 15 8 4 0 0" },
    { "command": "led.brightness", "data": "102" },
    { "command": "led.brightnessUG", "data": "178" },
    { "command": "led.brightness.wireless", "data": "76" },
    { "command": "led.fade", "data": "1 0" },
    { "command": "", "data": "" },
    { "command": "idleleds.true_sleep", "data": "0" },
    { "command": "idleleds.true_sleep_time", "data": "60" },
    { "command": "palette", "data": "255 255 255 255 0 0 255 255 0 255 108 0" },
    { "command": "colormap.map", "data": "4 0 0 0 0 0 0 0 0 0 0 3 4 0 0 0 0 0 0 0 0 0 0 5 4 0 0 0 0 0" }
  ]
}"#;

    /// The real device descriptor object from the shipped Defy wireless
    /// backup, for comparing parsed-and-reserialized backups against
    /// the original JSON byte for byte.
    fn defy_wireless_device_value() -> serde_json::Value {
        serde_json::from_str::<DefyBackup>(DEFY_WIRELESS_BACKUP_JSON)
            .unwrap()
            .neuron
            .device
            .unwrap()
    }

    #[test]
    fn real_shaped_defy_backup_parses_and_round_trips() {
        let backup = serde_json::from_str::<DefyBackup>(DEFY_WIRELESS_BACKUP_JSON).unwrap();

        // The load-bearing fields, exactly as in the shipped file.
        assert_eq!(backup.neuron_id, "8ed7f33db024a5b5");
        assert_eq!(backup.neuron.id, "8ed7f33db024a5b5");
        assert_eq!(backup.neuron.name, "Silver Defy");
        assert_eq!(
            backup.versions,
            BackupVersions {
                bazecor: Some("v1.1.0".to_owned()),
                kaleidoscope: None,
                firmware: None,
            }
        );
        assert_eq!(backup.backup.len(), 36);

        // Backup entries store live bytes only: the superkey map ends
        // `... 0 0` with no padding and no trailing space.
        let superkeys = backup
            .backup
            .iter()
            .find(|entry| entry.command == "superkeys.map")
            .unwrap();

        assert_eq!(superkeys.data, "262 281 2329 283 1 0 0");

        // The device descriptor survives verbatim, nested fields and all.
        let original: serde_json::Value = serde_json::from_str(DEFY_WIRELESS_BACKUP_JSON).unwrap();

        assert_eq!(
            backup.neuron.device.as_ref().unwrap(),
            &original["neuron"]["device"]
        );
        assert_eq!(backup.neuron.layers.len(), 2);
        assert_eq!(backup.neuron.layers[0]["name"], "Base");

        // Serialize -> parse -> equal: the round trip is lossless.
        let json = serde_json::to_string_pretty(&backup).unwrap();
        let reparsed = serde_json::from_str::<DefyBackup>(&json).unwrap();

        assert_eq!(reparsed, backup);
    }

    #[test]
    fn real_shaped_sonsei_backup_parses_with_its_quirks() {
        let backup = serde_json::from_str::<DefyBackup>(SONSEI_BACKUP_JSON).unwrap();

        assert_eq!(backup.neuron_id, "7f6a7facc35abff3");
        assert_eq!(backup.backup.len(), 17);

        // The captured empty command Bazecor's shipped Sonsei backup
        // carries parses and round-trips instead of failing the file.
        let empty = backup
            .backup
            .iter()
            .find(|entry| entry.command.is_empty())
            .unwrap();

        assert_eq!(empty.data, "");

        // The superkey store entry with the extra `superkey` field is
        // preserved verbatim (opaque store values, not typed away).
        let json = serde_json::to_string(&backup).unwrap();
        let reparsed = serde_json::from_str::<DefyBackup>(&json).unwrap();

        assert_eq!(reparsed, backup);
        assert!(json.contains(r#""superkey":""#));
    }

    #[test]
    fn every_command_a_real_backup_captures_passes_the_deny_filter() {
        // Cross-check the deny list against the capture Bazecor
        // actually shipped: every entry of the shipped Defy wireless
        // backup is what our filter keeps, and none of them contains a
        // denied substring (except the `led.brightnessUG` force-add,
        // which is not deny-listed).
        let backup = serde_json::from_str::<DefyBackup>(DEFY_WIRELESS_BACKUP_JSON).unwrap();

        for entry in &backup.backup {
            let denied = BACKUP_COMMAND_DENY_LIST
                .iter()
                .find(|denied| entry.command.contains(*denied));

            assert_eq!(denied, None, "deny list would drop `{}`", entry.command);
        }

        // And the reverse: feeding the captured command names through
        // the filter keeps them all, in order, with the forced command
        // already present.
        let commands: Vec<String> = backup
            .backup
            .iter()
            .map(|entry| entry.command.clone())
            .collect();
        let capture_list = backup_capture_commands(&commands);

        assert_eq!(capture_list, commands);
    }

    #[test]
    fn capture_shape_carries_empty_names_and_the_device_descriptor() {
        let versions = BackupVersions::from_version_response("v2.2.0 7622bb53 c9d9b7b-dirty");
        let backup = DefyBackup::capture(
            "8ed7f33db024a5b5",
            versions,
            vec![BackupCommand::new(
                "keymap.custom".to_owned(),
                "41 30 31".to_owned(),
            )],
            defy_wireless_device_value(),
        );

        // The neuron store entry matches what Bazecor's DoBackup
        // fallback writes for an unknown neuron: empty name, empty
        // arrays — plus the chip id and the device descriptor.
        assert_eq!(backup.neuron_id, "8ed7f33db024a5b5");
        assert_eq!(backup.neuron.id, "8ed7f33db024a5b5");
        assert_eq!(backup.neuron.name, "");
        assert!(backup.neuron.layers.is_empty());
        assert!(backup.neuron.macros.is_empty());
        assert!(backup.neuron.superkeys.is_empty());
        assert_eq!(
            backup.neuron.device.as_ref().unwrap(),
            &defy_wireless_device_value()
        );

        let json = serde_json::to_value(&backup).unwrap();

        // The serialized shape matches the real files': every neuron
        // key Bazecor writes is present, in a form its restore flow
        // reads (`neuron.device.info.product`).
        assert_eq!(json["neuronID"], "8ed7f33db024a5b5");
        assert_eq!(json["neuron"]["name"], "");
        assert_eq!(json["neuron"]["layers"], serde_json::json!([]));
        assert_eq!(json["neuron"]["device"]["info"]["product"], "Defy");
        assert_eq!(json["versions"]["bazecor"], "v2.2.0");
        assert_eq!(json["backup"][0]["command"], "keymap.custom");
        assert_eq!(json["backup"][0]["data"], "41 30 31");

        let reparsed = serde_json::from_value::<DefyBackup>(json).unwrap();

        assert_eq!(reparsed, backup);
    }

    #[test]
    fn foreign_backup_shapes_are_rejected() {
        // Bazecor's virtual-keyboard files (the Raise2 examples) are a
        // different, phase-2 format: no `backup` array, so they are not
        // backups and must not parse as one.
        let virtual_keyboard = r#"{
            "device": null,
            "virtual": { "keymap.custom": { "data": "41", "eraseable": true } }
        }"#;

        assert!(serde_json::from_str::<DefyBackup>(virtual_keyboard).is_err());

        // A file missing the neuron object is not a backup either.
        assert!(serde_json::from_str::<DefyBackup>(r#"{ "backup": [] }"#).is_err());
    }

    /// A real-shaped capture in the shipped Defy wireless backup's
    /// order: `keymap.custom` first, `superkeys.map` in the middle —
    /// the shape Bazecor's reorder exists for.
    fn defy_capture_entries() -> Vec<BackupCommand> {
        [
            ("keymap.custom", "41 30 31"),
            ("keymap.onlyCustom", "1"),
            ("settings.defaultLayer", "0"),
            ("led.brightness", "153"),
            ("palette", "255 196 0 0"),
            ("colormap.map", "10 4 4 4"),
            ("superkeys.map", "262 281 2329 283 1 0 0"),
            ("superkeys.waitfor", "500"),
            ("macros.map", "6 229 8 11 0 0"),
            ("mouse.speed", "20"),
        ]
        .iter()
        .map(|(command, data)| BackupCommand::new((*command).to_owned(), (*data).to_owned()))
        .collect()
    }

    #[test]
    fn restore_sequence_moves_superkeys_before_keymap_like_bazecor() {
        let sequence = restore_command_sequence(&defy_capture_entries());

        let commands: Vec<&str> = sequence
            .iter()
            .map(|entry| entry.command.as_str())
            .collect();

        // `superkeys.map` lands directly before `keymap.custom`; every
        // other entry keeps its relative order; nothing is lost.
        assert_eq!(
            commands,
            [
                "superkeys.map",
                "keymap.custom",
                "keymap.onlyCustom",
                "settings.defaultLayer",
                "led.brightness",
                "palette",
                "colormap.map",
                "superkeys.waitfor",
                "macros.map",
                "mouse.speed",
            ]
        );

        // The moved entry's data travels with it.
        let superkeys = sequence.first().unwrap();

        assert_eq!(superkeys.data, "262 281 2329 283 1 0 0");
    }

    #[test]
    fn restore_sequence_keeps_entries_already_ordered() {
        // Bazecor's reorder only touches `superkeys.map` entries that
        // come *after* `keymap.custom`; one already before it stays put.
        let sequence = restore_command_sequence(&[
            BackupCommand::new("superkeys.map".to_owned(), "262 281 1 0 0".to_owned()),
            BackupCommand::new("keymap.custom".to_owned(), "41 30 31".to_owned()),
            BackupCommand::new("macros.map".to_owned(), "6 229 0 0".to_owned()),
        ]);

        let commands: Vec<&str> = sequence
            .iter()
            .map(|entry| entry.command.as_str())
            .collect();

        assert_eq!(commands, ["superkeys.map", "keymap.custom", "macros.map"]);
    }

    #[test]
    fn restore_sequence_without_keymap_custom_is_unchanged() {
        let entries = [
            BackupCommand::new("macros.map".to_owned(), "6 229 0 0".to_owned()),
            BackupCommand::new("superkeys.map".to_owned(), "262 281 1 0 0".to_owned()),
            BackupCommand::new("led.brightness".to_owned(), "153".to_owned()),
        ];

        assert_eq!(restore_command_sequence(&entries), entries);
    }

    #[test]
    fn restore_sequence_moves_every_superkeys_entry_keeping_their_order() {
        // Two `superkeys.map` entries after the keymap (a foreign-file
        // oddity, but exactly what Bazecor's loop collects) both move,
        // in their own relative order, directly before the keymap.
        let sequence = restore_command_sequence(&[
            BackupCommand::new("keymap.custom".to_owned(), "41 30 31".to_owned()),
            BackupCommand::new("led.brightness".to_owned(), "153".to_owned()),
            BackupCommand::new("superkeys.map".to_owned(), "1 1 0 0".to_owned()),
            BackupCommand::new("macros.map".to_owned(), "6 229 0 0".to_owned()),
            BackupCommand::new("superkeys.map".to_owned(), "2 2 0 0".to_owned()),
        ]);

        let commands: Vec<&str> = sequence
            .iter()
            .map(|entry| entry.command.as_str())
            .collect();

        assert_eq!(
            commands,
            [
                "superkeys.map",
                "superkeys.map",
                "keymap.custom",
                "led.brightness",
                "macros.map",
            ]
        );

        // Their relative order survived.
        assert_eq!(sequence[0].data, "1 1 0 0");
        assert_eq!(sequence[1].data, "2 2 0 0");
    }

    #[test]
    fn restore_sequence_superkeys_match_is_bazecors_regular_expression() {
        // `^superkeys?\.map$`, case-insensitive: singular `superkey`
        // and any case match; anything with extra characters does not.
        let sequence = restore_command_sequence(&[
            BackupCommand::new("keymap.custom".to_owned(), "41 30 31".to_owned()),
            BackupCommand::new("SUPERKEYS.MAP".to_owned(), "1 1 0 0".to_owned()),
            BackupCommand::new("Superkey.Map".to_owned(), "2 2 0 0".to_owned()),
            BackupCommand::new("xsuperkeys.map".to_owned(), "nope".to_owned()),
            BackupCommand::new("superkeys.map2".to_owned(), "nope".to_owned()),
        ]);

        let commands: Vec<&str> = sequence
            .iter()
            .map(|entry| entry.command.as_str())
            .collect();

        // The regex matches moved; the near-misses stayed after the
        // keymap.
        assert_eq!(
            commands,
            [
                "SUPERKEYS.MAP",
                "Superkey.Map",
                "keymap.custom",
                "xsuperkeys.map",
                "superkeys.map2",
            ]
        );
    }

    #[test]
    fn restore_sequence_drops_empty_commands_but_keeps_empty_data() {
        // The shipped Sonsei backup's `{"command": "", "data": ""}`
        // artifact carries no state and cannot be sent; an entry with
        // a real command but no data is a read and stays.
        let sequence = restore_command_sequence(&[
            BackupCommand::new("keymap.custom".to_owned(), "41 30 31".to_owned()),
            BackupCommand::new(String::new(), String::new()),
            BackupCommand::new("wireless.bluetooth.macs".to_owned(), String::new()),
        ]);

        let commands: Vec<&str> = sequence
            .iter()
            .map(|entry| entry.command.as_str())
            .collect();

        assert_eq!(commands, ["keymap.custom", "wireless.bluetooth.macs"]);
    }

    #[test]
    fn has_names_distinguishes_the_empty_store_shape_from_a_named_neuron() {
        // Exactly the neuron object phase-1 captures carry: the empty
        // store shape.
        assert!(!BackupNeuron::default().has_names());

        // Any one of the name carriers counts.
        assert!(
            BackupNeuron {
                name: "Silver Defy".to_owned(),
                ..BackupNeuron::default()
            }
            .has_names()
        );

        assert!(
            BackupNeuron {
                layers: vec![serde_json::json!({ "id": 0, "name": "Base" })],
                ..BackupNeuron::default()
            }
            .has_names()
        );

        assert!(
            BackupNeuron {
                superkeys: vec![serde_json::json!({ "id": 0, "name": "F" })],
                ..BackupNeuron::default()
            }
            .has_names()
        );
    }

    fn capture_list(commands: &[&str]) -> Vec<String> {
        commands
            .iter()
            .map(|command| (*command).to_owned())
            .collect()
    }

    fn captured_entries(commands: &[&str]) -> Vec<BackupCommand> {
        commands
            .iter()
            .map(|command| BackupCommand::new((*command).to_owned(), String::new()))
            .collect()
    }

    #[test]
    fn missing_commands_are_the_expected_ones_not_captured() {
        let expected = capture_list(&["keymap.custom", "palette", "superkeys.map", "macros.map"]);

        // Everything captured: nothing missing.
        assert_eq!(
            missing_capture_commands(
                &expected,
                &captured_entries(&["keymap.custom", "palette", "superkeys.map", "macros.map",])
            ),
            Vec::<String>::new()
        );

        // The capture stopped after `palette`: the rest is missing, in
        // expected order.
        assert_eq!(
            missing_capture_commands(&expected, &captured_entries(&["keymap.custom", "palette"])),
            capture_list(&["superkeys.map", "macros.map"])
        );

        // Nothing captured at all: the whole list is missing.
        assert_eq!(missing_capture_commands(&expected, &[]), expected);

        // A foreign entry the expected list never mentions counts
        // against nothing and is not reported missing.
        assert_eq!(
            missing_capture_commands(
                &expected,
                &captured_entries(&["keymap.custom", "wireless.rf.power"])
            ),
            capture_list(&["palette", "superkeys.map", "macros.map"])
        );
    }

    #[test]
    fn missing_commands_are_counted_per_expected_occurrence() {
        // A command `help` lists twice is captured twice (Bazecor's
        // behavior), so one captured occurrence satisfies only one
        // expected one.
        let expected = capture_list(&["led.fade", "led.fade", "palette"]);

        assert_eq!(
            missing_capture_commands(&expected, &captured_entries(&["led.fade"])),
            capture_list(&["led.fade", "palette"])
        );

        assert_eq!(
            missing_capture_commands(&expected, &captured_entries(&["led.fade", "led.fade"])),
            capture_list(&["palette"])
        );
    }

    #[test]
    fn partial_marker_serializes_only_when_present() {
        // A complete capture — the shape every capture produced before
        // partial recovery existed — serializes without any `partial`
        // field, byte-identical to the older shape: Bazecor must not
        // see a marker on complete files.
        let backup = DefyBackup::capture(
            "8ed7f33db024a5b5",
            BackupVersions::default(),
            captured_entries(&["keymap.custom"]),
            defy_wireless_device_value(),
        );

        let json = serde_json::to_string(&backup).unwrap();

        assert!(!json.contains(r#""partial""#));

        // And the partial file's marker round-trips: parse, serialize,
        // parse — the missing list survives exactly.
        let partial = serde_json::from_str::<DefyBackup>(
            r#"{
                "neuronID": "8ed7f33db024a5b5",
                "neuron": {},
                "versions": {},
                "backup": [
                    { "command": "keymap.custom", "data": "41 30 31" }
                ],
                "partial": { "missing": ["palette", "colormap.map"] }
            }"#,
        )
        .unwrap();

        assert_eq!(
            partial.partial,
            Some(PartialCapture {
                missing: capture_list(&["palette", "colormap.map"]),
            })
        );
        assert!(partial.is_partial());
        assert_eq!(partial.missing_commands(), ["palette", "colormap.map"]);

        let json = serde_json::to_string(&partial).unwrap();
        let reparsed = serde_json::from_str::<DefyBackup>(&json).unwrap();

        assert_eq!(reparsed, partial);
    }

    #[test]
    fn partial_marker_parses_without_a_missing_list() {
        // `"partial": {}` is the marker of a capture that stopped
        // after every command but before the identity probes.
        let partial = serde_json::from_str::<DefyBackup>(
            r#"{
                "neuronID": "",
                "neuron": {},
                "versions": {},
                "backup": [],
                "partial": {}
            }"#,
        )
        .unwrap();

        assert_eq!(
            partial.partial,
            Some(PartialCapture {
                missing: Vec::new(),
            })
        );
        assert!(partial.is_partial());
        assert!(!partial.partial.as_ref().unwrap().has_missing());
    }

    #[test]
    fn partial_capture_assembles_the_empty_store_shape_and_the_marker() {
        let expected = capture_list(&["keymap.custom", "palette", "superkeys.map"]);
        let captured = captured_entries(&["keymap.custom"]);

        let partial = DefyBackup::partial_capture(captured.clone(), &expected);

        // The neuron object is the empty store shape: no chip id, and
        // no device descriptor — the probes that fill them never ran.
        assert_eq!(partial.neuron_id, "");
        assert_eq!(partial.neuron, BackupNeuron::default());
        assert_eq!(partial.versions, BackupVersions::default());
        assert_eq!(partial.backup, captured);
        assert_eq!(
            partial.partial,
            Some(PartialCapture {
                missing: capture_list(&["palette", "superkeys.map"]),
            })
        );

        // Serialized, the marker is the only field a Bazecor file does
        // not carry. (serde_json object keys iterate sorted; compare
        // as a set.)
        let json = serde_json::to_value(&partial).unwrap();

        assert_eq!(
            json["partial"],
            serde_json::json!({ "missing": ["palette", "superkeys.map"] })
        );

        let mut keys = json
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>();

        keys.sort();

        assert_eq!(
            keys,
            ["backup", "neuron", "neuronID", "partial", "versions"]
        );
    }

    #[test]
    fn partial_capture_after_every_command_still_carries_the_marker() {
        // The capture stopped at the identity probes: every command is
        // in the file, but the file is not a complete capture's
        // product. The marker serializes as `{}` — present, unmistakable.
        let expected = capture_list(&["keymap.custom", "palette"]);

        let partial =
            DefyBackup::partial_capture(captured_entries(&["keymap.custom", "palette"]), &expected);

        assert_eq!(
            partial.partial,
            Some(PartialCapture {
                missing: Vec::new(),
            })
        );

        assert_eq!(
            serde_json::to_value(&partial).unwrap()["partial"],
            serde_json::json!({})
        );
    }
}
