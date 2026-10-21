// Differential-recording harness for the Raise ↔ Raise2 backup
// conversions (issue #24, unit 1).
//
// Runs Bazecor's OWN conversion source (the read-only shallow clone at
// ../bazecor, or $BAZECOR_SRC) against the shipped Raise backups and
// Raise2 virtual keyboards in this directory's fixtures/, and records
// its output byte for byte into expectations/. The Rust conversion
// layer's tests then compare their output against these recorded
// expectations, so the conversion is validated against Bazecor's
// actual behavior, not a transcription.
//
// What runs from Bazecor, verbatim:
//   - src/api/parsers/conversions/raiseToRaise2.ts (the Raise → Raise2
//     keymap rotation, ANSI swaps, colormap interpolation)
//   - src/api/parsers/conversions/raise2ToRaise.ts (the inverse
//     rotation, the ANSI/ISO colormap swaps, the truncation)
//   - src/api/parsers/{keymap,colormap,palette}.ts (the raw parsers)
//   - src/api/color/ (rgb2w)
//
// What is mirrored here (and only here): the ~30-line orchestration of
// Backup.ts convertRaiseToRaise2 (lines ~490–531 at ff9e10b) and
// convertRaise2ToRaise (~533–572) — findIndex of the three commands,
// the layer sizes (keymap 80; colormap the backup descriptor's
// keyboardUnderglow rows × columns), the keyboardType arguments
// (target's `keyboardType`, backup's `backupKeyboardType`), and the
// join(" ") — each with a pointer to the exact lines mirrored.
//
// The shipped Raise2 files are NOT backups — they are hardware-virtual
// definitions `{device, virtual}`. The backup-shape input Bazecor's
// converters need is synthesized here (documented below) from the
// virtual's `device` descriptor and its three eraseable conversion
// commands; the conversion itself never sees anything else.
//
// The Raise2ISO virtual is the known shipped-data outlier: its
// colormap.map holds 1440 tokens (10 × 144) while its own descriptor
// geometry is 5 × 29 = 145 per layer. Bazecor chunks the ragged data
// and converts ten misaligned layers; that output is recorded as a
// clearly-marked OUTLIER entry the Rust side does NOT match — the Rust
// conversion rejects the input with its typed shape error instead
// (a documented, pinned deviation, the same class as the wireless
// channelHop capture).
//
// Usage:  node tools/bazecor-differential/record-raise-conversions.mjs
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

const RAISE_BACKUPS = ["RaiseANSI.json", "RaiseISO.json"];
const RAISE2_VIRTUALS = ["Raise2ANSI.json", "Raise2ISO.json"];

