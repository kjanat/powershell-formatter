$a = 'single'
$b = 'It''s escaped'
$c = "double $a"
$d = "sub $($a + 1) end"
$e = "nested $(("x") + $(1)) done"
$f = "escapes `"quoted`" `$literal `n`t"
$g = "doubled ""quotes"" here"
$h = @'
literal here-string
  keeps   spacing
'@
$i = @"
expandable $a
$($b)
"@
$j = 'unicode héllo 🎉 中文'
$k = "$a.member"
$l = "${a}glued"
$m = 'a)b('
Write-Output "$(""abc"")"
