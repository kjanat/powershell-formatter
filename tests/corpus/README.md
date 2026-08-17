# Real-world corpus

Fixtures under `files/` feed the invariant tests in
`crates/pwsh-formatter/tests/corpus.rs`: no panics, losslessness of the
scanner, deterministic and idempotent formatting, and preservation of
protected content (strings, here-strings, comments) plus the significant
token fingerprint.

## Provenance

| File                         | Source                                                                        | License |
| ---------------------------- | ----------------------------------------------------------------------------- | ------- |
| `pssa-build.ps1`             | PowerShell/PSScriptAnalyzer `build.ps1`                                       | MIT     |
| `pssa-resx.ps1`              | PowerShell/PSScriptAnalyzer `New-StronglyTypedCsFileForResx.ps1`              | MIT     |
| `pssa-commanddata.ps1`       | PowerShell/PSScriptAnalyzer `Utils/New-CommandDataFile.ps1`                   | MIT     |
| `pssa-CodeFormatting.psd1`   | PowerShell/PSScriptAnalyzer `Engine/Settings/CodeFormatting.psd1`             | MIT     |
| `pwsh-Parsing.Tests.ps1`     | PowerShell/PowerShell `test/powershell/Language/Parser/Parsing.Tests.ps1`     | MIT     |
| `pwsh-Ast.Tests.ps1`         | PowerShell/PowerShell `test/powershell/Language/Parser/Ast.Tests.ps1`         | MIT     |
| `pwsh-Conversions.Tests.ps1` | PowerShell/PowerShell `test/powershell/Language/Parser/Conversions.Tests.ps1` | MIT     |
| `pathological.ps1`           | hand-written for this repository                                              | MIT     |

The `tests/powershell-oracle/inputs/` and `tests/pssa-parity/inputs/` files
also participate in the corpus test via directory globbing.
