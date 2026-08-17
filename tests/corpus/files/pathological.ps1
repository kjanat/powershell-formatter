# Hand-written pathological syntax exercising lexer/formatter edge cases.
${a`}b} = 1
$true?2:3
$w = $true ? 2 : 3
$x = $null ?? 9?10:11
${y} = @'
here-string with '@-lookalikes
 '@ not a terminator
'@
$z = @"
interpolated $( "nested $( 'deep' ) string" ) here
"@
Get-Item [a-z]*.txt -ErrorAction:SilentlyContinue
& "C:\Program Files\App\tool.exe" --% /flag "quo|ted"
crazy`,name still-one-arg 77z.exe 2+2 1..2
$m = $a -band 0xFF -shl 2 -bxor 0b1010
$s = -join ('a', 'b' | Sort-Object)
filter Double { $_ * 2 }
workflow Flow { parallel { InlineScript { 1 } } }
class Generic {
    [System.Collections.Generic.Dictionary[string, [int]]] $Map
    hidden static [int] Compute([int] $x) { return $x * 2 }
}
switch -Regex ($v) {
    '^a.*z$' { break }
    default { continue }
}
:outer foreach ($i in 1..3) {
    :inner while ($true) { break outer }
}
$sb = { param([Parameter(Mandatory)] [int] $n) $n }
$h = @{ 'key=with=equals' = 1; ($key) = 2 }
$obj.
    Chained.
    Members | Out-Null
$neg = - 5 + + 3 - -2
Write-Output "tick `u{1F389} unicode 中文 é"
1kb..2mb | Out-Null
$i = 0; $i++; --$i; $i--
$r = 1 `
    + 2 `
    + 3
data Sample { 'constant' }
trap [System.Exception] { continue }
$flag = $PSBoundParameters?.Count ?? 0
