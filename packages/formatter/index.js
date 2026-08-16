/**
 * Browser entry point. The wasm is fetched relative to this module on first
 * use and the instance is cached; repeated formatting never re-instantiates.
 */
import initWasm, * as bindings from './dist/powershell_formatter_wasm.js';

let initPromise;

export function initialize(input) {
	if (!initPromise) {
		initPromise = initWasm(
			input === undefined ? undefined : { module_or_path: input },
		).then(() => undefined);
	}
	return initPromise;
}

export async function format(source, options, catalog) {
	await initialize();
	return bindings.format(source, options, catalog);
}

export async function formatRange(source, range, options, catalog) {
	await initialize();
	return bindings.formatRange(source, range, options, catalog);
}

export async function version() {
	await initialize();
	return bindings.version();
}

export async function defaultOptions() {
	await initialize();
	return bindings.defaultOptions();
}
