if ($x -eq 1) {
    'yes'
}
else {
    'no'
}
if ($a) {
    'one' 
}
elseif ($b) {
    'two' 
}
else {
    'three' 
}
function bar {
    'body'
}
$sb = { param($p) $p * 2 }
switch ($v) {
    1 {
        'one'
    } default {
        'other'
    } 
}
Get-Process | Where-Object { $_.CPU -gt 1 } | Select-Object Name
foreach ($i in 1..3) {
    $i
}
