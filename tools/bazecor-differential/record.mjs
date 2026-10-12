// Differential-recording harness for the cross-product backup
// conversions (WS-E phase 3).
//
// Runs Bazecor's OWN conversion source (the read-only shallow clone at
// ../bazecor, or $BAZECOR_SRC) against the shipped default backups in
// this directory's fixtures/, and records its output byte for byte
// into expectations/. The Rust conversion layer's tests then compare
// their output against these recorded expectations, so the conversion
// is validated against Bazecor's actual behavior, not a transcription.
//
// What runs from Bazecor, verbatim:
//   - src/api/parsers/conversions/defyToSonsei.ts (the mapping tables)
//   - src/api/parsers/{keymap,colormap,palette}.ts (the raw parsers)
//   - src/api/parsers/conversions/{raiseToRaise2,raise2ToRaise}.ts
//     (the palette converters)
//   - src/api/color/ (rgb2w / rgbw2b / sanitizeIntensity)
//
// What is mirrored here (and only here): the ~30-line orchestration of
// Backup.ts convertDefyToSonsei — findIndex of the three commands, the
// layer sizes (80/178/RGBW for the Defy), and the join(" ") — with a
// pointer to the exact lines mirrored.
//
// Usage:  node tools/bazecor-differential/record.mjs
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

const DEFY_FIXTURES = ["Defy-wireless-backup.json", "Defy-wired-backup.json"];

