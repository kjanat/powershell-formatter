/**
 * Node.js entry point. Loads the wasm from disk synchronously on first use;
 * the API stays async to match the browser entry.
 */
import { readFileSync } from 'node:fs';
import * as bindings from './dist/powershell_formatter_wasm.js';

let initialized = false;

function ensureInitialized(input) {
	if (!initialized) {
		const module = input === undefined
			? readFileSync(
				new URL('./dist/powershell_formatter_wasm_bg.wasm', import.meta.url),
			)
			: input;
		bindings.initSync({ module });
		initialized = true;
	}
}

export function initialize(input) {
	ensureInitialized(input);
	return Promise.resolve();
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
