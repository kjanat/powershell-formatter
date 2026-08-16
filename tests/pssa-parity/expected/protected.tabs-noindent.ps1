$here = @'
  content belongs exactly here
    IF($x){'no formatting'}
'@
$expand = @"
  value: $x
  sub: $( 1+2 )
"@
$str = 'single  with  spaces'
$dq = "double $var interp"
$uni = 'héllo 🎉 中文'
Write-Output "kept $( $a  +  $b ) verbatim"
cmd --% raw %VAR% "quoted" stuff
$multi = 'line one
line two'
