// Must be the first import of main.tsx: ES modules evaluate in import order, and
// @solana/spl-token reads the Node `Buffer` global while it is being evaluated (ADR-016).
import { Buffer } from "buffer";

globalThis.Buffer ??= Buffer;
