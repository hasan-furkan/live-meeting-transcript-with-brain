# Quality gate: every step must pass. Output goes to the console and check.log.
$ErrorActionPreference = 'Continue'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$log = Join-Path $root 'check.log'
Remove-Item $log -ErrorAction SilentlyContinue
function Out-Log { process { $line = "$_"; Write-Output $line; Add-Content -Path $log -Value $line -Encoding UTF8 } }

# whisper-rs needs cmake; on Windows it may only exist inside Visual Studio.
if (-not (Get-Command cmake -ErrorAction SilentlyContinue)) {
    $vsCmake = Get-ChildItem 'C:\Program Files\Microsoft Visual Studio\*\*\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin' -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($vsCmake) { $env:PATH = "$($vsCmake.FullName);$env:PATH" }
}
# whisper-rs-sys runs bindgen, which needs libclang (e.g. `pip install --user libclang`).
if ($IsWindows -or $env:OS -eq 'Windows_NT') {
    if (-not $env:LIBCLANG_PATH) {
        $clang = Get-ChildItem "$env:APPDATA\Python\Python3*\site-packages\clang\native\libclang.dll", 'C:\Program Files\LLVM\bin\libclang.dll' -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($clang) { $env:LIBCLANG_PATH = $clang.DirectoryName }
    }
    if (-not $env:CMAKE_GENERATOR -and -not (Get-Command ninja -ErrorAction SilentlyContinue)) { $env:CMAKE_GENERATOR = 'Visual Studio 17 2022' }
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
    "=== $($s.name)" | Out-Log
    & $s.exe @($s.args) 2>&1 | ForEach-Object { "$_" } | Out-Log
    if ($LASTEXITCODE -ne 0) {
        "CHECK FAILED: $($s.name)" | Out-Log
        exit 1
    }
}
'CHECK PASSED' | Out-Log
exit 0
