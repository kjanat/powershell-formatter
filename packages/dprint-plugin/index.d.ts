/** Absolute path of the bundled `plugin.wasm` on disk. */
export function getPath(): string;

/**
 * Contents of the bundled `plugin.wasm`.
 *
 * Typed as `Uint8Array` to keep the package free of a `@types/node`
 * dependency; at runtime this is a Node `Buffer` (a `Uint8Array` subclass).
 */
export function getBuffer(): Uint8Array;
