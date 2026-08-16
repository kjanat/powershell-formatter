foreach ($x in $y)
{
    if ($x -eq 1) { break }
    elseif ($x -ne 2) { continue }
}
try { Get-ChildItem -Path C:\ -Recurse } catch { }
function Test-Casing
{
    param([string] $Value)
    return $Value
}
$r = 'a' -replace 'b' -split 'c' -join 'd'
$b = $x -band $y -bor $z
while ($FALSE) { }
do { } while ($FALSE)
Write-Output 'lower cmdlet'
Get-Process | Where-Object { $_ }