for (const required of [
  path.join(bazecorSrc, "src", "api", "backup", "Backup.ts"),
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

const conversions = path.join(bazecorSrc, "src", "api", "parsers", "conversions");
const parsers = path.join(bazecorSrc, "src", "api", "parsers");

const [
  { convertKeymapDefyToSonsei, convertColormapDefyToSonsei },
  { convertPaletteRtoR2, convertPaletteR2toR },
  { parseKeymapRaw },
  { parseColormapRaw },
  { parsePaletteRaw },
] = await Promise.all([
  import(pathToFileURL(path.join(conversions, "defyToSonsei.ts")).href),
  import(pathToFileURL(path.join(conversions, "raiseToRaise2.ts")).href).then(async (
    raiseToRaise2,
  ) => ({
    ...raiseToRaise2,
    ...(await import(pathToFileURL(path.join(conversions, "raise2ToRaise.ts")).href)),
  })),
  import(pathToFileURL(path.join(parsers, "keymap.ts")).href),
  import(pathToFileURL(path.join(parsers, "colormap.ts")).href),
  import(pathToFileURL(path.join(parsers, "palette.ts")).href),
]);

const DEFI_KEY_LAYER_SIZE = 80; // convertDefyToSonsei's `defyKeyLayerSize`
const DEFI_COLOR_LAYER_SIZE = 178; // convertDefyToSonsei's `defyColorLayerSize`

/**
 * Mirrors Backup.ts `convertDefyToSonsei` (lines ~452–488): locate the
 * three commands, parse each against the Defy shapes, convert, and
 * join the way Backup.ts serializes (`.flat().map(String).join(" ")`).
 */
function convertDefyToSonsei(backup) {
  const find = (command) => {
    const index = backup.backup.findIndex((entry) => entry.command === command);

    if (index === -1) throw new Error(`fixture carries no ${command}`);

    return backup.backup[index].data;
  };

  const keymap = parseKeymapRaw(find("keymap.custom"), DEFI_KEY_LAYER_SIZE);
  const colormap = parseColormapRaw(find("colormap.map"), DEFI_COLOR_LAYER_SIZE);
  const palette = parsePaletteRaw(find("palette"), true);

  const keymapFinal = keymap.map((layer) => convertKeymapDefyToSonsei(layer));
  const colormapFinal = colormap.map((layer) => convertColormapDefyToSonsei(layer));
  const paletteFinal = palette.map((color) => convertPaletteR2toR(color));

  return {
    keymap_custom: keymapFinal.flat().map((k) => k.toString()).join(" "),
    colormap_map: colormapFinal.flat().map((k) => k.toString()).join(" "),
    palette: paletteFinal
      .map((color) => [color.r, color.g, color.b])
      .flat()
      .map((v) => v.toString())
      .join(" "),
  };
}

/**
 * Mirrors the palette branch of Backup.ts's Raise → Raise2 conversion
 * (restoreBackup convertRaiseToRaise2 and the restoreVirtual twin):
 * the only RGB → RGBW palette conversion Bazecor has. The Sonsei →
 * Defy direction does not exist in Bazecor (it blind-restores that
 * pair), so the Sonsei palette is fed through this exact pipeline —
 * parse as RGB, convertPaletteRtoR2, then rgb2w into four channels.
 */
function convertSonseiPaletteToDefy(backup) {
  const find = (command) => {
    const index = backup.backup.findIndex((entry) => entry.command === command);

    if (index === -1) throw new Error(`fixture carries no ${command}`);

    return backup.backup[index].data;
  };

  const palette = parsePaletteRaw(find("palette"), false);

  const paletteFinal = palette.map((color) => convertPaletteRtoR2(color));

  // Backup.ts's final map runs rgb2w over the converted colors.
  return {
    palette: paletteFinal
      .map((color) => {
        const rgbw = color_rgb2w(color);

        return [rgbw.r, rgbw.g, rgbw.b, rgbw.w];
      })
      .flat()
      .map((v) => v.toString())
      .join(" "),
  };
}

const color = await import(
  pathToFileURL(path.join(bazecorSrc, "src", "api", "color", "index.ts")).href
);
const color_rgb2w = color.rgb2w;

fs.mkdirSync(expectationsDir, { recursive: true });

// The committed fixture copies must stay byte-identical to the
// shipped files they claim to snapshot — a silent drift would decouple
// the recorded expectations from the inputs the Rust tests parse.
for (const fixture of [...DEFY_FIXTURES, "Sonsei.json"]) {
  const shipped = fs.readFileSync(backupPath(fixture));
  const copy = fs.readFileSync(path.join(fixturesDir, fixture));

  if (!shipped.equals(copy)) {
    throw new Error(
      `${fixture} in fixtures/ has drifted from the shipped file; re-copy it`,
    );
  }
}

for (const fixture of DEFY_FIXTURES) {
  const backup = JSON.parse(fs.readFileSync(path.join(fixturesDir, fixture), "utf-8"));

  if (backup.neuron.device?.info?.product !== "Defy") {
    throw new Error(`${fixture} is not a Defy backup`);
  }

  const converted = convertDefyToSonsei(backup);
  const expectation = { bazecor_commit: bazecorCommit, source_fixture: fixture, ...converted };
  // Lowercase on purpose: the Rust tests include these files by
  // path, and the canonical name must not depend on the recording
  // machine's filesystem case-sensitivity.
  const out = path.join(
    expectationsDir,
    fixture.replace("-backup.json", "-to-sonsei.json").toLowerCase(),
  );

  fs.writeFileSync(out, `${JSON.stringify(expectation, null, 2)}\n`);
  console.log(`recorded ${path.relative(repoRoot, out)}`);
}

// The Sonsei palette → Defy RGBW expectation, through Bazecor's own
// RGB → RGBW pipeline.
{
  const backup = JSON.parse(fs.readFileSync(path.join(fixturesDir, "Sonsei.json"), "utf-8"));

  if (backup.neuron.device?.info?.product !== "Sonsei") {
    throw new Error("Sonsei.json is not a Sonsei backup");
  }

  const converted = convertSonseiPaletteToDefy(backup);
  const out = path.join(expectationsDir, "sonsei-palette-to-defy.json");

  fs.writeFileSync(
    out,
    `${JSON.stringify({ bazecor_commit: bazecorCommit, ...converted }, null, 2)}\n`,
  );
  console.log(`recorded ${path.relative(repoRoot, out)}`);
}
