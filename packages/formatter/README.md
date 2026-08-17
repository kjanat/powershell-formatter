# pwsh-formatter

Standalone [PowerShell formatter](https://github.com/kjanat/powershell-formatter)
for browsers and Node.js: WebAssembly, no PowerShell installation required,
zero runtime dependencies. Defaults reproduce PSScriptAnalyzer's
`CodeFormatting` preset.

Formatting with the dprint toolchain instead? Use
[`dprint-plugin-pwsh`](https://www.npmjs.com/package/dprint-plugin-pwsh)
(or `dprint add kjanat/pwsh` from the CLI).

Also on JSR as
[`@kjanat/pwsh-formatter`](https://jsr.io/@kjanat/pwsh-formatter) — same
version, same API, one entry point that loads the wasm from disk or by
fetch depending on the runtime (`deno add jsr:@kjanat/pwsh-formatter`).

## Usage

```js
import { format } from "pwsh-formatter";

const result = await format("function foo {\n'hi'\n  }\n", {
	indentWidth: 4,
});
console.log(result.text);
```

`format` never throws on bad PowerShell: when the input can't be formatted
safely, `result.formatted` is `false`, `result.text` is the input unchanged,
and `result.diagnostics` says why (stable codes, 1-based line/column).

## API

- `format(source, options?, catalog?)` — format a whole source string.
- `formatRange(source, range, options?, catalog?)` — format only a 1-based
  line/column range (like `Invoke-Formatter -Range`), leaving the rest
  byte-identical.
- `defaultOptions()` — the complete default configuration.
- `version()` — the formatter's version.
- `initialize(input?)` — optional explicit runtime init; in the browser,
  `input` may be a URL, `Response`, or `WebAssembly.Module` to load the
  wasm from a custom location.

All options and types are documented in the package's TypeScript
declarations (`FormatOptions` mirrors the Rust option model; casing options
accept a `CommandCatalog` for canonical command/parameter casing).

## Runtimes

One API shape everywhere: the `node` export condition loads the wasm from
disk, the default (browser) entry fetches it. Node.js ≥ 18.19.
