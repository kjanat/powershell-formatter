# Architecture

```text
   powershell-parser
           ▲
           │
  powershell-formatter
    ▲      ▲       ▲
    │      │       │
psfmt   dprint   JS/WASM
       plugin    package
```

## powershell-parser

A **lossless, formatter-oriented** scanner plus shallow structural parser.
Every input byte belongs to exactly one token; concatenating token texts
reproduces the source. Spans are UTF-8 byte offsets; `LineIndex` translates
them to the 1-based line / UTF-16-column positions PowerShell extents use
(verified differentially — an astral emoji advances a pwsh column by two).

The scanner is mode-driven, approximating the decisions PowerShell's
parser-driven tokenizer makes: command-name position, command-argument
position, expression positions, hash-key position, signature bodies
(class/enum), `using` directives, and member-name scanning. The devious
parts — generic-token rescan fallbacks, number-termination rules per mode,
`ForceStartNewToken` character classes, here-string column-0 footers, naive
`$( )` paren counting inside strings, `--%` verbatim arguments, signed
numbers, ternary interactions with variable name characters — were encoded
from PowerShell's own `tokenizer.cs`/`CharTraits.cs` semantics and are
pinned by differential fixtures in [`tests/powershell-oracle`](../tests/powershell-oracle) (see
[oracles.md](oracles.md)).

The structural pass never re-lexes. In one linear walk it produces a shallow
tree of delimiter groups (script block / hashtable / paren / subexpression /
array / bracket), a symmetric open↔close match table, statement boundaries,
and statement classification. There is deliberately no execution-grade AST:
the formatter needs nesting and boundaries, not semantics.

## powershell-formatter

**Parse once, format once.** The engine splits the token stream into
significant tokens and the *gaps* (trivia runs) between them. Formatting
rules never edit text; they update per-gap layout state:

```text
source
  ↓ one lexical/structural analysis
lossless tokens + structure
  ↓ phases update gap decisions (Join{n spaces} / Break / indent level)
     and token respellings (casing)
one layout state
  ↓ one render
formatted source
```

Phases run in PSScriptAnalyzer's rule order — close brace, open brace,
whitespace, width-aware reflow, indentation, alignment, casing — each
reading the layout state left by its predecessors. That reproduces
`Invoke-Formatter`'s parse→fix fixpoint without reparsing *within a pass*;
the verification steps below may re-scan the rendered output afterwards,
and in rare cases trigger one more whole-file pass.

Comments and blank lines live inside gaps and are preserved by the renderer;
strings, here-strings, and `--%` arguments are opaque single tokens and are
never touched. A post-format verification re-scans the output and returns
the input unchanged if any protected content would differ (this can only
trigger on structurally odd input where moving tokens across lines flips
mode-dependent lexing).

Idempotence is enforced, not assumed: PowerShell token classes depend on
layout (`=` after a command name is an argument; at statement start it is
an operator), so a layout change can hand a re-format a different token
stream. When whole-file formatting changes the text, the engine verifies
the result is a fixpoint, stepping once more if reclassification moved it;
input that never stabilizes (not observed outside adversarial fuzzing) is
returned unchanged with a diagnostic. Already-formatted input pays for no
extra pass. Range formatting is exempt — the range names coordinates in
the original text, so only one pass is meaningful.

Range formatting falls out of the model: each gap decision and respelling
has an original span, and rendering applies only those inside the requested
range, emitting original bytes elsewhere.

Malformed input policy: unbalanced delimiters or unterminated strings mark
the parse incomplete and the input is returned byte-identical, with
diagnostics.

## Adapters

- [`cli/psfmt`](../cli/psfmt) — argument parsing, file I/O (atomic `--write`), exit codes.
- [`crates/dprint-plugin-powershell`](../crates/dprint-plugin-powershell) — dprint Wasm ABI + config resolution.
  Known config keys are derived from `FormatOptions`' own serde surface, so
  the plugin cannot drift from the core.
- [`crates/powershell-formatter-wasm`](../crates/powershell-formatter-wasm) + [`packages/formatter`](../packages/formatter) — wasm-bindgen
  boundary and the npm package (browser + Node entries).

Adapters contain no formatting policy.

## Safety

`#![forbid(unsafe_code)]` across the workspace. Formatting never executes
code, touches the network, or reads the filesystem (the CLI reads exactly
the files it is told to). Command/parameter casing data is injected via
`CommandCatalog`; no runspace, ever.
