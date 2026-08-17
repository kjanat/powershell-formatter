/**
 * Browser entry point. The wasm is fetched relative to this module on first
 * use and the instance is cached; repeated formatting never re-instantiates.
 */
import initWasm, * as bindings from './dist/pwsh_formatter_wasm.js';

let initPromise;
let initInput;

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
		input === undefined ? undefined : { module_or_path: input },
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
