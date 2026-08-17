// @ts-self-types="./index.d.ts"
/**
 * JSR entry point. JSR serves one entry to every runtime, so neither npm
 * entry fits: the browser one fetches the wasm relative to itself (no
 * `fetch` for `file:` URLs under Node), and the Node one reads it from disk
 * (no disk under Deno, which loads JSR modules over https). This resolves
 * the source by scheme and hands it to the shared runtime, which is
 * otherwise identical to the browser entry.
 */
import * as api from './index.js';

const wasmUrl = new URL('./dist/pwsh_formatter_wasm_bg.wasm', import.meta.url);

/** @returns {Promise<unknown>} bytes when on disk, else the URL to fetch. */
async function wasmSource() {
	if (wasmUrl.protocol !== 'file:') return wasmUrl;
	const { readFile } = await import('node:fs/promises');
	return readFile(wasmUrl);
}

/** @type {Promise<void> | undefined} */
let ready;

/**
 * @param {unknown} [input]
 * @returns {Promise<void>}
 */
export function initialize(input) {
	// An explicit source keeps the shared semantics, including the rejection
	// when a second call names a different one.
	if (input !== undefined) return api.initialize(input);
	// Resolve our default source once: `readFile` hands back a fresh buffer
	// each call, which a second delegation would read as a source change.
	ready ??= (async () => api.initialize(await wasmSource()))();
	return ready;
}

/**
 * @param {string} source
 * @param {import('#js').FormatOptions} [options]
 * @param {import('#js').CommandCatalog} [catalog]
 * @returns {Promise<import('#js').FormatResult>}
 */
export async function format(source, options, catalog) {
	await initialize();
	return api.format(source, options, catalog);
}

/**
 * @param {string} source
 * @param {import('#js').FormatRange} range
 * @param {import('#js').FormatOptions} [options]
 * @param {import('#js').CommandCatalog} [catalog]
 * @returns {Promise<import('#js').FormatResult>}
 */
export async function formatRange(source, range, options, catalog) {
	await initialize();
	return api.formatRange(source, range, options, catalog);
}

/** @returns {Promise<string>} */
export async function version() {
	await initialize();
	return api.version();
}

/** @returns {Promise<Required<import('#js').FormatOptions>>} */
export async function defaultOptions() {
	await initialize();
	return api.defaultOptions();
}
