Get-ChildItem -Path C:\Windows -Filter *.dll -Recurse
dir -Path:*
Copy-Item .\a.txt ..\b.txt
& "C:\Program Files\tool.exe" /switch -x
. .\dot-source.ps1
.\relative.ps1 arg1 arg2
cmd --% raw %VAR% "a|b" | more
Write-Output a#b
Write-Output http://example.com/path?q=1
Get-Item [a-z]*.txt
foo -foo-bar -baz:$true
ls | ? { $_.Length -gt 1kb } | % Name
Invoke-Thing @args @splat
cmd 2>&1 3> warn.txt *> all.log
cmd > out.txt 2> err.txt
echo user@host.com
echo -3
echo +5
crazy`,name still-one-arg
