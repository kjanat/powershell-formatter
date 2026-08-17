Get-Process |
Where-Object CPU -gt 5 |
Sort-Object CPU |
Select-Object -First 3
$Test |
    ForEach-Object {
        Get-Process |
            Select-Object -Last 1
    }
$result = Get-ChildItem |
Where-Object Length -gt 100 |
Measure-Object
foo |
# comment between
bar
'value' +
    'continued'
"a" + `
"b"
