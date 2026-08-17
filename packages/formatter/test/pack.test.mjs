// The publishable file set is contractual: a missing README or wasm would
// otherwise surface only after an immutable npm publish—the initial release
// shipped without a README exactly this way. `npm pack --dry-run` reports exactly
// what `npm publish` would ship.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { dirname } from "node:path";
import { test } from 'node:test';


test('npm pack manifest is exactly the contract', () => {
	/** @type {{ files: { path: string }[] }[]} */
	const reports = JSON.parse(
		execFileSync('npm', ['pack', '--dry-run', '--ignore-scripts', '--json'], {
			cwd: dirname(import.meta.dirname),
			encoding: 'utf8',
		}),
	);
	const [report] = reports;
	assert.ok(report);
	assert.deepEqual(report.files.map((f) => f.path).sort(), [
		'LICENSE',
		'README.md',
		'dist/pwsh_formatter_wasm.d.ts',
		'dist/pwsh_formatter_wasm.js',
		'dist/pwsh_formatter_wasm_bg.wasm',
		'dist/pwsh_formatter_wasm_bg.wasm.d.ts',
		'index.d.ts',
		'index.js',
		'index.node.js',
		'package.json',
	]);
});
