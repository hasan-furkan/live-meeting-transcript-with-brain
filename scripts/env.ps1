# Build environment for whisper-rs (dot-source it: `. scripts/env.ps1`).
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
        else { Write-Warning 'libclang not found: run `py -m pip install --user libclang` (or install LLVM), or set LIBCLANG_PATH.' }
    }
    if (-not $env:CMAKE_GENERATOR -and -not (Get-Command ninja -ErrorAction SilentlyContinue)) { $env:CMAKE_GENERATOR = 'Visual Studio 17 2022' }
}
