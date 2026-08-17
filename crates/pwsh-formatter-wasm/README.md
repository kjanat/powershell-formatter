# pwsh-formatter-wasm

`pwsh-formatter-wasm` provides `wasm-bindgen` bindings for the
`pwsh-formatter` Rust library. It exposes full-document and range formatting,
default options, and the formatter version to browser and Node.js consumers.

This crate is an internal build target and is not published directly to
crates.io. Its WebAssembly output is packaged by the repository's JavaScript
package.

For JavaScript installation and usage, see the [project README](../../README.md). For package build details, see
[`packages/formatter`](../../packages/formatter/).
