#!/usr/bin/env bash
# Builds the wasm-bindgen package into dist/.
# Requires: wasm32-unknown-unknown target, wasm-bindgen-cli matching the
# wasm-bindgen version pinned in crates/powershell-formatter-wasm.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
out_dir="$(cd "$(dirname "$0")" && pwd)/dist"

cargo build -p powershell-formatter-wasm --profile wasm-release \
	--target wasm32-unknown-unknown --manifest-path "${repo_root}/Cargo.toml"

wasm-bindgen \
	--target web \
	--out-dir "${out_dir}" \
	--no-typescript \
	"${repo_root}/target/wasm32-unknown-unknown/wasm-release/powershell_formatter_wasm.wasm"

ls -la "${out_dir}"
