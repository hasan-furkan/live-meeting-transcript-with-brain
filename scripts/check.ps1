# Quality gate: every step must pass. Output goes to the console and check.log.
$ErrorActionPreference = 'Continue'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$log = Join-Path $root 'check.log'
Remove-Item $log -ErrorAction SilentlyContinue

# whisper-rs needs cmake; on Windows it may only exist inside Visual Studio.
if (-not (Get-Command cmake -ErrorAction SilentlyContinue)) {
    $vsCmake = Get-ChildItem 'C:\Program Files\Microsoft Visual Studio\*\*\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin' -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($vsCmake) { $env:PATH = "$($vsCmake.FullName);$env:PATH" }
}

$clippy = @('clippy', '--workspace', '--all-targets', '--', '-D', 'warnings', '-D', 'clippy::unwrap_used', '-D', 'clippy::expect_used', '-D', 'clippy::panic', '-D', 'clippy::todo', '-D', 'clippy::dbg_macro')
$steps = @(
    @{ name = 'cargo fmt'; exe = 'cargo'; args = @('fmt', '--all', '--check') },
    @{ name = 'cargo clippy'; exe = 'cargo'; args = $clippy },
    @{ name = 'cargo test'; exe = 'cargo'; args = @('test', '--workspace') },
    @{ name = 'cargo audit'; exe = 'cargo'; args = @('audit') },
    @{ name = 'cargo deny'; exe = 'cargo'; args = @('deny', 'check') }
)
if (Test-Path (Join-Path $root 'ui/package.json')) {
    $steps += @(
        @{ name = 'ui typecheck'; exe = 'npm'; args = @('--prefix', 'ui', 'run', 'typecheck') },
        @{ name = 'ui lint'; exe = 'npm'; args = @('--prefix', 'ui', 'run', 'lint') },
        @{ name = 'ui test'; exe = 'npm'; args = @('--prefix', 'ui', 'run', 'test', '--', '--run') },
        @{ name = 'ui build'; exe = 'npm'; args = @('--prefix', 'ui', 'run', 'build') }
    )
}

foreach ($s in $steps) {
    "=== $($s.name)" | Tee-Object -FilePath $log -Append
    & $s.exe @($s.args) 2>&1 | ForEach-Object { "$_" } | Tee-Object -FilePath $log -Append
    if ($LASTEXITCODE -ne 0) {
        "CHECK FAILED: $($s.name)" | Tee-Object -FilePath $log -Append
        exit 1
    }
}
'CHECK PASSED' | Tee-Object -FilePath $log -Append
exit 0
