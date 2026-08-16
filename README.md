# PowerShell Formatter

A fast, portable PowerShell formatter built around a small Rust core — no
PowerShell process, no runspace, no CLR, no `System.Management.Automation`.
The same engine ships as a native CLI (`psfmt`), a dprint Wasm plugin, and a
browser/Node.js WebAssembly package.

Formatting behavior reproduces PSScriptAnalyzer's `Invoke-Formatter`
(`CodeFormatting`/OTBS/Allman/Stroustrup presets) and is verified
byte-for-byte against the real module; the previous WASM approach that
dragged a .NET runtime along weighed ~19 MB — this one is **282 KB** (105 KB
gzipped).

```powershell
# before
IF($x-EQ 1){'yes'}ELSE{'no'}

# after (default preset)
if ($x -eq 1) { 'yes' }else { 'no' }   # exactly what Invoke-Formatter produces
```

## Using it

**CLI**

```sh
psfmt < script.ps1 > formatted.ps1     # stdin → stdout filter
psfmt --write src/**/*.ps1             # atomic in-place
psfmt --check src/                     # CI gate (exit 1 on changes)
psfmt --preset allman --config fmt.json --catalog commands.json
psfmt --range 10,1,20,1 < script.ps1   # Invoke-Formatter -Range semantics
```

Formatted source is the only thing on stdout; diagnostics go to stderr;
malformed input passes through unchanged with exit code 4 — an editor
buffer is never corrupted.

**dprint** (`dprint.json`)

```jsonc
{
	"powershell": { "indentWidth": 4, "braceStyle": "sameLine" },
	"plugins": ["<url-or-path-to>/plugin.wasm"]
}
```

**Browser / Node.js**

```js
import { format } from '@kjanat/powershell-formatter';

const result = await format("if($x){'y'}", { indentWidth: 2 });
result.text; // "if ($x) {'y'}"
result.diagnostics; // []
```

The runtime is instantiated once and cached; the browser entry has no Node
dependencies.

## How it works

One lossless lexical + structural analysis, one set of layout decisions,
one render — never six passes to indent a brace. Strings, here-strings, and
comments are opaque protected spans; a post-format verification guarantees
they survive byte-for-byte or the input is returned untouched. Formatting is
deterministic and idempotent (`format(format(x)) == format(x)`), enforced by
unit, corpus, property, and fuzz tests.

The lexer mirrors PowerShell's own devious tokenizer (generic-token rescan
fallbacks, mode-dependent numbers, here-string column-0 terminators, `--%`,
`$true?2:3` being a single variable...) and is differential-tested against
pinned `pwsh` fixtures. Where PowerShell and this repository disagree,
PowerShell wins. Where `Invoke-Formatter` disagrees with its own source
code, the shipped binary wins — see [docs/pssa-quirks.md](docs/pssa-quirks.md)
for the upstream oddities discovered along the way, and
[docs/formatting.md](docs/formatting.md) for the short list of intentional
divergences (all safety- or idempotence-motivated).

More: [architecture](docs/architecture.md) ·
[configuration](docs/configuration.md) · [oracles](docs/oracles.md) ·
[releasing](docs/releasing.md)

## Performance

Measured on x86_64-linux (divan benchmarks, `cargo bench -p powershell-formatter`):

| Input                       | Full format | Throughput |
| --------------------------- | ----------- | ---------- |
| tiny (24 B)                 | 2.4 µs      | —          |
| medium (4.6 KB real script) | 220 µs      | 21 MB/s    |
| large (614 KB real scripts) | 60 ms       | 10 MB/s    |
| here-string heavy (220 KB)  | 665 µs      | 331 MB/s   |

Numbers include the idempotence-verification pass (a changed output is
re-formatted once to confirm it is a fixpoint — see
[architecture](docs/architecture.md)); already-formatted input skips it.

Against the oracle on the same medium script: warm `Invoke-Formatter` needs
≈21 ms per format in an already-running PowerShell; `psfmt` needs ≈2.7 ms
**per process invocation** (spawn + read + format + write), ≈220 µs of which
is formatting. Cold start: `pwsh -NoProfile` + module import + one format
≈590 ms; `psfmt` ≈3 ms — editors can invoke it directly, no daemon needed.

Artifact sizes: `psfmt` 659 KB; dprint plugin 282 KB (105 KB gz); browser
wasm 188 KB (74 KB gz); npm tarball 82 KB.

## Development

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace                 # unit + oracle + parity + corpus + property
crates/dprint-plugin-powershell/tests/e2e.sh   # real dprint end-to-end
packages/formatter/build.sh && (cd packages/formatter && npm test)
cargo +nightly fuzz run formatter      # see fuzz/README.md
cargo bench -p powershell-formatter
```

Regenerating oracle fixtures needs `pwsh` + PSScriptAnalyzer (pinned
versions recorded next to the fixtures); normal `cargo test` does not.

## Security model

Formatting is syntax transformation, not evaluation: no code execution, no
network, no module imports, no filesystem access beyond the files the CLI is
told to read. `#![forbid(unsafe_code)]` across the workspace. Command
casing data is injected as JSON — never obtained from a live shell. See
[SECURITY.md](SECURITY.md).

## License

MIT. Test fixtures derived from MIT-licensed upstream projects record their
provenance in [`tests/corpus/README.md`](tests/corpus/README.md).
