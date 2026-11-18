<#
.SYNOPSIS
  Authenticode-sign Windows release files, then verify every signature.

.DESCRIPTION
  Signs with a certificate that is already available to this machine's
  certificate store — which is how every current public code-signing
  certificate is delivered: on a hardware token, or in a vendor's cloud HSM
  whose key storage provider (KSP) exposes it to the store. The private key
  never touches this script or the repository.

  The certificate is chosen by SHA-1 thumbprint. Optionally, a PFX can be
  imported first (for certificates a CA still issues as files); its password
  is read from an environment variable, never an argument, so it does not land
  in a process list or a CI log.

  After signing, `signtool verify /pa` checks each file against the Windows
  default authentication policy. A file that does not verify fails the run:
  shipping a broken signature is worse than shipping none.

  This script never creates a certificate. A self-signed certificate does not
  make Windows trust a file and is not a substitute; see SIGNING.md.

.EXAMPLE
  ./sign.ps1 -Thumbprint 0123...ABCD -Files target\release\oxidezap.exe,target\release\oxidezapd.exe
#>
param(
    [Parameter(Mandatory = $true)][string[]]$Files,
    [string]$Thumbprint = $env:WINDOWS_SIGNING_THUMBPRINT,
    # Base64 of a .pfx, for CAs that still deliver one. Password from
    # $env:WINDOWS_SIGNING_PFX_PASSWORD.
    [string]$PfxBase64 = $env:WINDOWS_SIGNING_PFX_BASE64,
    [string]$TimestampUrl = $(if ($env:WINDOWS_SIGNING_TIMESTAMP_URL) { $env:WINDOWS_SIGNING_TIMESTAMP_URL } else { 'http://timestamp.digicert.com' }),
    [string]$Description = 'Lightweight Rust-Based WhatsApp Desktop Client',
    [string]$DescriptionUrl = $env:WINDOWS_SIGNING_DESCRIPTION_URL
)

$ErrorActionPreference = 'Stop'

function Find-SignTool {
    $onPath = Get-Command signtool.exe -ErrorAction SilentlyContinue
    if ($onPath) { return $onPath.Source }
    $kits = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\bin'
    $found = Get-ChildItem -Path $kits -Recurse -Filter signtool.exe -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -match '\\x64\\' } |
        Sort-Object FullName -Descending | Select-Object -First 1
    if (!$found) { throw 'signtool.exe not found. Install the Windows SDK (Signing Tools for Desktop Apps).' }
    return $found.FullName
}

$signtool = Find-SignTool

if ($PfxBase64) {
    if (!$env:WINDOWS_SIGNING_PFX_PASSWORD) { throw 'WINDOWS_SIGNING_PFX_BASE64 is set but WINDOWS_SIGNING_PFX_PASSWORD is not.' }
    $pfx = Join-Path ([IO.Path]::GetTempPath()) ("sign-" + [guid]::NewGuid().ToString('N') + '.pfx')
    try {
        [IO.File]::WriteAllBytes($pfx, [Convert]::FromBase64String($PfxBase64))
        $secure = ConvertTo-SecureString $env:WINDOWS_SIGNING_PFX_PASSWORD -AsPlainText -Force
        $imported = Import-PfxCertificate -FilePath $pfx -CertStoreLocation Cert:\CurrentUser\My -Password $secure
        if (!$Thumbprint) { $Thumbprint = $imported.Thumbprint }
    } finally {
        Remove-Item -LiteralPath $pfx -Force -ErrorAction SilentlyContinue
    }
}

if (!$Thumbprint) { throw 'No signing certificate: set WINDOWS_SIGNING_THUMBPRINT (or provide a PFX).' }
$Thumbprint = ($Thumbprint -replace '\s', '').ToUpperInvariant()

$cert = Get-ChildItem Cert:\CurrentUser\My, Cert:\LocalMachine\My -ErrorAction SilentlyContinue |
    Where-Object Thumbprint -eq $Thumbprint | Select-Object -First 1
if (!$cert) { throw "Certificate $Thumbprint is not in the CurrentUser or LocalMachine personal store." }
if (!$cert.HasPrivateKey) { throw "Certificate $Thumbprint has no usable private key on this machine (is the token or cloud KSP connected?)." }
if ($cert.EnhancedKeyUsageList.ObjectId -notcontains '1.3.6.1.5.5.7.3.3') { throw "Certificate $Thumbprint is not a code-signing certificate." }
if ($cert.Subject -eq $cert.Issuer) { throw 'Refusing a self-signed certificate: Windows will not treat it as a trusted publisher.' }
"Signing as: $($cert.Subject)"
"Issued by:  $($cert.Issuer)"
"Valid to:   $($cert.NotAfter.ToString('yyyy-MM-dd'))"

foreach ($file in $Files) {
    if (!(Test-Path -LiteralPath $file)) { throw "No such file: $file" }
    $arguments = @('sign', '/sha1', $Thumbprint, '/fd', 'SHA256', '/tr', $TimestampUrl, '/td', 'SHA256', '/d', $Description)
    if ($DescriptionUrl) { $arguments += @('/du', $DescriptionUrl) }
    $arguments += $file
    & $signtool @arguments
    if ($LASTEXITCODE -ne 0) { throw "signtool sign failed for $file" }
}

$failed = @()
foreach ($file in $Files) {
    & $signtool verify /pa /v $file | Out-Null
    if ($LASTEXITCODE -ne 0) { $failed += $file; continue }
    $signature = Get-AuthenticodeSignature -LiteralPath $file
    "{0}: {1} — {2}" -f (Split-Path -Leaf $file), $signature.Status, $signature.SignerCertificate.Subject
}
if ($failed) { throw "Signature verification failed: $($failed -join ', ')" }
'All files signed and verified.'
