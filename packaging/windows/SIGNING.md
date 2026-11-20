# Signing the Windows release

Windows decides whether to trust an `.exe` by its **Authenticode signature**,
not by the publisher name written into the file. This page explains what that
means for this project and what is needed to ship a build that Windows shows
as coming from **Ruchin Audichya** instead of "Unknown publisher".

## What is already done

- **Version metadata.** Every build embeds a version block
  (`packaging/windows/version_resource.rs`): Company *Ruchin Audichya*,
  description *Velo* (the service: *Velo background service*), copyright
  *© 2026 Ruchin Audichya*, and the version from `Cargo.toml`. It shows in the
  file's Properties → Details and in Task Manager. **It is not a signature**:
  any file can claim any company name, so Windows never uses this field to
  decide trust, and the UAC/SmartScreen prompts keep saying "Unknown
  publisher" until the file is signed.
- **A signing script**, `packaging/windows/sign.ps1`. It signs with SHA-256,
  adds an RFC 3161 timestamp (so signatures stay valid after the certificate
  expires), and runs `signtool verify /pa` on every file, failing if any
  does not verify. It refuses self-signed certificates.
- **CI integration.** `.github/workflows/build.yml` signs the three Windows
  binaries before packaging when the signing secrets exist, and is unchanged
  (unsigned) when they do not.

## Why Smart App Control blocks the app

Smart App Control (Windows 11) allows an app to run only if Microsoft's cloud
service already recognises it as safe **or** it is signed with a certificate
from a certificate authority in Microsoft's trusted root program. A new,
unsigned app has neither, so it is blocked — every time, for every user —
and there is no setting inside the app that changes this. On the development
machine the same rule blocks the build's own helper programs (Rust build
scripts and proc-macro libraries), which is why local builds fail with
`An Application Control policy has blocked this file (os error 4551)` while
Smart App Control is on.

A valid signature from a trusted CA is the supported way to be allowed to run.
It does **not** by itself give SmartScreen reputation: a new certificate still
shows a SmartScreen warning for a while after the first downloads, and that
reputation builds with clean installs over time. Since 2024 that is true of
EV certificates too.

## What you need to provide

A **code-signing certificate issued to you** (individual validation, in the
name *Ruchin Audichya*) by a public certificate authority. Options that exist
today — check current eligibility and prices with each provider:

| Route | Publisher shown | Notes |
| --- | --- | --- |
| **Individual OV code-signing certificate** from a public CA (e.g. Sectigo, DigiCert, SSL.com, Certum) | Ruchin Audichya | Identity check with government ID. Since June 2023 the key must live on a hardware token or the CA's cloud HSM, so you usually receive a USB token or a cloud-signing account rather than a `.pfx` file. Certum offers a lower-cost certificate aimed at open-source developers. |
| **Microsoft Artifact Signing** (formerly Trusted Signing) | Your validated identity | Cheapest and integrates with GitHub Actions, but **public-trust certificates for individuals are only available to developers located in the US or Canada** (organizations in more countries). Not open to an individual in India at the time of writing. |
| **SignPath Foundation** (free for OSS) | SignPath Foundation | Free signing for qualifying open-source projects, but the publisher shown is the foundation, not you, and the source must be public and built on CI. |
| **Microsoft Store (MSIX)** | Microsoft re-signs | Free for individuals, but Store policy 10.1.6 requires permission under the connected service's terms, and WhatsApp's terms do not allow third-party clients. Not a realistic route for Velo. |

**Recommended for an individual in India:** a standard individual code-signing
certificate from Certum, using its SimplySign cloud signing so no hardware
token has to be shipped. Prices and eligibility change; check on the CA's site.

A self-signed certificate is not an option: Windows does not trust it, Smart
App Control still blocks the file, and presenting it as trusted would be
misleading. `sign.ps1` refuses one.

## Signing a release

`build-release.ps1` signs when it is given a certificate. With SimplySign
logged in (or the token plugged in):

```powershell
pwsh packaging/windows/build-release.ps1 -Thumbprint <THUMBPRINT>
```

It signs the three programs before they are packed into the installer and the
portable zip, then signs the installer (the uninstaller is a copy of it), and
verifies every signature with `signtool verify /pa`; a failure stops the
release. The checksums are computed after signing. Without `-Thumbprint` (or
the `WINDOWS_SIGNING_*` variables) the release builds unsigned and says so.

After a signed build, upload the new files to the GitHub release and update
the winget manifest's `InstallerSha256` — the hash changes when a file is
signed.

## Signing individual files

With the certificate available in your Windows certificate store (token
plugged in, or the CA's signing client logged in), get its thumbprint:

```powershell
Get-ChildItem Cert:\CurrentUser\My -CodeSigningCert | Format-Table Subject, Thumbprint, NotAfter
```

Then sign and verify the release:

```powershell
./packaging/windows/sign.ps1 -Thumbprint <THUMBPRINT> -Files `
  target\release\velo.exe, target\release\velod.exe, target\release\velo-cli.exe
```

Check the result yourself:

```powershell
& "C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64\signtool.exe" verify /pa /v target\release\velo.exe
Get-AuthenticodeSignature target\release\velo.exe | Format-List Status, SignerCertificate
```

## Signing in GitHub Actions

Add these as **repository secrets** (Settings → Secrets and variables →
Actions). Never commit a certificate, key, or password.

| Secret | When |
| --- | --- |
| `WINDOWS_SIGNING_PFX_BASE64` | Your certificate as a base64-encoded `.pfx`, if your CA issues one. `[Convert]::ToBase64String([IO.File]::ReadAllBytes('cert.pfx'))` |
| `WINDOWS_SIGNING_PFX_PASSWORD` | The `.pfx` password. |
| `WINDOWS_SIGNING_THUMBPRINT` | Optional with a PFX; required when the certificate is installed on a self-hosted runner. |

A certificate held on a USB token cannot be used by GitHub's hosted runners.
Either sign locally with `sign.ps1` after downloading the CI build, use a
self-hosted Windows runner with the token attached, or use your CA's
cloud-signing service, which provides its own GitHub Action or CLI — add that
step in place of `sign.ps1` and keep the `signtool verify /pa` check.

Optionally set `WINDOWS_SIGNING_TIMESTAMP_URL` to your CA's timestamp server
(DigiCert's is the default) and `WINDOWS_SIGNING_DESCRIPTION_URL` to the
project's homepage.

## After signing

1. `signtool verify /pa` passes on all three binaries (the script checks).
2. Properties → Digital Signatures shows *Ruchin Audichya*.
3. On a machine with Smart App Control on, the signed build starts.
4. SmartScreen may still warn on first downloads until reputation builds;
   that is expected and not something a build setting can change.
