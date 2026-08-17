# Regenerates all oracle fixtures from inputs/ using the local pwsh.
# Run: pwsh -NoProfile -File generate.ps1
Set-StrictMode -Version 3
$ErrorActionPreference = 'Stop'
$root = $PSScriptRoot
$inputs = Join-Path $root 'inputs'
$fixtures = Join-Path $root 'fixtures'
New-Item -ItemType Directory -Force -Path $fixtures | Out-Null

# The tokenizer is whatever this pwsh ships, so the fixtures are only
# meaningful alongside the version that produced them. A different build is
# not fatal — the differential tests re-check behaviour against it — but it
# must be visible, since it changes what the committed fixtures mean.
$versionFile = Join-Path $fixtures 'VERSION'
$current = $PSVersionTable.PSVersion.ToString()
if (Test-Path $versionFile) {
	$recorded = (Get-Content $versionFile -Raw).Trim()
	if ($recorded -and $recorded -ne $current) {
		Write-Warning "fixtures were generated with PowerShell $recorded; this is $current"
	}
}

Get-ChildItem $inputs -Filter *.ps1 | ForEach-Object {
	$out = Join-Path $fixtures ($_.BaseName + '.json')
	& (Join-Path $root 'dump-tokens.ps1') -Path $_.FullName -OutPath $out
	Write-Host "wrote $out"
}
Set-Content -Path $versionFile -Value $current
