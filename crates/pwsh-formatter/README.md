# pwsh-formatter

`pwsh-formatter` is a standalone PowerShell formatting library that reproduces
PSScriptAnalyzer's `Invoke-Formatter` behavior without PowerShell, runspaces, or
the .NET runtime.

## Usage

```rust
use pwsh_formatter::{FormatOptions, format};

let result = format("if($value){'yes'}", &FormatOptions::default());
assert!(result.diagnostics.is_empty());
println!("{}", result.text);
```

The formatter is deterministic and idempotent. Strings, here-strings, and
comments are protected, and unsafe transformations fall back to the original
input with diagnostics.

Optional crate features:

- `serde` (default) enables serializable configuration and JSON helpers.
- `schema` enables JSON Schema generation for formatter options.

See the [project README](../../README.md) for CLI, dprint, and WebAssembly usage.
