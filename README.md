# PowerShell Formatter

A fast, portable PowerShell formatter designed around a small Rust core rather than a PowerShell runspace or `System.Management.Automation` runtime dependency.

The project is intentionally split by responsibility while remaining a single workspace:

```text
crates/powershell-parser          lexical scanner and shallow syntax model
crates/powershell-formatter       formatting policy and layout engine
crates/dprint-plugin-powershell   thin dprint/WASM adapter
cli/psfmt                         native stdin/stdout formatter
packages/formatter                browser/Node.js WASM package boundary
tests/corpus                      real-world PowerShell fixtures
tests/pssa-parity                 Invoke-Formatter compatibility fixtures
tests/powershell-oracle           differential tests against PowerShell's parser
fuzz                              scanner/formatter fuzz targets
```

## Goals

- No PowerShell process, runspace, CLR host, or `System.Management.Automation` dependency in distributed formatter artifacts.
- Parse once, format once. Formatting stages share a single token/structural representation.
- Preserve comments, strings, here-strings, source encoding semantics, and syntactically significant trivia.
- Ship the same core as a native CLI, dprint plugin, and browser/Node.js WASM package.
- Use PowerShell and PSScriptAnalyzer as compatibility oracles in development and CI, not runtime dependencies.

## Status

Early scaffold. The public APIs and package boundaries are intentionally small; formatting currently preserves input unchanged until the scanner and layout engine land.

## CLI contract

`psfmt` is a Unix-style filter: PowerShell source enters on stdin and formatted PowerShell leaves on stdout.

```sh
psfmt < script.ps1 > formatted.ps1
```

Diagnostics belong on stderr and formatting failures must return a non-zero exit code.

## Development

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## License

MIT
