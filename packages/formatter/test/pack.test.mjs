// The publishable file set is contractual: a missing README or wasm would
// otherwise surface only after an (immutable) npm publish — 0.0.0 shipped
// without a README exactly this way. `npm pack --dry-run` reports exactly
// what `npm publish` would ship.
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
		'dist/pwsh_formatter_wasm.js',
		'dist/pwsh_formatter_wasm_bg.wasm',
		'index.d.ts',
		'index.js',
		'index.node.js',
		'package.json',
	]);
});
