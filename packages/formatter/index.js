// @ts-self-types="./index.d.ts"
/**
 * Browser entry point. The wasm is fetched relative to this module on first
 * use and the instance is cached; repeated formatting never re-instantiates.
 */
import initWasm, * as bindings from './dist/pwsh_formatter_wasm.js';

/** @typedef {import('#js').CommandCatalog} CommandCatalog */
/** @typedef {import('#js').FormatOptions}  FormatOptions */
/** @typedef {import('#js').FormatRange}    FormatRange */
/** @typedef {import('#js').FormatResult}   FormatResult */

/** @typedef {import('#wasm').InitInput} InitInput */

/** @type {Promise<void> | undefined} */
let initPromise;
/** @type {unknown} */
let initInput;

/**
 * @param {unknown} [input]
 * @returns {Promise<void>}
 */
export function initialize(input) {
	if (initPromise) {
		// A second call naming a different wasm source cannot be honored: the
		// module is already instantiated. Say so instead of silently handing
		// back an instance loaded from somewhere else.
		if (input !== undefined && input !== initInput) {
			return Promise.reject(
				new Error(
					'the formatter is already initialized from a different wasm source',
				),
			);
		}
		return initPromise;
	}
	initInput = input;
	initPromise = initWasm(
		input === undefined
			? undefined
			: { module_or_path: /** @type {InitInput} */ (input) },
	).then(
		() => undefined,
		(err) => {
			// Never cache a failure: a transient fetch error would otherwise
			// poison every later call with no way to retry.
			initPromise = undefined;
			initInput = undefined;
			throw err;
		},
	);
	return initPromise;
}

/**
 * @param {string} source
 * @param {FormatOptions} [options]
 * @param {CommandCatalog} [catalog]
 * @returns {Promise<FormatResult>}
 */
export async function format(source, options, catalog) {
	await initialize();
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
	await initialize();
	return bindings.formatRange(source, range, options, catalog);
}

/** @returns {Promise<string>} */
export async function version() {
	await initialize();
	return bindings.version();
}

/** @returns {Promise<Required<FormatOptions>>} */
export async function defaultOptions() {
	await initialize();
	return bindings.defaultOptions();
}
