// Module-resolution hooks for loading Bazecor's TypeScript sources
// (the READ-ONLY shallow clone) under plain Node.
//
// Three things need intercepting:
// 1. Extensionless relative imports (`from "../color"`): Node runs TS
//    by type-stripping but does not guess extensions, so try
//    `<spec>.ts` and `<spec>/index.ts` under the importing module's
//    directory.
// 2. `@Renderer/types/layout` — a type-only import Bazecor's
//    conversion files carry as a value import; redirect it to a stub
//    (the types vanish at runtime).
// 3. `parsers/keymap.ts`'s `import { KeymapDB } from "../keymap"` —
//    the keycode database, whose transitive imports reach deep into
//    Bazecor's renderer. Only `serializeKeymap` ever calls it; the
//    conversion harness uses `parseKeymapRaw`, so a stub keeps
//    Bazecor's own parser file loadable verbatim.

import { pathToFileURL } from "node:url";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const STUB_RENDERER_TYPES = pathToFileURL(path.join(here, "stubs", "renderer-types.mjs")).href;
const STUB_KEYMAP_DB = pathToFileURL(path.join(here, "stubs", "keymap-db.mjs")).href;
const STUB_COLOR_TYPES = pathToFileURL(path.join(here, "stubs", "color-types.mjs")).href;

export async function resolve(specifier, context, nextResolve) {
  if (specifier === "@Renderer/types/layout") {
    return { url: STUB_RENDERER_TYPES, shortCircuit: true };
  }

  const parent = context.parentURL ?? "";

  // Bazecor's `api/color/types.ts` declares only TypeScript
  // interfaces; its value imports (`import { RGB, RGBW } from
  // "./types"`) fail Node's named-export check against the stripped
  // (empty) module, so it redirects to a stub with dummy exports.
  if (parent.includes("/src/api/color/") && (specifier === "./types" || specifier === "../types")) {
    return { url: STUB_COLOR_TYPES, shortCircuit: true };
  }

  if (parent.includes("/src/api/parsers/keymap.ts") && specifier === "../keymap") {
    return { url: STUB_KEYMAP_DB, shortCircuit: true };
  }

  // Matched on Bazecor's api tree, not the clone's directory name —
  // the clone may sit at any path (BAZECOR_SRC).
  const insideBazecor =
    parent.includes("/src/api/") && (specifier.startsWith("./") || specifier.startsWith("../"));

  if (insideBazecor) {
    const parentDir = new URL(".", parent);

    for (const candidate of [`${specifier}.ts`, `${specifier}/index.ts`]) {
      try {
        return await nextResolve(new URL(candidate, parentDir).href, {
          ...context,
          parentURL: undefined,
        });
      } catch {
        // Try the next candidate, then the default resolution.
      }
    }
  }

  return nextResolve(specifier, context);
}
