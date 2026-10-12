// Differential-recording harness for the DEFAULT CONFIGS (WS-E phase 3,
// `config init`): runs none of Bazecor's logic — the defaults are
// DATA — but loads Bazecor's OWN hardware-virtual keyboard definitions
// (the read-only shallow clone at ../bazecor, or $BAZECOR_SRC)
// verbatim, records the default configuration each product's virtual
// keyboard carries into expectations/default-configs.json, and — the
// load-bearing part — machine-checks the evidence claim the SDK's
// defaults module stands on: that Bazecor's two independent sources of
// the defaults, the virtual keyboards and the shipped default backups,
// agree. If they ever drift, this script fails and the defaults need
// re-adjudicating against the ground-truth hierarchy before an SDK
// default silently changes.
//
// What loads from Bazecor, verbatim (data definitions, no logic):
//   - src/api/hardware-virtual/{DefyWired,DefyWireless,SonseiWireless,
//     RaiseANSI,RaiseISO,Raise2ANSI,Raise2ISO}.ts
//
// What is checked here against src/defaultBackups/*.json (already
// committed as fixtures/ copies, byte-identity verified): the Defy
// virtual keyboards' keymap.custom/superkeys.map/macros.map/palette/
// colormap.map equal the shipped Defy wireless backup's captures, and
// the Sonsei virtual keyboard's equal the shipped Sonsei backup's.
// (The shipped Defy WIRED backup is the known outlier: its keymap
// layer 0 carries `0 0` at wire positions 72–73 — right-thumb-cluster
// keys — where the virtual keyboards and the wireless backup carry
// `54109 54108`, the bluetooth-pairing and battery-status keys, so it
// is recorded as a fixture note, not an equality check.)
//
// Usage:  node tools/bazecor-differential/record-defaults.mjs
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

const VIRTUAL_KEYBOARDS = [
  "DefyWired",
  "DefyWireless",
  "SonseiWireless",
  "RaiseANSI",
  "RaiseISO",
  "Raise2ANSI",
  "Raise2ISO",
];

