[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    $cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
    if (-not (Test-Path -LiteralPath (Join-Path $cargoBin "cargo.exe") -PathType Leaf)) {
        throw "Rust cargo was not found in PATH or $cargoBin. Install the Rust MSVC toolchain before building the Windows release."
    }
    $env:Path = "$cargoBin;$env:Path"
}

$tauri = Join-Path $PSScriptRoot "..\node_modules\.bin\tauri.cmd"
if (-not (Test-Path -LiteralPath $tauri -PathType Leaf)) {
    throw "Tauri CLI is missing. Run npm ci before building the Windows release."
}

& $tauri build --bundles nsis
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}
