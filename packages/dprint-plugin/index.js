/**
 * Wasm carrier for the PowerShell dprint plugin, following the convention of
 * `@dprint/json`, `@dprint/typescript`, etc.: the package holds the artifact
 * and two accessors, nothing more. Hosting it (e.g. via `@dprint/formatter`'s
 * `createFromBuffer`) is the consumer's dependency, not this package's.
 */
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const wasmUrl = new URL('./plugin.wasm', import.meta.url);

/** Absolute path of the bundled `plugin.wasm` on disk. */
export function getPath() {
	return fileURLToPath(wasmUrl);
}

/** Contents of the bundled `plugin.wasm`. */
export function getBuffer() {
	return readFileSync(wasmUrl);
}
