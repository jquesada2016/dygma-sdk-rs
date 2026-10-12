// Stub for Bazecor's `@Renderer/types/layout` — a type-only module its
// parser and conversion files import as value imports (`KeyType`,
// `PaletteType`); under Node's type stripping the types vanish, so
// dummy exports satisfy the import-time named-export checks. The
// values are never referenced at runtime — only in type positions.
export const KeyType = undefined;
export const PaletteType = undefined;
