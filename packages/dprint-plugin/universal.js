// @ts-self-types="./universal.d.ts"
/**
 * JSR entry point. The npm accessors hand back an on-disk path and its
 * bytes, which only exists when the module itself was loaded from disk —
 * Deno loads JSR modules over https, where both throw. This entry resolves
 * the artifact by URL and reads it whichever way that URL allows.
 */
const wasmUrl = new URL('./plugin.wasm', import.meta.url);

/** URL of the bundled `plugin.wasm`, wherever this module was loaded from. */
export function getUrl() {
	return wasmUrl;
}

/** Contents of the bundled `plugin.wasm`; works on disk and over http(s). */
export async function getBytes() {
	if (wasmUrl.protocol === 'file:') {
		const { readFile } = await import('node:fs/promises');
		return new Uint8Array(await readFile(wasmUrl));
	}
	const response = await fetch(wasmUrl);
	if (!response.ok) {
		throw new Error(
			`failed to fetch plugin.wasm: ${response.status} ${response.statusText}`,
		);
	}
	return new Uint8Array(await response.arrayBuffer());
}

/**
 * Absolute path of the bundled `plugin.wasm`. Only a module loaded from
 * disk has one; anywhere else this throws rather than hand back a path
 * that does not exist.
 */
export async function getPath() {
	if (wasmUrl.protocol !== 'file:') {
		throw new Error(
			`plugin.wasm has no filesystem path when loaded from ${wasmUrl.protocol}//; use getBytes()`,
		);
	}
	const { fileURLToPath } = await import('node:url');
	return fileURLToPath(wasmUrl);
}
