[CmdletBinding()]
param(
    [string]$ArtifactDirectory,
    [string]$OutputDirectory
)

$ErrorActionPreference = "Stop"

if ([string]::IsNullOrWhiteSpace($ArtifactDirectory)) {
    $cargoTargetDirectory = if ([string]::IsNullOrWhiteSpace($env:CARGO_TARGET_DIR)) {
        Join-Path $PSScriptRoot "..\src-tauri\target"
    } else {
        $env:CARGO_TARGET_DIR
    }
    $ArtifactDirectory = Join-Path $cargoTargetDirectory "release\bundle\nsis"
}
if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
    $OutputDirectory = Join-Path $PSScriptRoot "..\release"
}

if (-not (Test-Path -LiteralPath $ArtifactDirectory -PathType Container)) {
    throw "NSIS artifact directory was not found: $ArtifactDirectory. Run npm run release:windows first."
}

$installers = @(Get-ChildItem -LiteralPath $ArtifactDirectory -Filter "*.exe" -File)
if ($installers.Count -ne 1) {
    throw "Expected exactly one NSIS installer in $ArtifactDirectory; found $($installers.Count)."
}

$installer = $installers[0]
if ($installer.Length -eq 0) {
    throw "NSIS installer is empty: $($installer.FullName)"
}

New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
$releaseInstaller = Join-Path $OutputDirectory $installer.Name
Copy-Item -LiteralPath $installer.FullName -Destination $releaseInstaller -Force

$sha256Algorithm = [System.Security.Cryptography.SHA256]::Create()
$releaseStream = [System.IO.File]::OpenRead($releaseInstaller)
try {
    $sha256 = [System.BitConverter]::ToString($sha256Algorithm.ComputeHash($releaseStream)).Replace("-", "").ToLowerInvariant()
} finally {
    $releaseStream.Dispose()
    $sha256Algorithm.Dispose()
}
$checksumPath = "$releaseInstaller.sha256"
Set-Content -LiteralPath $checksumPath -Value "$sha256 *$($installer.Name)" -Encoding ascii -NoNewline

Write-Host "Installer: $releaseInstaller"
Write-Host "SHA-256:  $checksumPath"
