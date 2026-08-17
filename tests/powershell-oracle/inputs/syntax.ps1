:outer while ($true) {
    break outer
}
foreach ($x in 1..10) {
    if ($x -eq 3) { continue }
    elseif ($x -GT 5) { break }
    else { $x }
}
switch -Regex ($value) {
    'a.*' { 1 }
    default { 2 }
}
try {
    throw [System.Exception]::new('x')
}
catch [System.IO.IOException], [System.ArgumentException] {
    $_.Exception.Message
}
finally {
    'done'
}
function Get-Thing {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string] $Name,
        [int] $Count = 5
    )
    begin { $acc = @() }
    process { $acc += $Name }
    end { $acc }
}
filter Double { $_ * 2 }
class Point {
    [int] $X
    hidden [int] $Y
    static [Point] Origin() { return [Point]::new() }
    Point([int] $x) { $this.X = $x }
}
enum Color {
    Red = 1
    Green
}
$sb = { param($a) $a * 2 }
$h = @{ one = 1; two = 2 }
$arr = @(1, 2, 3)
$sub = $(Get-Date)
$null1 = $x ?? 'default'
$null2 = $obj?.Member
$null3 = $arr?[0]
$tern = $true ? 'yes' : 'no'
$chain = Test-Path C:\ && Write-Output ok || Write-Output no
$x = [System.Collections.Generic.Dictionary[string, int]]::new()
[int[]] $nums = 1, 2, 3
$data = data { 'constant' }
trap { 'trapped'; continue }
using namespace System.Text
$i++
$i--
--$i
++$i
$y = $i++ + 2
