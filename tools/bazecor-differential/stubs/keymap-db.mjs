// Stub for the keycode database `bazecor/src/api/keymap.ts` exports as
// `KeymapDB`. Only `serializeKeymap` in Bazecor's `parsers/keymap.ts`
// calls it; the differential harness uses that file's `parseKeymapRaw`
// alone, and constructing the real database would drag in Bazecor's
// whole renderer keycode tree.
export class KeymapDB {
  serialize() {
    throw new Error("the differential harness never serializes through Bazecor's KeymapDB");
  }
}
