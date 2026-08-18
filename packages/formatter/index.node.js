/**
 * Node.js entry point. Loads the wasm from disk synchronously on first use;
 * the API stays async to match the browser entry.
 */
import { readFileSync } from 'node:fs';
import * as bindings from '#wasm';

/** @typedef {import('#js').CommandCatalog} CommandCatalog */
/** @typedef {import('#js').FormatOptions}  FormatOptions */
/** @typedef {import('#js').FormatRange}    FormatRange */
/** @typedef {import('#js').FormatResult}   FormatResult */

let initialized = false;
/** @type {unknown} */
let initInput;

/** @param {unknown} [input] */
function ensureInitialized(input) {
	if (initialized) {
		// Already instantiated; a different wasm source cannot be swapped in.
		if (input !== undefined && input !== initInput) {
			throw new Error(
				'the formatter is already initialized from a different wasm source',
			);
		}
		return;
	}
	const module = input === undefined
		? readFileSync(new URL(import.meta.resolve('#wasm/bg')))
		: input;
	// `initialized` is only set after initSync returns, so a failed load
	// leaves the module retryable rather than permanently broken.
	bindings.initSync({
		module:
			/** @type {import('#wasm').SyncInitInput} */ (
				module
			),
	});
	initInput = input;
	initialized = true;
}

/** @param {unknown} [input] */
export async function initialize(input) {
	ensureInitialized(input);
}

/**
 * @param {string} source
 * @param {FormatOptions} [options]
 * @param {CommandCatalog} [catalog]
 * @returns {Promise<FormatResult>}
 */
export async function format(source, options, catalog) {
	ensureInitialized();
	return bindings.format(source, options, catalog);
}

/**
 * @param {string} source
 * @param {FormatRange} range
 * @param {FormatOptions} [options]
 * @param {CommandCatalog} [catalog]
 * @returns {Promise<FormatResult>}
 */
export async function formatRange(source, range, options, catalog) {
	ensureInitialized();
	return bindings.formatRange(source, range, options, catalog);
}

/** @returns {Promise<string>} */
export async function version() {
	ensureInitialized();
	return bindings.version();
}

/** @returns {Promise<Required<FormatOptions>>} */
export async function defaultOptions() {
	ensureInitialized();
	return bindings.defaultOptions();
}
