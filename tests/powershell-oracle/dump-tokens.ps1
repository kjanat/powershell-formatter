# Dumps a normalized token stream for a PowerShell source file as JSON.
#
# Used to (re)generate the fixtures under fixtures/. The normalized categories
# deliberately blur distinctions the formatter does not need (see
# docs/oracles.md); the Rust test in
# crates/pwsh-parser/tests/oracle.rs applies the same normalization to
# the Rust lexer's output and compares the two streams.
#
# Usage:
#   pwsh -NoProfile -File dump-tokens.ps1 -Path input.ps1 [-OutPath out.json]
#
# Regenerated with the pinned PowerShell version recorded in fixtures/VERSION.
[CmdletBinding()]
param(
    [Parameter(Mandatory)] [string] $Path,
    [string] $OutPath
)

Set-StrictMode -Version 3
$ErrorActionPreference = 'Stop'

$source = [System.IO.File]::ReadAllText($Path)
$tokens = $null
$errors = $null
[void][System.Management.Automation.Language.Parser]::ParseInput($source, [ref]$tokens, [ref]$errors)

$kw = [System.Management.Automation.Language.TokenFlags]::Keyword
$cmdName = [System.Management.Automation.Language.TokenFlags]::CommandName

function Normalize([System.Management.Automation.Language.Token] $t) {
    $k = $t.Kind
    switch ($k) {
        'Comment'              { return 'comment' }
        'Variable'             { return 'variable' }
        'SplattedVariable'     { return 'splat' }
        'StringLiteral'        { return 'string1' }
        'StringExpandable'     { return 'string2' }
        'HereStringLiteral'    { return 'herestring1' }
        'HereStringExpandable' { return 'herestring2' }
        'Number'               { return 'number' }
        'Parameter'            { return 'parameter' }
        'Generic'              { return 'word' }
        'Identifier'           { return 'word' }
        'Label'                { return 'label' }
        'LParen'               { return 'lparen' }
        'RParen'               { return 'rparen' }
        'LCurly'               { return 'lcurly' }
        'RCurly'               { return 'rcurly' }
        'LBracket'             { return 'lbracket' }
        'RBracket'             { return 'rbracket' }
        'AtParen'              { return 'atparen' }
        'AtCurly'              { return 'atcurly' }
        'DollarParen'          { return 'dollarparen' }
        'Semi'                 { return 'semi' }
        'Comma'                { return 'comma' }
        'Pipe'                 { return 'pipe' }
        'AndAnd'               { return 'andand' }
        'OrOr'                 { return 'oror' }
        'NewLine'              { return $null }
        'LineContinuation'     { return $null }
        'EndOfInput'           { return $null }
        'Unknown'              { return 'unknown' }
        default {
            # Keyword kinds: a keyword used as a command name is a word.
            if ($t.TokenFlags.HasFlag($kw) -or ($k -ge 'Begin' -and $t.TokenFlags.HasFlag($cmdName))) {
                if ($t.TokenFlags.HasFlag($cmdName)) { return 'word' }
                return 'keyword'
            }
            if ($t.TokenFlags.HasFlag($cmdName)) { return 'word' }
            return 'op'
        }
    }
}

$stream = foreach ($t in $tokens) {
    $cat = Normalize $t
    if ($null -ne $cat) {
        [pscustomobject]@{ k = $cat; t = $t.Text }
    }
}

# No per-file pwsh stamp: the generating version lives in fixtures/VERSION
# alone, so fixture content stays byte-identical across runner patch
# releases and the CI drift check compares tokens, not metadata.
$result = [pscustomobject]@{
    file   = [System.IO.Path]::GetFileName($Path)
    errors = @($errors).Count
    tokens = @($stream)
}

$json = $result | ConvertTo-Json -Depth 5 -Compress
if ($OutPath) {
    [System.IO.File]::WriteAllText($OutPath, $json)
} else {
    $json
}
