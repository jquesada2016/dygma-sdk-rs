// Differential-recording harness for the WIRELESS & BATTERY commands
// (WS-D): every `wireless.*` response shape Bazecor's own data sources
// carry, machine-recorded.
//
// Runs none of Bazecor's logic — the wireless responses are DATA — but
// loads Bazecor's OWN hardware-virtual keyboard definitions (the
// read-only shallow clone at ../bazecor, or $BAZECOR_SRC) verbatim and
// reads the shipped default backups (already committed as fixtures/
// copies, byte-identity verified), recording every `wireless.*` command
// entry into expectations/wireless.json. The Rust wireless types' tests
// then check their parsers against these recorded responses, so the
// WS-D wire shapes are validated against the app's actual data, not a
// transcription.
//
// What loads from Bazecor, verbatim (data definitions, no logic):
//   - src/api/hardware-virtual/{DefyWireless,SonseiWireless,Raise2ANSI,Raise2ISO}.ts
//   - src/defaultBackups/{Defy-wired,Defy-wireless,Sonsei,Raise2ANSI,Raise2ISO}.json
//
// (The Raise2 pair of sources belongs to the same evidence set: the
// Raise2 virtual keyboards are served by the same VirtualSelector
// mechanism as the Defy's, and the Raise2ANSI/Raise2ISO.json files sit
// in the same defaultBackups/ directory.)
//
// What is checked here (the evidence claims the SDK's wireless module
// stands on):
//   - where a virtual keyboard and its product's shipped backups BOTH
//     carry a wireless command, their responses agree — except
//     `wireless.rf.channelHop`, a recorded outlier (the virtual says
//     `false`, the real devices' backups say `0`), noted, not equated;
//   - the Sonsei pair agrees on the one wireless command it carries
//     (`wireless.battery.savingMode`);
//   - `wireless.battery.forceRead` and the `infoChannel` reads are
//     recorded AS ABSENT from every source — the notes say so, so the
//     SDK's thinner evidence for those stays pinned.
//
// Usage:  node tools/bazecor-differential/record-wireless.mjs
// Never modifies the Bazecor clone; reads it, and writes only under
// tools/bazecor-differential/expectations/.

import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { register } from "node:module";

const here = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(here, "..", "..");
const bazecorSrc = process.env.BAZECOR_SRC ?? path.resolve(repoRoot, "..", "bazecor");
const fixturesDir = path.join(here, "fixtures");
const expectationsDir = path.join(here, "expectations");

const backupPath = (name) => path.join(bazecorSrc, "src", "defaultBackups", name);
const virtualPath = (name) =>
  path.join(bazecorSrc, "src", "api", "hardware-virtual", `${name}.ts`);

for (const required of [
  virtualPath("DefyWireless"),
  backupPath("Defy-wireless-backup.json"),
  backupPath("Raise2ANSI.json"),
]) {
  if (!fs.existsSync(required)) {
    console.error(`Bazecor clone not found at ${bazecorSrc} (set BAZECOR_SRC)`);
    process.exit(1);
  }
}

const bazecorCommit = execFileSync("git", ["-C", bazecorSrc, "rev-parse", "HEAD"])
  .toString()
  .trim();

register("./hooks.mjs", import.meta.url);

// The WS-D scope (issue #4). Everything else the sources carry under
// `wireless.*` is recorded too — the full evidence map — but only
// these commands are parsed by the SDK tests.
const SCOPE_COMMANDS = [
  "wireless.battery.left.level",
  "wireless.battery.right.level",
  "wireless.battery.left.status",
  "wireless.battery.right.status",
  "wireless.battery.savingMode",
  "wireless.battery.forceRead",
  "wireless.rf.power",
  "wireless.rf.channelHop",
  "wireless.rf.syncPairing",
  "wireless.bluetooth.deviceName",
  "wireless.bluetooth.infoChannel 1",
  "wireless.bluetooth.infoChannel 2",
  "wireless.bluetooth.infoChannel 3",
  "wireless.bluetooth.infoChannel 4",
  "wireless.bluetooth.infoChannel 5",
];

// Where a KNOWN outlier is tolerated instead of equated: the virtual
// keyboards' `channelHop` is a bool word (`false`) where the real
// devices' backup captures are decimals (`0`) — both parse to the same
// Off, and the disagreement is a note, not a failure.
const KNOWN_OUTLIERS = new Set(["wireless.rf.channelHop"]);

const virtuals = {};
for (const name of ["DefyWireless", "SonseiWireless", "Raise2ANSI", "Raise2ISO"]) {
  virtuals[name] = (
    await import(pathToFileURL(virtualPath(name)).href)
  ).default;
}

// The committed fixture copies must stay byte-identical to the shipped
// files they claim to snapshot (same rule as record.mjs).
for (const fixture of [
  "Defy-wireless-backup.json",
  "Defy-wired-backup.json",
  "Sonsei.json",
  "Raise2ANSI.json",
  "Raise2ISO.json",
]) {
  const shipped = fs.readFileSync(backupPath(fixture));
  const copy = fs.readFileSync(path.join(fixturesDir, fixture));

  if (!shipped.equals(copy)) {
    throw new Error(
      `${fixture} in fixtures/ has drifted from the shipped file; re-copy it`,
    );
  }
}

