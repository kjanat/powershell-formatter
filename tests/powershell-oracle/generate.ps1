# Regenerates all oracle fixtures from inputs/ using the local pwsh.
# Run: pwsh -NoProfile -File generate.ps1
Set-StrictMode -Version 3
$ErrorActionPreference = 'Stop'
$root = $PSScriptRoot
$inputs = Join-Path $root 'inputs'
$fixtures = Join-Path $root 'fixtures'
New-Item -ItemType Directory -Force -Path $fixtures | Out-Null

Get-ChildItem $inputs -Filter *.ps1 | ForEach-Object {
    $out = Join-Path $fixtures ($_.BaseName + '.json')
    & (Join-Path $root 'dump-tokens.ps1') -Path $_.FullName -OutPath $out
    Write-Host "wrote $out"
}
Set-Content -Path (Join-Path $fixtures 'VERSION') -Value $PSVersionTable.PSVersion.ToString()
