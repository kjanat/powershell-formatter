// Tests of the actual built package through its Node entry point.
// Run `./build.sh` first (CI does); `node --test test/` executes these.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';

import { defaultOptions, format, formatRange, initialize, version } from '../index.node.js';

test('formats the baseline example', async () => {
	const result = await format('function foo {\n"hello"\n  }');
	assert.equal(result.text, 'function foo {\n    "hello"\n}');
	assert.equal(result.formatted, true);
	assert.deepEqual(result.diagnostics, []);
});

test('runtime is cached across calls', async () => {
	await initialize();
	const a = await format('$x=1');
	const b = await format('$y=2');
	assert.equal(a.text, '$x = 1');
	assert.equal(b.text, '$y = 2');
});

test('options are honored', async () => {
	const result = await format('if ($x) {\n1\n}', { indentWidth: 2 });
	assert.equal(result.text, 'if ($x) {\n  1\n}');
	const allman = await format('if ($x) {\n1\n}', { braceStyle: 'nextLine' });
	assert.equal(allman.text, 'if ($x)\n{\n    1\n}');
});

test('invalid options reject', async () => {
	await assert.rejects(() => format('$x=1', { braceStyle: 'sideways' }));
});

test('malformed input is preserved with diagnostics', async () => {
	const src = "function f {\n 'x'\n";
	const result = await format(src);
	assert.equal(result.formatted, false);
	assert.equal(result.text, src);
	assert.ok(result.diagnostics.length > 0);
	assert.ok(result.diagnostics.some((d) => d.code === 'formatting-skipped'));
	const d = result.diagnostics[0];
	assert.equal(typeof d.line, 'number');
	assert.equal(typeof d.column, 'number');
});

test('range formatting', async () => {
	const result = await formatRange("if($a){'x'}\nif($b){'y'}", {
		startLine: 2,
		startColumn: 1,
		endLine: 2,
		endColumn: 12,
	});
	assert.equal(result.text, "if($a){'x'}\nif ($b) { 'y' }");
});

test('catalog drives command casing', async () => {
	const result = await format(
		'get-childitem -path C:\\ -recurse',
		{},
		{ commands: { 'Get-ChildItem': ['Path', 'Recurse'] } },
	);
	assert.equal(result.text, 'Get-ChildItem -Path C:\\ -Recurse');
});

test('idempotence through the package', async () => {
	const once = await format("IF($x-EQ 1){'yes'}ELSE{'no'}");
	const twice = await format(once.text);
	assert.equal(once.text, twice.text);
});

test('unicode round-trips', async () => {
	const src = "$x = 'héllo 🎉 中文'";
	const result = await format(src);
	assert.equal(result.text, src);
});

test('version is a semver-ish string', async () => {
	assert.match(await version(), /^\d+\.\d+\.\d+/);
});

test('TypeScript declarations cover exactly the Rust option surface', async () => {
	const defaults = await defaultOptions();
	const rustKeys = new Set(Object.keys(defaults));

	const dts = readFileSync(new URL('../index.d.ts', import.meta.url), 'utf8');
	const block = dts.match(
		/export interface FormatOptions \{([\s\S]*?)\n\}/,
	)?.[1];
	assert.ok(block, 'FormatOptions interface found in index.d.ts');
	const tsKeys = new Set(
		[...block.matchAll(/^\s*(\w+)\?:/gm)].map((m) => m[1]),
	);

	assert.deepEqual(
		[...tsKeys].sort(),
		[...rustKeys].sort(),
		'index.d.ts FormatOptions must match the Rust FormatOptions serde surface',
	);
});

test('browser entry point does not import node builtins', () => {
	const browser = readFileSync(new URL('../index.js', import.meta.url), 'utf8');
	assert.ok(!browser.includes('node:'), 'browser entry must be node-free');
});

test('re-initializing with a different wasm source is reported', async () => {
	// The default instance is already live by now; asking for a different one
	// must fail loudly rather than silently keep serving the first.
	await assert.rejects(
		() => initialize(new Uint8Array([0x00, 0x61, 0x73, 0x6d])),
		/already initialized from a different wasm source/,
	);
	// A repeat call with no argument still resolves against the live instance.
	await initialize();
	assert.equal((await format('$x=1')).text, '$x = 1');
});

test('both entry points refuse a conflicting re-initialization', () => {
	for (const entry of ['../index.js', '../index.node.js']) {
		const src = readFileSync(new URL(entry, import.meta.url), 'utf8');
		assert.match(
			src,
			/already initialized from a different wasm source/,
			`${entry} must reject a conflicting wasm source`,
		);
	}
});
