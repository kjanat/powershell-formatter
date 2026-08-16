#!/usr/bin/env bash
# End-to-end test: build the Wasm plugin and drive it with the real dprint
# binary. Requires `dprint` on PATH and the wasm32-unknown-unknown target.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "${repo_root}"

cargo build -p dprint-plugin-powershell --profile wasm-release \
	--target wasm32-unknown-unknown
wasm="${repo_root}/target/wasm32-unknown-unknown/wasm-release/dprint_plugin_powershell.wasm"
[[ -f "${wasm}" ]]

workdir="$(mktemp -d)"
trap 'rm -rf "${workdir}"' EXIT
cp "${wasm}" "${workdir}/plugin.wasm"

cat >"${workdir}/dprint.json" <<EOF
{
  "powershell": { "indentWidth": 4 },
  "plugins": ["./plugin.wasm"]
}
EOF

# shellcheck disable=SC2016
{
	printf 'function foo {\n"hello"\n  }\n' >"${workdir}/sample.ps1"
	printf 'IF($x-EQ 1){'"'"'yes'"'"'}ELSE{'"'"'no'"'"'}\n' >"${workdir}/casing.ps1"
}
cd "${workdir}"

# 1. `dprint check` must fail on unformatted input.
if dprint check sample.ps1 casing.ps1 >/dev/null 2>&1; then
	echo "FAIL: dprint check passed on unformatted input" >&2
	exit 1
fi

# 2. `dprint fmt` rewrites the files.
dprint fmt sample.ps1 casing.ps1

sample_content="$(cat sample.ps1)"
expected_sample='function foo {
    "hello"
}'
if [[ "${sample_content}" != "${expected_sample}" ]]; then
	echo "FAIL: sample.ps1 not formatted as expected:" >&2
	printf '%s\n' "${sample_content}" >&2
	exit 1
fi

casing_content="$(cat casing.ps1)"
# The glued `}else {` is PSScriptAnalyzer's own output for this input —
# Invoke-Formatter 1.25.0 produces exactly this across the relevant
# presets, and the parity fixtures pin it.
expected_casing="if (\$x -eq 1) { 'yes' }else { 'no' }"
if [[ "${casing_content}" != "${expected_casing}" ]]; then
	echo "FAIL: casing.ps1 not formatted as expected:" >&2
	printf '%s\n' "${casing_content}" >&2
	exit 1
fi

# 3. Idempotence: check now passes, and a second fmt is a no-op.
dprint check sample.ps1 casing.ps1
before="$(cat sample.ps1 casing.ps1)"
dprint fmt sample.ps1 casing.ps1
after="$(cat sample.ps1 casing.ps1)"
[[ "${before}" == "${after}" ]]

# 4. Config is honored (indentWidth 2).
cat >dprint.json <<EOF
{
  "powershell": { "indentWidth": 2 },
  "plugins": ["./plugin.wasm"]
}
EOF

# shellcheck disable=SC2016
printf 'if ($x) {\n1\n}\n' >indent.ps1
dprint fmt indent.ps1
# shellcheck disable=SC2016
expected_indent='if ($x) {
  1
}'

indent_content="$(cat indent.ps1)"
if [[ "${indent_content}" != "${expected_indent}" ]]; then
	echo "FAIL: indent.ps1 not formatted with indentWidth 2:" >&2
	printf '%s\n' "${indent_content}" >&2
	exit 1
fi

# 5. Unknown config keys surface as diagnostics. Keep indentWidth 2 so the
# file itself is still correctly formatted: the failure must come from the
# unknown key, not from an indentation mismatch.
cat >dprint.json <<EOF
{
  "powershell": { "indentWidth": 2, "frobnicate": true },
  "plugins": ["./plugin.wasm"]
}
EOF
if output="$(dprint check indent.ps1 2>&1)"; then
	echo "FAIL: unknown config key did not fail" >&2
	exit 1
fi
if [[ "${output}" != *"frobnicate"* ]]; then
	echo "FAIL: failure was not caused by the unknown config key:" >&2
	printf '%s\n' "${output}" >&2
	exit 1
fi

echo "e2e OK"
