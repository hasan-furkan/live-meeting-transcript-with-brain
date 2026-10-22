# Release build with the right environment. Usage: powershell -File scripts/build.ps1 [dev]
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
. (Join-Path $PSScriptRoot "env.ps1")
$feature = if ($env:OS -eq 'Windows_NT') { 'cuda' } else { 'metal' }
$command = if ($args -contains 'dev') { 'dev' } else { 'build' }
cargo tauri $command --features $feature
exit $LASTEXITCODE
