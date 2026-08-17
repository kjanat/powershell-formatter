#!/usr/bin/env bash
# Builds the wasm-bindgen package into dist/.
# Requires: wasm32-unknown-unknown target, wasm-bindgen-cli matching the
# wasm-bindgen version pinned in crates/pwsh-formatter-wasm.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
out_dir="$(cd "$(dirname "$0")" && pwd)/dist"

# A wasm-bindgen-cli that differs from the pinned crate version produces
# bindings that fail at load time with a confusing error; fail here instead.
pinned="$(sed -n 's/^wasm-bindgen *= *"=\([0-9.]*\)"$/\1/p' "${repo_root}/Cargo.toml")"
actual="$(wasm-bindgen --version | awk '{print $2}')"
if [[ -z "${pinned}" || "${pinned}" != "${actual}" ]]; then
	echo "wasm-bindgen-cli ${actual} does not match the pinned wasm-bindgen ${pinned:-<unparsed>}" >&2
	exit 1
fi

# Stale artifacts in dist/ would otherwise be published with the package.
rm -rf "${out_dir}"

cargo build -p pwsh-formatter-wasm --profile wasm-release \
	--target wasm32-unknown-unknown --manifest-path "${repo_root}/Cargo.toml"

wasm-bindgen \
	--target web \
	--out-dir "${out_dir}" \
	--no-typescript \
	"${repo_root}/target/wasm32-unknown-unknown/wasm-release/pwsh_formatter_wasm.wasm"

ls -la "${out_dir}"
