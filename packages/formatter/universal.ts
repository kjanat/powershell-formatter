/**
 * JSR entry point. JSR serves one entry to every runtime, so neither npm
 * entry fits: the browser one fetches the wasm relative to itself (no
 * `fetch` for `file:` URLs under Node), and the Node one reads it from
 * disk (no disk under Deno, which loads JSR modules over https). This
 * resolves the source by scheme and hands it to the shared runtime, which
 * is otherwise identical to the browser entry.
 */
import * as api from './index.js';
import type {
	CommandCatalog,
	FormatOptions,
	FormatRange,
	FormatResult,
} from './index.d.ts';

export type {
	CommandCatalog,
	Diagnostic,
	FormatOptions,
	FormatRange,
	FormatResult,
} from './index.d.ts';

const wasmUrl = new URL('./dist/pwsh_formatter_wasm_bg.wasm', import.meta.url);

/** Bytes when the wasm is on disk, else the URL for the runtime to fetch. */
async function wasmSource(): Promise<unknown> {
	if (wasmUrl.protocol !== 'file:') return wasmUrl;
	const { readFile } = await import('node:fs/promises');
	return readFile(wasmUrl);
}

let ready: Promise<void> | undefined;

/** Explicitly initialize the WebAssembly runtime (optional). */
export function initialize(input?: unknown): Promise<void> {
	// An explicit source keeps the shared semantics, including the rejection
	// when a second call names a different one.
	if (input !== undefined) return api.initialize(input);
	// Resolve our default source once: `readFile` hands back a fresh buffer
	// each call, which a second delegation would read as a source change.
	ready ??= (async () => api.initialize(await wasmSource()))();
	return ready;
}

/** Format a complete PowerShell source string. */
export async function format(
	source: string,
	options?: FormatOptions,
	catalog?: CommandCatalog,
): Promise<FormatResult> {
	await initialize();
	return api.format(source, options, catalog);
}

/** Format only the given range, leaving the rest byte-identical. */
export async function formatRange(
	source: string,
	range: FormatRange,
	options?: FormatOptions,
	catalog?: CommandCatalog,
): Promise<FormatResult> {
	await initialize();
	return api.formatRange(source, range, options, catalog);
}

/** The formatter's version. */
export async function version(): Promise<string> {
	await initialize();
	return api.version();
}

/** The complete default configuration. */
export async function defaultOptions(): Promise<Required<FormatOptions>> {
	await initialize();
	return api.defaultOptions();
}
