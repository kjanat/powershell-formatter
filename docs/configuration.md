# Configuration

One conceptual model — the Rust `FormatOptions` struct — drives every
surface:

- **Rust**: `powershell_formatter::FormatOptions` (strong enums, serde
  camelCase).
- **CLI**: `psfmt --config file.json` deserializes the same camelCase JSON;
  `--preset default|otbs|allman|stroustrup` selects a preset.
- **dprint**: the `"powershell"` key in `dprint.json`. Known keys are
  derived from the serde surface of `FormatOptions` at runtime, so the
  plugin cannot accept keys the core does not have; unknown keys and
  invalid values produce configuration diagnostics.
- **JSON Schema**: [`crates/dprint-plugin-powershell/deployment/schema.json`](../crates/dprint-plugin-powershell/deployment/schema.json),
  generated from the same type via `schemars`
  (`cargo run -p dprint-plugin-powershell --features schema --bin generate-schema`);
  CI fails if it drifts.
- **JS/TS**: [`packages/formatter`](../packages/formatter) accepts the same camelCase object; its
  `index.d.ts` is validated in the package tests against the keys the wasm
  actually exposes (`defaultOptions()`).

## Options

Defaults reproduce PSScriptAnalyzer's `CodeFormatting` preset.

| Key                              | Type                           | Default                                 | Meaning                                                                     |
| -------------------------------- | ------------------------------ | --------------------------------------- | --------------------------------------------------------------------------- |
| `indentWidth`                    | number                         | `4`                                     | Spaces per indent level                                                     |
| `useTabs`                        | bool                           | `false`                                 | One tab per level instead                                                   |
| `lineWidth`                      | number                         | `120`                                   | Width-aware pipeline reflow target; `0` disables                            |
| `placeOpenBrace`                 | bool                           | `true`                                  | Run the open-brace rule                                                     |
| `placeCloseBrace`                | bool                           | `true`                                  | Run the close-brace rule                                                    |
| `braceStyle`                     | `"sameLine"` \| `"nextLine"`   | `"sameLine"`                            | K&R vs Allman                                                               |
| `newlineAfterOpenBrace`          | bool                           | `true`                                  | `{` must end its line                                                       |
| `ignoreOneLineBlock`             | bool                           | `true`                                  | Leave one-line `{ ... }` alone                                              |
| `branchKeywordPlacement`         | `"nextLine"` \| `"cuddled"`    | `"nextLine"`                            | `}` `else` vs `} else`                                                      |
| `noEmptyLineBeforeCloseBrace`    | bool                           | `false`                                 | Strip blank lines before `}`                                                |
| `spaceBeforeOpenBrace`           | bool                           | `true`                                  | `if (...) {`                                                                |
| `spaceInsideBrace`               | bool                           | `true`                                  | `{ 'x' }` on one-line blocks                                                |
| `spaceAfterKeyword`              | bool                           | `true`                                  | `if (` after if/elseif/switch/for/foreach/while                             |
| `spaceAroundOperator`            | bool                           | `true`                                  | `$a -eq $b`, `$x = 1`, `1 + 2`                                              |
| `spaceAfterSeparator`            | bool                           | `true`                                  | `1, 2`; `a; b`                                                              |
| `spaceAroundPipe`                | bool                           | `true`                                  | Add missing spaces around `\|`                                              |
| `collapseSpaceAroundPipe`        | bool                           | `false`                                 | Also collapse runs                                                          |
| `collapseSpaceBetweenParameters` | bool                           | `false`                                 | Collapse runs between command elements                                      |
| `ignoreAssignmentInHashtable`    | bool                           | `true`                                  | Leave `=` in multi-line hashtables to alignment                             |
| `indentation`                    | bool                           | `true`                                  | Reindent lines                                                              |
| `pipelineIndentation`            | enum                           | `"increaseIndentationForFirstPipeline"` | Also `"increaseIndentationAfterEveryPipeline"`, `"noIndentation"`, `"none"` |
| `alignAssignment`                | bool                           | `true`                                  | Align `=` in hashtables and enums                                           |
| `keywordCasing`                  | bool                           | `true`                                  | Lowercase keywords                                                          |
| `operatorCasing`                 | bool                           | `true`                                  | Lowercase `-EQ` → `-eq` etc.                                                |
| `commandCasing`                  | bool                           | `true`                                  | Use the injected catalog for commands/parameters                            |
| `endOfLine`                      | `"auto"` \| `"lf"` \| `"crlf"` | `"auto"`                                | Output newline style                                                        |
| `finalNewline`                   | bool \| null                   | `null`                                  | Force/strip final newline; `null` preserves                                 |

## Command catalogs

```json
{ "commands": { "Get-ChildItem": ["Path", "Filter", "Recurse"] } }
```

Canonical casing is taken from the spellings used. Load with
`psfmt --catalog file.json`, `JsonCatalog::from_json` in Rust, or the
`catalog` argument of the JS `format()`. A catalog can be dumped from any
PowerShell session — see [`tests/pssa-parity/generate-catalog.ps1`](../tests/pssa-parity/generate-catalog.ps1).

## PSScriptAnalyzer `.psd1` settings files

Not supported. Translating the data-only subset of a PSSA settings file
into `FormatOptions` is a possible future addition; it would parse the
`.psd1` hashtable with this repository's own parser (never executing
PowerShell) and reject dynamic expressions.
