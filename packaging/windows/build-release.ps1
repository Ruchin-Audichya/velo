# Build Velo's Windows release: the program, then the installer around it.
#
#   pwsh packaging/windows/build-release.ps1
#
# Produces, in dist/:
#   Velo-Setup-<version>-x64.exe   the installer (and uninstaller)
#   Velo-<version>-x64-portable.zip the same program, unzip-and-run
#   SHA256SUMS.txt                 checksums for both
#
# Signing is optional and happens when a certificate is named: by
# -Thumbprint, or by the WINDOWS_SIGNING_* variables sign.ps1 reads. The three
# programs are signed before they are packed (so the installer carries signed
# files and the portable zip holds them too), then the installer itself — and
# with it the uninstaller, which is a copy of it. Without a certificate the
# release builds unsigned, as before. See SIGNING.md.

param(
    [string]$Cargo = 'cargo',
    # Build from what is already downloaded, without touching the network.
    [switch]$Offline,
    # The code-signing certificate's SHA-1 thumbprint, as installed in this
    # machine's certificate store (token plugged in, or the CA's cloud
    # signing client logged in).
    [string]$Thumbprint = $env:WINDOWS_SIGNING_THUMBPRINT
)

$ErrorActionPreference = 'Stop'
$sign = [bool]($Thumbprint -or $env:WINDOWS_SIGNING_PFX_BASE64)
$signer = Join-Path $PSScriptRoot 'sign.ps1'
function Sign-Files([string[]]$Files) {
    if (!$sign) { return }
    & $signer -Files $Files -Thumbprint $Thumbprint
}
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
Sign-Files @(
    (Join-Path $payload 'oxidezap.exe'),
    (Join-Path $payload 'oxidezapd.exe'),
    (Join-Path $payload 'oxidezap-cli.exe')
)

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
Sign-Files @($setup)
$zip = Join-Path $dist "Velo-$version-x64-portable.zip"
Remove-Item $zip -ErrorAction SilentlyContinue
Compress-Archive -Path (Join-Path $payload '*') -DestinationPath $zip

$sums = foreach ($file in $setup, $zip) {
    $hash = (Get-FileHash -Algorithm SHA256 $file).Hash.ToLowerInvariant()
    "$hash  $(Split-Path $file -Leaf)"
}
$sums | Set-Content -Encoding ascii (Join-Path $dist 'SHA256SUMS.txt')
$sums
if (!$sign) { Write-Host 'Built unsigned: no certificate was named. See packaging/windows/SIGNING.md.' }
