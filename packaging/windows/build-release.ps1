# Build Velo's Windows release: the program, then the installer around it.
#
#   pwsh packaging/windows/build-release.ps1
#
# Produces, in dist/:
#   Velo-Setup-<version>-x64.exe   the installer (and uninstaller)
#   Velo-<version>-x64-portable.zip the same program, unzip-and-run
#   SHA256SUMS.txt                 checksums for both
#
# Nothing here signs anything. A signed release runs sign.ps1 over both
# executables before the installer is built, and over the installer after;
# see SIGNING.md.

param(
    [string]$Cargo = 'cargo',
    # Build from what is already downloaded, without touching the network.
    [switch]$Offline
)

$ErrorActionPreference = 'Stop'
$extra = @('--locked')
if ($Offline) { $extra += '--offline' }
$root = Resolve-Path (Join-Path $PSScriptRoot '..\..')
Set-Location $root

$version = (Select-String -Path Cargo.toml -Pattern '^version = "(.+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
Write-Host "Velo $version"

& $Cargo build --release @extra -p oxidezap-gui -p oxidezap-daemon -p oxidezap-cli
if ($LASTEXITCODE -ne 0) { throw 'building Velo failed' }

$payload = Join-Path $root 'target\velo-payload'
Remove-Item -Recurse -Force $payload -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $payload | Out-Null
foreach ($exe in 'oxidezap.exe', 'oxidezapd.exe', 'oxidezap-cli.exe') {
    Copy-Item (Join-Path $root "target\release\$exe") $payload
}
Copy-Item (Join-Path $root 'packaging\release\LICENSE.txt') $payload
Copy-Item (Join-Path $root 'packaging\release\THIRD-PARTY-NOTICES.txt') $payload

$env:VELO_PAYLOAD_DIR = $payload
try {
    & $Cargo build --release @extra -p velo-setup
    if ($LASTEXITCODE -ne 0) { throw 'building the installer failed' }
} finally {
    Remove-Item Env:\VELO_PAYLOAD_DIR
}

$dist = Join-Path $root 'dist'
New-Item -ItemType Directory -Force $dist | Out-Null
$setup = Join-Path $dist "Velo-Setup-$version-x64.exe"
Copy-Item (Join-Path $root 'target\release\velo-setup.exe') $setup -Force
$zip = Join-Path $dist "Velo-$version-x64-portable.zip"
Remove-Item $zip -ErrorAction SilentlyContinue
Compress-Archive -Path (Join-Path $payload '*') -DestinationPath $zip

$sums = foreach ($file in $setup, $zip) {
    $hash = (Get-FileHash -Algorithm SHA256 $file).Hash.ToLowerInvariant()
    "$hash  $(Split-Path $file -Leaf)"
}
$sums | Set-Content -Encoding ascii (Join-Path $dist 'SHA256SUMS.txt')
$sums
