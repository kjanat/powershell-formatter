// Tests the assembled package through @dprint/formatter — the same host
// stack npm consumers use. Run `make wasm-plugin` first (CI does); `node --test
// test/` executes these. Mirrors crates/dprint-plugin-pwsh/tests/e2e.sh,
// which exercises the identical artifact through the dprint CLI.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';

import { createFromBuffer } from '@dprint/formatter';

import { getBuffer, getPath } from '#js';

test('accessors agree and point at the packaged wasm', () => {
	assert.ok(getPath().endsWith('plugin.wasm'));
	assert.deepEqual(getBuffer(), readFileSync(getPath()));
});

const formatter = createFromBuffer(getBuffer());

test('plugin identity and file matching', () => {
	const info = formatter.getPluginInfo();
	assert.equal(info.name, 'dprint-plugin-pwsh');
	assert.equal(info.configKey, 'pwsh');
	// File matching comes out of config resolution, so it needs a config.
	formatter.setConfig({}, {});
	assert.deepEqual(formatter.getFileMatchingInfo().fileExtensions, [
		'ps1',
		'psm1',
		'psd1',
	]);
});

test('formats the baseline example, idempotently', () => {
	const request = {
		filePath: 'sample.ps1',
		fileText: 'function foo {\n"hello"\n  }\n',
	};
	const once = formatter.formatText(request);
	assert.equal(once, 'function foo {\n    "hello"\n}\n');
	assert.equal(formatter.formatText({ ...request, fileText: once }), once);
});

test('config is honored', () => {
	formatter.setConfig({}, { indentWidth: 2 });
	assert.deepEqual(formatter.getConfigDiagnostics(), []);
	const formatted = formatter.formatText({
		filePath: 'indent.ps1',
		fileText: 'if ($x) {\n1\n}\n',
	});
	assert.equal(formatted, 'if ($x) {\n  1\n}\n');
	formatter.setConfig({}, {});
});

test('unknown config keys surface as diagnostics', () => {
	formatter.setConfig({}, { frobnicate: true });
	const diagnostics = formatter.getConfigDiagnostics();
	assert.equal(diagnostics.length, 1);
	assert.equal(diagnostics[0]?.propertyName, 'frobnicate');
	formatter.setConfig({}, {});
});

test('malformed input reports a diagnostic error instead of output', () => {
	assert.throws(
		() =>
			formatter.formatText({
				filePath: 'broken.ps1',
				fileText: "function f {\n 'x'\n",
			}),
		/formatting-skipped/,
	);
});
