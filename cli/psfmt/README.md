# psfmt

`psfmt` is a fast, standalone PowerShell formatter CLI. It reproduces
PSScriptAnalyzer's `Invoke-Formatter` behavior without starting PowerShell or
loading the .NET runtime.

## Installation

```sh
cargo install psfmt
```

## Usage

Use `psfmt` as a stdin-to-stdout filter, format files in place, or check whether
files are already formatted:

```sh
psfmt < script.ps1 > formatted.ps1
psfmt --write src/**/*.ps1
psfmt --check src/
psfmt --preset allman --config fmt.json
```

Formatted source is written to stdout and diagnostics are written to stderr.
Malformed input is returned unchanged so editor buffers are not corrupted.

See the [project README](../../README.md) for configuration, architecture, and
development documentation.
