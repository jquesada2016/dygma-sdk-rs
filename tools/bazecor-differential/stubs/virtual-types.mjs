// Stub for Bazecor's `@Renderer/types/virtual` — a type-only module
// the hardware-virtual keyboard definitions import as a value import
// (`VirtualType`); under Node's type stripping the type vanishes, so a
// dummy export satisfies the import-time named-export check. The value
// is never referenced at runtime — only in a type position.
export const VirtualType = undefined;
