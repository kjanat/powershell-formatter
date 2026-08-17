// The publishable file set is contractual: a missing plugin.wasm or an
// accidentally-included extra would otherwise surface only after an
// (immutable) npm publish. `npm pack --dry-run` reports exactly what
// `npm publish` would ship.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';

test('npm pack manifest is exactly the contract', () => {
	const [report] = JSON.parse(
		execFileSync('npm', ['pack', '--dry-run', '--json'], {
			cwd: fileURLToPath(new URL('..', import.meta.url)),
			encoding: 'utf8',
		}),
	);
	assert.deepEqual(report.files.map((f) => f.path).sort(), [
		'LICENSE',
		'README.md',
		'index.d.ts',
		'index.js',
		'package.json',
		'plugin.wasm',
	]);
});
