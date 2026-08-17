# Dumps canonical command/parameter casing for the commands the parity
# fixtures use, as a JSON catalog consumable by powershell_formatter's
# JsonCatalog. Run: pwsh -NoProfile -File generate-catalog.ps1
Set-StrictMode -Version 3
$ErrorActionPreference = 'Stop'

$names = @(
    'Get-Process', 'Where-Object', 'Select-Object', 'Sort-Object',
    'ForEach-Object', 'Get-ChildItem', 'Measure-Object', 'Write-Output',
    'Get-Date', 'Get-Random', 'Get-Item', 'Copy-Item', 'Test-Path',
    'Out-Null', 'Get-Content', 'New-Item', 'Set-Content', 'Invoke-Command'
)

$commands = [ordered]@{}
foreach ($n in $names) {
    $ci = Get-Command -Name $n -ErrorAction SilentlyContinue
    if ($null -eq $ci) { continue }
    $params = @()
    if ($null -ne $ci.Parameters) {
        $params = @($ci.Parameters.Keys)
    }
    $commands[$ci.Name] = $params
}

$json = [pscustomobject]@{ commands = $commands } | ConvertTo-Json -Depth 4
[System.IO.File]::WriteAllText((Join-Path $PSScriptRoot 'catalog.json'), $json)
Write-Host "wrote catalog.json with $($commands.Count) commands"
