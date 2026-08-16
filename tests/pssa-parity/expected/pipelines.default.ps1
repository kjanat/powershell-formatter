Get-Process |
    Where-Object CPU -GT 5 |
    Sort-Object CPU |
    Select-Object -First 3
$Test |
    ForEach-Object {
        Get-Process |
            Select-Object -Last 1
        }
$result = Get-ChildItem |
    Where-Object Length -GT 100 |
    Measure-Object
foo |
    # comment between
    bar
'value' +
'continued'
"a" + `
    "b"