for (const required of [
  path.join(bazecorSrc, "src", "api", "backup", "Backup.ts"),
  ...RAISE_BACKUPS.map(backupPath),
  ...RAISE2_VIRTUALS.map(backupPath),
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

const conversions = path.join(bazecorSrc, "src", "api", "parsers", "conversions");
const parsers = path.join(bazecorSrc, "src", "api", "parsers");

const [
  { convertKeymapRtoR2, convertColormapRtoR2 },
  { convertKeymapR2toR, convertColormapR2toR },
  { parseKeymapRaw },
  { parseColormapRaw },
  { parsePaletteRaw },
] = await Promise.all([
  import(pathToFileURL(path.join(conversions, "raiseToRaise2.ts")).href),
  import(pathToFileURL(path.join(conversions, "raise2ToRaise.ts")).href),
  import(pathToFileURL(path.join(parsers, "keymap.ts")).href),
  import(pathToFileURL(path.join(parsers, "colormap.ts")).href),
  import(pathToFileURL(path.join(parsers, "palette.ts")).href),
]);

const color = await import(
  pathToFileURL(path.join(bazecorSrc, "src", "api", "color", "index.ts")).href
);
const color_rgb2w = color.rgb2w;

// Backup.ts convertRaiseToRaise2's `keyLayerSize` (line ~493): the
// keymap layer width, hardcoded on every Raise path.
const KEY_LAYER_SIZE = 80;

const join = (layers) => layers.flat().map((k) => k.toString()).join(" ");
const find = (backup, command) => {
  const index = backup.backup.findIndex((entry) => entry.command === command);

  if (index === -1) throw new Error(`fixture carries no ${command}`);

  return backup.backup[index].data;
};

/**
 * Mirrors Backup.ts `convertRaiseToRaise2` (lines ~490–531): the layer
 * sizes (keymap 80; colormap the backup descriptor's keyboardUnderglow
 * rows × columns, fallback 132 — the fixtures all carry the geometry),
 * the parse shapes (palette RGB: `parsePaletteRaw(data, false)`), the
 * keyboardType arguments (the TARGET device's `keyboardType`, the
 * backup's as `backupKeyboardType`), the conversions, and the
 * serialization — palette through rgb2w into four channels.
 *
 * `dev` is Bazecor's Device shape only where the converter reads it:
 * `dev.device` (the target descriptor replacing the backup's) and
 * `dev.device.info.keyboardType`.
 */
function convertRaiseToRaise2(backup, dev) {
  const bkpDev = backup.neuron.device;
  const colorLayerSize = bkpDev
    ? bkpDev.keyboardUnderglow.rows * bkpDev.keyboardUnderglow.columns
    : 132;

  const custom = parseKeymapRaw(find(backup, "keymap.custom"), KEY_LAYER_SIZE);
  const palette = parsePaletteRaw(find(backup, "palette"), false);
  const colormap = parseColormapRaw(find(backup, "colormap.map"), colorLayerSize);

  const keymapFinal = custom.map((layer) =>
    convertKeymapRtoR2(layer, dev.device.info.keyboardType),
  );
  const colormapFinal = colormap.map((layer) =>
    convertColormapRtoR2(layer, dev.device.info.keyboardType, backup.neuron.device.info.keyboardType),
  );
  const paletteFinal = palette.map((color) => convertRaisePaletteToRaise2(color));

  return {
    keymap_custom: join(keymapFinal),
    colormap_map: join(colormapFinal),
    palette: join(paletteFinal),
  };
}

/**
 * Backup.ts convertRaiseToRaise2's palette serialization (lines
 * ~523–529): every parsed color goes through rgb2w and serializes as
 * `[r, g, b, w]`. (The converter's own `convertPaletteRtoR2` call at
 * line ~510 copies r/g/b unchanged into a fresh object, so feeding its
 * output into rgb2w is numerically the parse output — recorded through
 * the real rgb2w, the live math.)
 */
function convertRaisePaletteToRaise2(color) {
  const rgbw = color_rgb2w(color);

  return [rgbw.r, rgbw.g, rgbw.b, rgbw.w];
}

/**
 * Mirrors Backup.ts `convertRaise2ToRaise` (lines ~533–572): the same
 * orchestration the other way — palette parsed as RGBW
 * (`parsePaletteRaw(data, true)`, which folds the white channel into
 * the display colors and drops it), serialized back as `[r, g, b]`;
 * colormap source width from the backup descriptor's geometry,
 * fallback 176.
 */
function convertRaise2ToRaise(backup, dev) {
  const bkpDev = backup.neuron.device;
  const colorLayerSize = bkpDev
    ? bkpDev.keyboardUnderglow.rows * bkpDev.keyboardUnderglow.columns
    : 176;

  const custom = parseKeymapRaw(find(backup, "keymap.custom"), KEY_LAYER_SIZE);
  const palette = parsePaletteRaw(find(backup, "palette"), true);
  const colormap = parseColormapRaw(find(backup, "colormap.map"), colorLayerSize);

  const keymapFinal = custom.map((layer) =>
    convertKeymapR2toR(layer, dev.device.info.keyboardType),
  );
  const colormapFinal = colormap.map((layer) =>
    convertColormapR2toR(layer, dev.device.info.keyboardType, backup.neuron.device.info.keyboardType),
  );
  const paletteFinal = palette.map((color) => [color.r, color.g, color.b]);

  return {
    keymap_custom: join(keymapFinal),
    colormap_map: join(colormapFinal),
    palette: join(paletteFinal),
  };
}

/**
 * The shipped Raise2 files are hardware-virtual definitions
 * `{device, virtual}`, not backups. Bazecor's converters take a
 * backup `{neuron: {device}, backup: [{command, data}...]}` — the
 * backup-shape input is synthesized here from exactly the pieces the
 * conversion reads: the virtual's `device` descriptor (which becomes
 * the synthesized backup's `neuron.device`, carrying both the
 * keyboardUnderglow geometry and the source `keyboardType`) and the
 * three eraseable conversion commands' data strings, verbatim.
 */
function raise2VirtualAsBackup(fixtureName) {
  const virtual = JSON.parse(fs.readFileSync(path.join(fixturesDir, fixtureName), "utf-8"));
  const command = (name) => ({ command: name, data: virtual.virtual[name].data });

  return {
    neuronID: "",
    neuron: {
      id: "",
      name: "",
      layers: [],
      macros: [],
      superkeys: [],
      device: virtual.device,
    },
    versions: {},
    backup: [command("keymap.custom"), command("colormap.map"), command("palette")],
  };
}

fs.mkdirSync(expectationsDir, { recursive: true });

// The committed fixture copies must stay byte-identical to the
// shipped files they claim to snapshot — a silent drift would decouple
// the recorded expectations from the inputs the Rust tests parse.
for (const fixture of [...RAISE_BACKUPS, ...RAISE2_VIRTUALS]) {
  const shipped = fs.readFileSync(backupPath(fixture));
  const copy = fs.readFileSync(path.join(fixturesDir, fixture));

  if (!shipped.equals(copy)) {
    throw new Error(
      `${fixture} in fixtures/ has drifted from the shipped file; re-copy it`,
    );
  }
}

// Raise → Raise2: both real shipped Raise backups, each onto both
// target layouts (Bazecor's restore picks the connected device's
// `keyboardType`; offline, both are recorded).
for (const fixture of RAISE_BACKUPS) {
  const backup = JSON.parse(fs.readFileSync(path.join(fixturesDir, fixture), "utf-8"));

  if (backup.neuron.device?.info?.product !== "Raise") {
    throw new Error(`${fixture} is not a Raise backup`);
  }

  for (const targetKeyboardType of ["ANSI", "ISO"]) {
    const dev = { device: { info: { keyboardType: targetKeyboardType } } };
    const converted = convertRaiseToRaise2(backup, dev);
    const stem = fixture.replace(".json", "").toLowerCase();
    const out = path.join(expectationsDir, `${stem}-to-raise2-${targetKeyboardType.toLowerCase()}.json`);
    const expectation = {
      bazecor_commit: bazecorCommit,
      source_fixture: fixture,
      source_keyboard_type: backup.neuron.device.info.keyboardType,
      target_keyboard_type: targetKeyboardType,
      ...converted,
    };

    fs.writeFileSync(out, `${JSON.stringify(expectation, null, 2)}\n`);
    console.log(`recorded ${path.relative(repoRoot, out)}`);
  }
}

// Raise2 → Raise: the two virtual keyboards synthesized into
// backup shape (see raise2VirtualAsBackup), each onto both target
// layouts — except the Raise2ISO outlier, recorded once against the
// ANSI target, clearly marked (the Rust side pins its typed rejection
// instead of this output).
for (const fixture of RAISE2_VIRTUALS) {
  const backup = raise2VirtualAsBackup(fixture);

  if (backup.neuron.device?.info?.product !== "Raise2") {
    throw new Error(`${fixture} is not a Raise2 virtual keyboard`);
  }

  const stem = fixture.replace(".json", "").toLowerCase();
  const targets =
    stem === "raise2iso"
      ? ["ANSI"]
      : ["ANSI", "ISO"];

  for (const targetKeyboardType of targets) {
    const dev = { device: { info: { keyboardType: targetKeyboardType } } };
    const converted = convertRaise2ToRaise(backup, dev);
    const outlier = stem === "raise2iso";
    const out = path.join(
      expectationsDir,
      outlier
        ? `${stem}-to-raise-outlier.json`
        : `${stem}-to-raise-${targetKeyboardType.toLowerCase()}.json`,
    );
    const expectation = {
      bazecor_commit: bazecorCommit,
      source_fixture: fixture,
      source_keyboard_type: backup.neuron.device.info.keyboardType,
      target_keyboard_type: targetKeyboardType,
      ...(outlier
        ? {
            outlier:
              "The shipped Raise2ISO virtual's colormap.map holds 1440 tokens (10 × 144) while its own descriptor geometry is 5 × 29 = 145 per layer. This entry records what Bazecor produces from that ragged input (ten misaligned layers chunked 145-wide, truncated to 132) — the Rust conversion rejects the input with its typed WrongValueCount shape error instead of matching this output. A documented, pinned deviation.",
          }
        : {}),
      ...converted,
    };

    fs.writeFileSync(out, `${JSON.stringify(expectation, null, 2)}\n`);
    console.log(`recorded ${path.relative(repoRoot, out)}${outlier ? " (OUTLIER)" : ""}`);
  }
}

// The from-ISO colormap swap (Raise 2 → Raise, ANSI target): no
// shipped Raise 2 fixture pairs a self-consistent geometry with an ISO
// layout (the ISO virtual's colormap is the ragged outlier), so the
// recording stamps the synthesized Raise2ANSI backup's
// `info.keyboardType` to "ISO" — exactly the field Backup.ts passes
// its converters as `backupKeyboardType` — and runs the same mirror.
// Data note: the ANSI virtual's colormap carries equal indices at LED
// positions 19 and 20 of every layer, so swapping them changes nothing
// and this entry's strings come out identical to the
// raise2ansi-to-raise-ansi recording — what it pins is that the
// stamped argument flows through Bazecor's own `=== "ISO"` branch
// leaving everything else untouched. The swap's visible effect is
// pinned Rust-side by a property test on data perturbed at position
// 19.
{
  const backup = raise2VirtualAsBackup("Raise2ANSI.json");

  backup.neuron.device.info.keyboardType = "ISO";

  const dev = { device: { info: { keyboardType: "ANSI" } } };
  const converted = convertRaise2ToRaise(backup, dev);
  const out = path.join(
    expectationsDir,
    "raise2ansi-stamped-iso-to-raise-ansi.json",
  );
  const expectation = {
    bazecor_commit: bazecorCommit,
    source_fixture: "Raise2ANSI.json",
    source_keyboard_type: backup.neuron.device.info.keyboardType,
    target_keyboard_type: "ANSI",
    stamped_source:
      'The source is the Raise2ANSI virtual synthesized into backup shape, with its `info.keyboardType` stamped to "ISO" — exactly the field Backup.ts reads as `backupKeyboardType` — so this entry runs Bazecor\'s from-ISO branch over the ANSI virtual\'s self-consistent data. That data carries equal colormap indices at LEDs 19 and 20 of every layer, so the recorded strings are byte-identical to the raise2ansi-to-raise-ansi entry; the swap\'s visible effect is pinned Rust-side on perturbed data.',
    ...converted,
  };

  fs.writeFileSync(out, `${JSON.stringify(expectation, null, 2)}\n`);
  console.log(`recorded ${path.relative(repoRoot, out)}`);
}
