/** URL of the bundled `plugin.wasm`, wherever this module was loaded from. */
export function getUrl(): URL;

/**
 * Contents of the bundled `plugin.wasm`. Reads from disk when this module
 * was loaded from disk, and fetches otherwise — so it works under Deno,
 * which loads JSR modules over https.
 */
export function getBytes(): Promise<Uint8Array>;

/**
 * Absolute path of the bundled `plugin.wasm`. Rejects when this module was
 * not loaded from disk, where no such path exists.
 */
export function getPath(): Promise<string>;
