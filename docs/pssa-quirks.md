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
closing `}` lands at the block-content level. (Current PSSA `master` has a
different algorithm; the shipped 1.25.0 behaves as above.) **We reproduce
this for parity.**

## 3. Trailing whitespace is left behind when blocks expand

`if ($x) { BREAK }` (with `IgnoreOneLineBlock` off) becomes:

```text
if ($x) {\n        break \n    }
                        ^ trailing space survives
```

Corrections replace only the brace tokens, so the spaces that used to
separate content from braces stay behind as trailing whitespace
(`PSAvoidTrailingWhitespace` is not part of the formatting presets).
**We reproduce this for parity.**

## 4. Asymmetric inner-brace spacing for one-line hashtables

`@{a=1}` → `@{a = 1 }` — a space is enforced *before* `}` but not after
`@{`, because `CheckInnerBrace`'s open-side check only looks at `LCurly`
tokens and `@{` is `AtCurly`. **We reproduce this.**

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

`Invoke-Formatter -ScriptDefinition "if(` $a){'x'}`nif(`$b){'y'}" -Range @(2,1,2,12)`returns`if ($b) { 'y'}`— the space before`}` is never added because
earlier fixes on the line grew it past the (fixed) range end, and the
filter re-runs each iteration against shifted coordinates. **We diverge**
(apply all corrections whose original extent is inside the range).

## 7. Mixed newlines are a hard error

`EditableText` requires perfectly uniform line endings and throws otherwise.
**We diverge** (normalize the newlines the formatter owns to the dominant
inter-token style).

## 8. The documented `TokenFlags` operator mask does not match behavior

Reading current `UseConsistentWhitespace.cs` suggests `CheckOperator`
covers only assignment/add/multiply-precedence operators via a bit-subset
test — but the shipped 1.25.0 normalizes *every* binary dash-word operator
(`-eq`, `-and`, `-band`, `-shl`, ...). Trust the binary, not the source you
happen to be reading: our operator set was fixed empirically.
