$true?2:3
$false?
$env:PATH
$env:123
$:abc
$?
$$
$^
${braced name}
${a`}b}
$global:x = $script:y + $private:z
@{}
@()
$a::b
[Array]::Empty
!true
! $x
!5
'Hello' + `
    ' world'
'Hello' |
    ForEach-Object { $_ }
$v = "multi`nline"
%{ $_ }
1?2:3
a?b
7#not-a-comment-arg
$x = 7#comment
$w = @"
no ""escape"" in here-strings
"@
Get-Process |
    Where-Object CPU -gt 1 |
    Select-Object -First 3
$m = $x -band 0xF -bor 2 -shl 1
$s = 'a', 'b' -join ','
$sp = -split 'a b c'
$neg = -not $flag
$j = "interp $env:HOME and $global:thing"
