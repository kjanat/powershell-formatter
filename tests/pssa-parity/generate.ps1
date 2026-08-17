# Regenerates PSScriptAnalyzer parity fixtures.
#
# For every input under inputs/ and every settings profile below, runs
# Invoke-Formatter and stores the result under expected/<input>.<profile>.ps1
# The pinned module version is recorded in expected/VERSION.
#
# Run: pwsh -NoProfile -File generate.ps1
[CmdletBinding()]
param(
	# Parity expectations are defined against one exact analyzer build, and
	# rules change between releases: generating with a different version
	# silently rewrites every expectation. Pin it, and record the pin in
	# expected/VERSION so CI installs the same build.
	[string]$RequiredVersion = '1.25.0'
)
Set-StrictMode -Version 3
$ErrorActionPreference = 'Stop'
Import-Module PSScriptAnalyzer -RequiredVersion $RequiredVersion

$root = $PSScriptRoot
$inputs = Join-Path $root 'inputs'
$expected = Join-Path $root 'expected'
New-Item -ItemType Directory -Force -Path $expected | Out-Null

# Profile name → Invoke-Formatter -Settings value. Names must match the
# option mapping in crates/pwsh-formatter/tests/pssa_parity.rs.
$profiles = [ordered]@{
    'default'    = 'CodeFormatting'
    'otbs'       = 'CodeFormattingOTBS'
    'allman'     = 'CodeFormattingAllman'
    'stroustrup' = 'CodeFormattingStroustrup'
    'expand-oneline' = @{
        IncludeRules = @('PSPlaceOpenBrace', 'PSPlaceCloseBrace', 'PSUseConsistentWhitespace', 'PSUseConsistentIndentation', 'PSAlignAssignmentStatement', 'PSUseCorrectCasing')
        Rules = @{
            PSPlaceOpenBrace = @{ Enable = $true; OnSameLine = $true; NewLineAfter = $true; IgnoreOneLineBlock = $false }
            PSPlaceCloseBrace = @{ Enable = $true; NewLineAfter = $true; IgnoreOneLineBlock = $false; NoEmptyLineBefore = $false }
            PSUseConsistentWhitespace = @{ Enable = $true; CheckInnerBrace = $true; CheckOpenBrace = $true; CheckOpenParen = $true; CheckOperator = $true; CheckPipe = $true; CheckSeparator = $true; IgnoreAssignmentOperatorInsideHashTable = $true }
            PSUseConsistentIndentation = @{ Enable = $true; Kind = 'space'; IndentationSize = 4; PipelineIndentation = 'IncreaseIndentationForFirstPipeline' }
            PSAlignAssignmentStatement = @{ Enable = $true; CheckHashtable = $true }
            PSUseCorrectCasing = @{ Enable = $true }
        }
    }
    'tabs-noindent' = @{
        IncludeRules = @('PSPlaceOpenBrace', 'PSPlaceCloseBrace', 'PSUseConsistentWhitespace', 'PSUseConsistentIndentation', 'PSAlignAssignmentStatement', 'PSUseCorrectCasing')
        Rules = @{
            PSPlaceOpenBrace = @{ Enable = $true; OnSameLine = $true; NewLineAfter = $true; IgnoreOneLineBlock = $true }
            PSPlaceCloseBrace = @{ Enable = $true; NewLineAfter = $true; IgnoreOneLineBlock = $true; NoEmptyLineBefore = $false }
            PSUseConsistentWhitespace = @{ Enable = $true; CheckInnerBrace = $true; CheckOpenBrace = $true; CheckOpenParen = $true; CheckOperator = $true; CheckPipe = $true; CheckSeparator = $true; IgnoreAssignmentOperatorInsideHashTable = $true }
            PSUseConsistentIndentation = @{ Enable = $true; Kind = 'tab'; IndentationSize = 4; PipelineIndentation = 'NoIndentation' }
            PSAlignAssignmentStatement = @{ Enable = $true; CheckHashtable = $true }
            PSUseCorrectCasing = @{ Enable = $true }
        }
    }
    'pipeline-every' = @{
        IncludeRules = @('PSUseConsistentIndentation')
        Rules = @{
            PSUseConsistentIndentation = @{ Enable = $true; Kind = 'space'; IndentationSize = 4; PipelineIndentation = 'IncreaseIndentationAfterEveryPipeline' }
        }
    }
    'pipeline-none' = @{
        IncludeRules = @('PSUseConsistentIndentation')
        Rules = @{
            PSUseConsistentIndentation = @{ Enable = $true; Kind = 'space'; IndentationSize = 4; PipelineIndentation = 'None' }
        }
    }
    'pipe-redundant' = @{
        IncludeRules = @('PSUseConsistentWhitespace')
        Rules = @{
            PSUseConsistentWhitespace = @{ Enable = $true; CheckInnerBrace = $true; CheckOpenBrace = $true; CheckOpenParen = $true; CheckOperator = $true; CheckPipe = $true; CheckPipeForRedundantWhitespace = $true; CheckSeparator = $true; CheckParameter = $true; IgnoreAssignmentOperatorInsideHashTable = $true }
        }
    }
}

foreach ($file in Get-ChildItem $inputs -Filter *.ps1) {
    $source = [System.IO.File]::ReadAllText($file.FullName)
    foreach ($name in $profiles.Keys) {
        $settings = $profiles[$name]
        try {
            $result = Invoke-Formatter -ScriptDefinition $source -Settings $settings
        } catch {
            Write-Warning "FAILED $($file.Name) / ${name}: $_"
            continue
        }
        $out = Join-Path $expected "$($file.BaseName).$name.ps1"
        [System.IO.File]::WriteAllText($out, $result)
        Write-Host "wrote $out"
    }
}

$loaded = (Get-Module PSScriptAnalyzer).Version.ToString()
if ($loaded -ne $RequiredVersion) {
	throw "expected PSScriptAnalyzer $RequiredVersion, loaded $loaded"
}
Set-Content -Path (Join-Path $expected 'VERSION') -Value $loaded
