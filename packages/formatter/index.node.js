/**
 * Node.js entry point. Loads the wasm from disk synchronously on first use;
 * the API stays async to match the browser entry.
 */
import { readFileSync } from 'node:fs';
import * as bindings from './dist/pwsh_formatter_wasm.js';

let initialized = false;
let initInput;

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
		? readFileSync(
			new URL('./dist/pwsh_formatter_wasm_bg.wasm', import.meta.url),
		)
		: input;
	// `initialized` is only set after initSync returns, so a failed load
	// leaves the module retryable rather than permanently broken.
	bindings.initSync({ module });
	initInput = input;
	initialized = true;
}

export async function initialize(input) {
	ensureInitialized(input);
}

export async function format(source, options, catalog) {
	ensureInitialized();
	return bindings.format(source, options, catalog);
}

export async function formatRange(source, range, options, catalog) {
	ensureInitialized();
	return bindings.formatRange(source, range, options, catalog);
}

export async function version() {
	ensureInitialized();
	return bindings.version();
}

export async function defaultOptions() {
	ensureInitialized();
	return bindings.defaultOptions();
}