/** Every `wireless.*` entry of a virtual-keyboard command map, command → data. */
function virtualMapWirelessEntries(virtualMap) {
  return Object.fromEntries(
    Object.entries(virtualMap)
      .filter(([command]) => command.startsWith("wireless."))
      .map(([command, entry]) => [command, entry.data]),
  );
}

/** Every `wireless.*` entry of a virtual keyboard, command → data. */
function virtualWirelessEntries(vk) {
  return virtualMapWirelessEntries(vk.virtual);
}

/** Every `wireless.*` entry of a backup-array backup, command → data. */
function backupWirelessEntries(backup) {
  return Object.fromEntries(
    backup.backup
      .filter((entry) => entry.command.startsWith("wireless."))
      .map((entry) => [entry.command, entry.data]),
  );
}

/** Whitespace-normalized form, for data-level (token) comparison. */
const tokens = (data) => data?.trim().split(/\s+/).join(" ") ?? null;

const readFixtureJson = (name) =>
  JSON.parse(fs.readFileSync(path.join(fixturesDir, name), "utf-8"));

const sources = {
  "virtual DefyWireless": virtualWirelessEntries(virtuals.DefyWireless),
  "virtual SonseiWireless": virtualWirelessEntries(virtuals.SonseiWireless),
  "virtual Raise2ANSI": virtualWirelessEntries(virtuals.Raise2ANSI),
  "virtual Raise2ISO": virtualWirelessEntries(virtuals.Raise2ISO),
  "backup Defy-wireless": backupWirelessEntries(readFixtureJson("Defy-wireless-backup.json")),
  "backup Defy-wired": backupWirelessEntries(readFixtureJson("Defy-wired-backup.json")),
  "backup Sonsei": backupWirelessEntries(readFixtureJson("Sonsei.json")),
  // The shipped Raise2 default backups carry a virtual-keyboard-shaped
  // command map (a `virtual` object), not the Defy backups' `backup`
  // array — so they read out through the same map extractor.
  "backup Raise2ANSI": virtualMapWirelessEntries(readFixtureJson("Raise2ANSI.json").virtual),
  "backup Raise2ISO": virtualMapWirelessEntries(readFixtureJson("Raise2ISO.json").virtual),
};

const notes = [];

// Machine-check the cross-source agreement claims, Defy pairs first:
// where a virtual keyboard and a shipped backup of the same product
// both carry a command, their responses agree — unless the command is
// a known outlier, whose exact disagreement is noted, not asserted
// (the Rust tests are what assert both forms parse).
const productPairs = [
  ["virtual DefyWireless", "backup Defy-wireless"],
  ["virtual DefyWireless", "backup Defy-wired"],
  ["virtual Raise2ANSI", "backup Raise2ANSI"],
  ["virtual Raise2ISO", "backup Raise2ISO"],
];

for (const [virtualSource, backupSource] of productPairs) {
  const virtualEntries = sources[virtualSource];
  const backupEntries = sources[backupSource];

  for (const command of Object.keys(backupEntries)) {
    if (!(command in virtualEntries)) {
      notes.push(
        `${backupSource} carries \`${command}\` but ${virtualSource} does not record it`,
      );
      continue;
    }

    if (tokens(virtualEntries[command]) === tokens(backupEntries[command])) {
      continue;
    }

    if (KNOWN_OUTLIERS.has(command)) {
      notes.push(
        `KNOWN OUTLIER: ${virtualSource} answers \`${virtualEntries[command]}\` ` +
          `where ${backupSource} captures \`${backupEntries[command]}\` for ` +
          `\`${command}\` — the SDK reads both forms`,
      );
      continue;
    }

    throw new Error(
      `${virtualSource} and ${backupSource} disagree on \`${command}\` ` +
        `(\`${virtualEntries[command]}\` vs \`${backupEntries[command]}\`) — ` +
        `the wireless shapes need re-adjudication before the SDK can ` +
        `keep citing both sources`,
    );
  }
}

// The Sonsei pair: the one wireless command the Sonsei sources carry
// (`savingMode`) must agree.
if (
  tokens(sources["virtual SonseiWireless"]["wireless.battery.savingMode"]) !==
  tokens(sources["backup Sonsei"]["wireless.battery.savingMode"])
) {
  throw new Error(
    "the Sonsei virtual keyboard and the shipped Sonsei backup disagree on " +
      "wireless.battery.savingMode — the wireless shapes need re-adjudication",
  );
}

// Pin the ABSENCES that bound the SDK's evidence: the scope commands
// no source records are the ones the SDK documents as thin.
const recordedEverywhere = new Set(
  Object.values(sources).flatMap((entries) => Object.keys(entries)),
);
for (const command of SCOPE_COMMANDS) {
  if (!recordedEverywhere.has(command)) {
    notes.push(
      `\`${command}\` is recorded by NO source — no capture pins its ` +
        `response shape, and the SDK's docs claim none (they cite only ` +
        `the evidence each command actually has)`,
    );
  }
}

fs.mkdirSync(expectationsDir, { recursive: true });

const expectations = {
  bazecor_commit: bazecorCommit,
  scope_commands: SCOPE_COMMANDS,
  sources,
  notes,
};

const outPath = path.join(expectationsDir, "wireless.json");
fs.writeFileSync(outPath, `${JSON.stringify(expectations, null, 2)}\n`);

console.log(`Recorded wireless expectations at Bazecor ${bazecorCommit}`);
console.log(`  -> ${outPath}`);
for (const note of notes) {
  console.log(`  note: ${note}`);
}