for (const required of [
  virtualPath("DefyWireless"),
  backupPath("Defy-wireless-backup.json"),
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

// The five captured commands whose defaults a `config init` reproduces,
// plus the per-product identity the descriptor carries.
const CONFIG_COMMANDS = [
  "keymap.custom",
  "superkeys.map",
  "macros.map",
  "palette",
  "colormap.map",
];

// Settings the virtual keyboards carry values for — recorded for the
// evidence map (they are NOT in shipped backups: backups capture no
// settings). The evidence class is thinner here: the virtual keyboards
// are Bazecor's simulation of a factory-state device, with nothing to
// cross-check against. Recorded, labeled, not replicated as SDK file
// output — the SDK has no settings file format (settings live on the
// device; `settings get`/`set` talk to it directly).
const SETTING_COMMANDS = [
  "keymap.onlyCustom",
  "settings.defaultLayer",
  "qukeys.holdTimeout",
  "qukeys.overlapThreshold",
  "superkeys.waitfor",
  "superkeys.timeout",
  "superkeys.repeat",
  "superkeys.holdstart",
  "superkeys.overlap",
  "led.brightness",
  "led.brightness.wireless",
  "led.brightnessUG",
  "led.brightnessUG.wireless",
  "led.fade",
  "idleleds.time_limit",
  "idleleds.wireless",
  "mouse.speed",
  "mouse.speedDelay",
  "mouse.accelSpeed",
  "mouse.accelDelay",
  "mouse.wheelSpeed",
  "mouse.wheelDelay",
  "mouse.speedLimit",
];

/** The `data` of one command in a backup JSON, or null when absent. */
function backupCommandData(backup, command) {
  return backup.backup.find((entry) => entry.command === command)?.data ?? null;
}

/** Whitespace-normalized form, for data-level (token) comparison. */
const tokens = (data) => data?.trim().split(/\s+/).join(" ") ?? null;

/** Asserts two backup/virtual data strings hold the same tokens. */
function assertSameTokens(sourceLabel, command, actual, expected) {
  if (tokens(actual) !== tokens(expected)) {
    throw new Error(
      `${sourceLabel}: virtual keyboard and shipped backup disagree on ` +
        `${command} — the defaults need re-adjudication before the SDK ` +
        `can keep citing them as one source`,
    );
  }
}

const virtuals = {};
for (const name of VIRTUAL_KEYBOARDS) {
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
]) {
  const shipped = fs.readFileSync(backupPath(fixture));
  const copy = fs.readFileSync(path.join(fixturesDir, fixture));

  if (!shipped.equals(copy)) {
    throw new Error(
      `${fixture} in fixtures/ has drifted from the shipped file; re-copy it`,
    );
  }
}

const defies = {
  virtual: virtuals.DefyWireless,
  shippedBackup: JSON.parse(
    fs.readFileSync(path.join(fixturesDir, "Defy-wireless-backup.json"), "utf-8"),
  ),
};

const sonsei = {
  virtual: virtuals.SonseiWireless,
  shippedBackup: JSON.parse(
    fs.readFileSync(path.join(fixturesDir, "Sonsei.json"), "utf-8"),
  ),
};

// The evidence claim, machine-checked: the Defy virtual keyboard's
// config data equals the shipped Defy wireless backup's captures, and
// the Sonsei virtual's equal the shipped Sonsei backup's. Both Defy
// virtual keyboards carry identical config data (asserted too), so the
// wired/wireless split does not multiply the defaults.
for (const command of CONFIG_COMMANDS) {
  assertSameTokens(
    "Defy",
    command,
    defies.virtual.virtual[command].data,
    backupCommandData(defies.shippedBackup, command),
  );
  assertSameTokens(
    "Sonsei",
    command,
    sonsei.virtual.virtual[command].data,
    backupCommandData(sonsei.shippedBackup, command),
  );
}

for (const command of CONFIG_COMMANDS) {
  if (
    tokens(virtuals.DefyWired.virtual[command].data) !==
    tokens(virtuals.DefyWireless.virtual[command].data)
  ) {
    throw new Error(
      `the DefyWired and DefyWireless virtual keyboards disagree on ` +
        `${command} — the defaults need re-adjudication`,
    );
  }
}

fs.mkdirSync(expectationsDir, { recursive: true });

/** The recorded shape of one virtual keyboard's defaults. */
function recordVirtual(vk) {
  const data = (command) => vk.virtual[command]?.data ?? null;

  return {
    product: vk.device.info.product,
    keyboardType: vk.device.info.keyboardType,
    usb: {
      vendorId: vk.device.usb.vendorId,
      productId: vk.device.usb.productId,
    },
    ...Object.fromEntries(
      CONFIG_COMMANDS.map((command) => [
        command.replace(".", "_"),
        data(command),
      ]),
    ),
    // The firmware-side factory layer the virtual keyboard simulates.
    // Known Bazecor data artifacts, recorded as-is and NOT used as SDK
    // defaults: DefyWired's `keymap_default` is a 160-token fragment,
    // and SonseiWireless's is an 800-token Defy-shaped keymap on a
    // 60-keys-per-layer product — both look like copy-paste drift in
    // Bazecor's virtual definitions. DefyWireless's equals its
    // `keymap.custom`.
    keymap_default: data("keymap.default"),
    settings: Object.fromEntries(
      SETTING_COMMANDS.map((command) => [
        command,
        data(command) === null ? null : data(command).trim(),
      ]),
    ),
  };
}

const expectation = {
  bazecor_commit: bazecorCommit,
  evidence:
    "Bazecor's hardware-virtual keyboards (src/api/hardware-virtual), " +
    "cross-checked at record time against the shipped default backups " +
    "(src/defaultBackups) — the two sources agree token for token on " +
    "every recorded config command. The shipped Defy WIRED backup is the " +
    "known outlier: its keymap.custom differs from the virtual keyboards " +
    "and the wireless backup at wire positions 72-73 of layer 0 (right " +
    "thumb-cluster keys: 0 0 vs 54109 54108, the bluetooth-pairing and " +
    "battery-status keys). Settings exist only in the " +
    "virtual keyboards — backups capture no settings — so their evidence " +
    "class is Bazecor's simulation alone.",
  defy: {
    wired: recordVirtual(virtuals.DefyWired),
    wireless: recordVirtual(virtuals.DefyWireless),
  },
  sonsei: {
    wireless: recordVirtual(sonsei.virtual),
  },
  raise: {
    ansi: recordVirtual(virtuals.RaiseANSI),
    iso: recordVirtual(virtuals.RaiseISO),
  },
  raise2: {
    ansi: recordVirtual(virtuals.Raise2ANSI),
    iso: recordVirtual(virtuals.Raise2ISO),
  },
};

const out = path.join(expectationsDir, "default-configs.json");

fs.writeFileSync(out, `${JSON.stringify(expectation, null, 2)}\n`);
console.log(`recorded ${path.relative(repoRoot, out)}`);
