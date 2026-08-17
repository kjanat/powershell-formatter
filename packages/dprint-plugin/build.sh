#!/usr/bin/env bash
# Assembles the package: builds the dprint-ABI Wasm artifact and copies it in
# as plugin.wasm. The copy is generated output — gitignored, never committed;
# the release workflow and CI always rebuild it from source.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
pkg_dir="$(cd "$(dirname "$0")" && pwd)"

cargo build -p dprint-plugin-powershell --profile wasm-release \
	--target wasm32-unknown-unknown --manifest-path "${repo_root}/Cargo.toml"

cp "${repo_root}/target/wasm32-unknown-unknown/wasm-release/dprint_plugin_powershell.wasm" \
	"${pkg_dir}/plugin.wasm"
ls -la "${pkg_dir}/plugin.wasm"
