# Oracles: PowerShell and PSScriptAnalyzer as ground truth

Neither PowerShell nor PSScriptAnalyzer is a runtime dependency of anything
this repository ships. Both are **development/CI oracles**: pinned versions
generate deterministic fixtures that ordinary `cargo nextest run` consumes without
PowerShell installed.

## tests/powershell-oracle — the tokenizer oracle

`dump-tokens.ps1` parses an input with
`[System.Management.Automation.Language.Parser]::ParseInput` and emits a
normalized `(category, text)` token stream as JSON. The Rust side
([`crates/pwsh-parser/tests/oracle.rs`](../crates/pwsh-parser/tests/oracle.rs)) applies the same normalization
to our lexer's output and requires the sequences to match exactly — which
pins token boundaries and classifications simultaneously without comparing
offsets across UTF-8/UTF-16 conventions.

Normalization deliberately blurs distinctions the formatter does not need:
`Identifier`/`Generic` (and keyword-used-as-command-name) collapse to
`word`; all operators collapse to `op`; newline/continuation trivia is
skipped; `--%` maps to `word` (PowerShell lexes it as a Generic token).

- Regenerate: `pwsh -NoProfile -File tests/powershell-oracle/generate.ps1`
- Pinned version: [`tests/powershell-oracle/fixtures/VERSION`](../tests/powershell-oracle/fixtures/VERSION)
- Inputs: hand-written coverage files (strings, numbers, commands, syntax,
  edge cases, ternaries, CRLF) plus upstream PowerShell parser test files
  (MIT; provenance in the file names).

When our lexer and pwsh disagree, **pwsh wins**; behavior gets fixed and
usually a targeted probe becomes a new fixture.

## tests/pssa-parity — the Invoke-Formatter oracle

`generate.ps1` runs real `Invoke-Formatter` over every input under a matrix
of settings profiles (the four presets plus one-line-block expansion,
tabs + `NoIndentation`, per-rule-only profiles, redundant-pipe collapse)
and stores the results under `expected/`. `generate-catalog.ps1` dumps
canonical command/parameter casing for the commands the fixtures use from
the *same* pwsh session, so our catalog-driven command casing is tested
against PSSA's runspace-driven casing.

[`crates/pwsh-formatter/tests/pssa_parity.rs`](../crates/pwsh-formatter/tests/pssa_parity.rs) maps each profile to
`FormatOptions`, formats each input, and requires byte equality — plus that
formatting the oracle's own output is a no-op (idempotence against the
oracle, not against ourselves).

Expected outputs are never generated with our formatter.

- Regenerate: `pwsh -NoProfile -File tests/pssa-parity/generate.ps1`
- Pinned version: [`tests/pssa-parity/expected/VERSION`](../tests/pssa-parity/expected/VERSION) (PSScriptAnalyzer)

## CI

The `powershell-oracles` job regenerates all fixtures with the runner's
PowerShell + freshly installed PSScriptAnalyzer and re-runs the
differential tests — so drift between the pinned fixtures and current
upstream is detected (reported by a `git diff --stat`, failing the
differential tests if behavior actually changed).

## Positions

PowerShell extents are 1-based lines with 1-based **UTF-16 code unit**
columns (.NET string indices). `LineIndex` reproduces exactly that; the
Unicode fixtures (astral emoji, CJK, combining accents) pin the behavior.
