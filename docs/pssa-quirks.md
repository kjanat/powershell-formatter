# PSScriptAnalyzer oddities observed while building the parity suite

Everything below was reproduced against PSScriptAnalyzer **1.25.0** on
pwsh **7.5.2** with `Invoke-Formatter` and default settings unless noted.
These look like genuine upstream bugs or at least surprising behavior;
recorded here because our formatter either reproduces them (for parity) or
deliberately diverges (documented in [formatting.md](formatting.md)).

## 1. `--%` verbatim arguments make Invoke-Formatter non-idempotent

```powershell
Invoke-Formatter 'cmd --% raw | text & stuff'
# → 'cmd --% raw  | text & stuff'
# run it again → 'cmd --% raw   | text & stuff'
# and again    → 'cmd --% raw    | text & stuff'
```

The verbatim argument token owns the text up to the pipe, so `CheckPipe`
sees a zero-width gap before `|` and inserts a space — into the verbatim
argument's territory — on every run. **We diverge**: spacing adjacent to a
verbatim argument is never touched.

Upstream: PowerShell/PSScriptAnalyzer#2209.

## 2. Statements after a nested pipeline stay over-indented

```powershell
$t |
ForEach-Object {
Get-Process |
Select-Object 1
$x
}
```

formats to:

```powershell
$t |
    ForEach-Object {
        Get-Process |
            Select-Object 1
            $x          # ← still at the pipeline-continuation level
        }               # ← closing brace too
```

The indentation "restore" for a pipeline's extra level only fires at the
end of the *outermost* pipeline, so `$x` — a fresh statement after the
nested `Get-Process | Select-Object` pipeline — keeps the +1 level, and the
closing `}` lands at the block-content level. This is fixed on PSSA `main`
at [PowerShell/PSScriptAnalyzer@`4b0117ca7d`](https://github.com/PowerShell/PSScriptAnalyzer/commit/4b0117ca7d2887711c9699f467ba7171f8859156);
the shipped 1.25.0 behaves as above. **We reproduce this for parity.**

## 3. Trailing whitespace is left behind when blocks expand

`if ($x) { BREAK }` (with `IgnoreOneLineBlock` off) becomes:

```text
if ($x) {\n        break \n    }
                        ^ trailing space survives
```

Corrections replace only the brace tokens, so the spaces that used to
separate content from braces stay behind as trailing whitespace.
`PSAvoidTrailingWhitespace` can remove it when explicitly configured, but
the brace rules still introduce it. **We reproduce this for parity.**

Upstream: PowerShell/PSScriptAnalyzer#2210.

## 4. Asymmetric inner-brace spacing for one-line hashtables

`@{a=1}` → `@{a = 1 }` — a space is enforced *before* `}` but not after
`@{`, because `CheckInnerBrace`'s open-side check only looks at `LCurly`
tokens and `@{` is `AtCurly`. **We reproduce this.**

Upstream: PowerShell/PSScriptAnalyzer#1742.

## 5. Values stay glued to `=` in multi-line hashtables

With the presets' `IgnoreAssignmentOperatorInsideHashTable = $true`:

```powershell
@{
    a  =1     # aligned before '=', but '=1' stays glued
    bb =2
}
```

Alignment manages the gap before `=`; nothing manages the gap after it.
**We reproduce this.**

## 6. Range formatting drops corrections as text shifts

```powershell
Invoke-Formatter -ScriptDefinition "if(`$a){'x'}`nif(`$b){'y'}" -Range @(2,1,2,12)
```

returns `if ($b) { 'y'}` for line 2 — the space before `}` is never added
because earlier fixes on the line grew it past the (fixed) range end, and
the filter re-runs each iteration against shifted coordinates. **We
diverge** (apply all corrections whose original extent is inside the
range).

Upstream: PowerShell/PSScriptAnalyzer#2211.

## 7. Mixed newlines are a hard error

`EditableText` requires perfectly uniform line endings and throws otherwise.
**We diverge** (normalize the newlines the formatter owns to the first
inter-token style seen in the input, matching [`docs/formatting.md`](formatting.md)).

## 8. Unary operators receive inconsistent whitespace

Spacing every binary operator is desirable. The problem is that
`CheckOperator` sometimes treats the same token as binary when it is being
used as a unary operator. PSScriptAnalyzer 1.25.0 produces, among others:

```powershell
return -$x                                 # -> return - $x
$a = @(-$b)                                # -> $a = @( - $b)
$a = [int]-$b                              # -> $a = [int] - $b
ConvertFrom-Json (-join (dotnet gitversion))
# -> ConvertFrom-Json ( -join (dotnet gitversion))
```

The space after unary `-` and `+`, or before unary `-join` and `-split`, is
undesirable. Conversely, word-based unary operators are handled
inconsistently: `-split$b` becomes `-split $b`, while `-not$b` is left alone.

Upstream: PowerShell/PSScriptAnalyzer#1239.

<details>
<summary>Why the token-based test behaves this way</summary>

`UseConsistentWhitespace.IsOperator` checks `AssignmentOperator`,
`BinaryPrecedenceAdd`, and `BinaryPrecedenceMultiply` through
`TokenTraits.HasTrait`. This initially looked as though it selected only three
operator categories. That interpretation was wrong.

The binary-precedence members occupy the low nibble as packed numeric values,
not as independent flag bits:

| Precedence category | Value |
| ------------------- | ----: |
| Logical             | `0x1` |
| Bitwise             | `0x2` |
| Comparison          | `0x5` |
| Coalesce            | `0x7` |
| Add                 | `0x9` |
| Multiply            | `0xA` |
| Format              | `0xC` |
| Range               | `0xD` |

`HasTrait` tests `(GetTraits(kind) & flag) != None`, so any shared bit is
enough. For example, Logical overlaps Add (`0x1 & 0x9`), Bitwise overlaps
Multiply (`0x2 & 0xA`), Comparison overlaps Add (`0x5 & 0x9`), and Format
overlaps Add (`0xC & 0x9`). Every binary-precedence value therefore overlaps
either `BinaryPrecedenceAdd` or `BinaryPrecedenceMultiply`.

This makes the predicate select every binary operator. `DotDot` is excluded
explicitly later. `AndAnd` and `OrOr` are included explicitly because they do
not carry the ordinary binary-precedence traits.

That broad selection is fine for binary use. The bug appears because operator
selection is token-based, while unary and binary uses can share a token kind.
The later unary exception only recognizes a narrow `(`, unary `+` or `-`,
variable pattern. It misses the contexts shown above and does not provide one
coherent spacing policy for word-based unary operators.

</details>
