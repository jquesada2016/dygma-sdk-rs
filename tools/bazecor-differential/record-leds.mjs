// Differential-recording harness for the LED wire formats (WS-C):
// palette and colormap, as Bazecor's own parsers see them.
//
// Runs Bazecor's OWN parsing and color sources (the read-only shallow
// clone at ../bazecor, or $BAZECOR_SRC) against the shipped default
// backups in this directory's fixtures/, and records its output into
// expectations/leds.json. The Rust LED types' tests then compare their
// behavior against these recorded expectations, so the palette/colormap
// work is validated against Bazecor's actual behavior, not a
// transcription.
//
// What runs from Bazecor, verbatim:
//   - src/api/parsers/palette.ts (parsePaletteRaw — the RGBW chunking
//     and the rgbw2b display conversion)
//   - src/api/parsers/colormap.ts (parseColormapRaw — the layer
//     chunking)
//   - src/api/color/ (rgb2w — what LayoutEditor's updatePalette writes
//     back on an RGBW device)
//
// Recorded per fixture:
//   - palette: the wire data (input), echoed so the expectation file
//     is self-contained
//   - display_colors: parsePaletteRaw's per-color display RGB — what
//     our RgbwColor::to_rgb must reproduce exactly
//   - app_writeback: what LayoutEditor's `updatePalette` would write
//     back for the displayed palette — rgb2w(display) per color, its
//     lossy re-derivation of the white channel (ours is verbatim, and
//     this is the recorded proof of the difference). Recorded for the
//     RGBW fixtures only: on an RGB device updatePalette writes the
//     display channels as-is, so there the writeback IS display_colors.
//   - colormap: the wire data's layer chunking (layer count and per-
//     layer sizes under the product's layer size)
//
// Usage:  node tools/bazecor-differential/record-leds.mjs
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

const DEFY_FIXTURES = ["Defy-wireless-backup.json", "Defy-wired-backup.json"];
const SONSEI_FIXTURE = "Sonsei.json";

for (const required of [
  path.join(bazecorSrc, "src", "api", "parsers", "palette.ts"),
  path.join(bazecorSrc, "src", "api", "color", "index.ts"),
  path.join(fixturesDir, "Defy-wireless-backup.json"),
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

const parsers = path.join(bazecorSrc, "src", "api", "parsers");

const [{ parsePaletteRaw }, { parseColormapRaw }, colorModule] = await Promise.all([
  import(pathToFileURL(path.join(parsers, "palette.ts")).href),
  import(pathToFileURL(path.join(parsers, "colormap.ts")).href),
  import(pathToFileURL(path.join(bazecorSrc, "src", "api", "color", "index.ts")).href),
]);

const DEFI_COLOR_LAYER_SIZE = 178; // LayoutEditor getColormap's layer size for the Defy
const SONSEI_COLOR_LAYER_SIZE = 56; // ...and for the Sonsei (56 keyboard LEDs, no underglow)

const fixtureData = (fixture, command) => {
  const backup = JSON.parse(fs.readFileSync(path.join(fixturesDir, fixture), "utf-8"));
  const entry = backup.backup.find((entry) => entry.command === command);

  if (!entry) throw new Error(`${fixture} carries no ${command}`);

  return entry.data;
};

const displayOf = (color) => ({ r: color.r, g: color.g, b: color.b });

// LayoutEditor's `updatePalette` on an RGBW device, per color: the
// display color run through `rgb2w`, re-deriving the white channel the
// parse dropped. (On an RGB device it writes the display channels
// as-is — no writeback is recorded there; display_colors is the
// writeback.)
const writebackOf = (color) => {
  const rgbw = colorModule.rgb2w({ r: color.r, g: color.g, b: color.b });

  return { r: rgbw.r, g: rgbw.g, b: rgbw.b, w: rgbw.w };
};

fs.mkdirSync(expectationsDir, { recursive: true });

const expectation = { bazecor_commit: bazecorCommit, devices: {} };

for (const fixture of [...DEFY_FIXTURES, SONSEI_FIXTURE]) {
  const product = JSON.parse(fs.readFileSync(path.join(fixturesDir, fixture), "utf-8"))
    .neuron?.device?.info?.product;

  // The RGBW flag follows the product's descriptor: the Defy carries
  // `RGBWMode: true`, everything else Bazecor parses as plain RGB.
  const isRGBW = product === "Defy";
  const layerSize = isRGBW ? DEFI_COLOR_LAYER_SIZE : SONSEI_COLOR_LAYER_SIZE;

  const paletteData = fixtureData(fixture, "palette");
  const colormapData = fixtureData(fixture, "colormap.map");

  const display = parsePaletteRaw(paletteData, isRGBW);
  const colormap = parseColormapRaw(colormapData, layerSize);

  expectation.devices[fixture] = {
    product,
    rgbw_mode: isRGBW,
    palette_wire: paletteData.trim(),
    // What Bazecor's UI shows per palette color (its `rgbw2b` for the
    // Defy; plain RGB passthrough for the Sonsei).
    display_colors: display.map(displayOf),
    // What LayoutEditor's `updatePalette` would write back for the
    // displayed palette on an RGBW device (see `writebackOf`); absent
    // for RGB fixtures, where the writeback is display_colors itself.
    app_writeback: isRGBW ? display.map(writebackOf) : null,
    colormap_wire: colormapData.trim(),
    colormap_layers: colormap.length,
    colormap_layer_size: Math.min(...colormap.map((layer) => layer.length)),
  };
}

const out = path.join(expectationsDir, "leds.json");

fs.writeFileSync(out, `${JSON.stringify(expectation, null, 2)}\n`);
console.log(`recorded ${path.relative(repoRoot, out)}`);
