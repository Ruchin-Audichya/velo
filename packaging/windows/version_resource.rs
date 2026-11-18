// Shared by the window's and the daemon's build scripts through `include!`.
//
// Writes a Windows resource script into OUT_DIR carrying the application
// icon and a VERSIONINFO block, and compiles it into the executable. The
// script is generated rather than checked in because its numbers come from
// Cargo: the version is the workspace's, so a release cannot ship an `.exe`
// whose Properties tab disagrees with its tag.
//
// What this sets is *metadata*: the Details tab, Task Manager's description,
// the name in the Start menu's search. It is not a signature. Windows shows
// a verified publisher, and Smart App Control lets an app run, only for a
// file signed with a certificate it trusts — see packaging/windows/SIGNING.md.

/// The publisher and product as they appear in Windows. One place, so the
/// product's final name is a one-line change.
const PUBLISHER: &str = "Ruchin Audichya";
const PRODUCT_NAME: &str = "Velo";
const COPYRIGHT: &str = "\u{a9} 2026 Ruchin Audichya";

/// Embed the icon and version block for one executable.
///
/// `icon` is relative to the crate's manifest directory; `description` is
/// what Windows calls the file; `original` is its file name.
fn embed_windows_resources(icon: &str, description: &str, original: &str) {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let manifest = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let icon = manifest.join(icon);
    println!("cargo:rerun-if-changed={}", icon.display());
    println!("cargo:rerun-if-changed=../../packaging/windows/version_resource.rs");

    let version = std::env::var("CARGO_PKG_VERSION").unwrap();
    let mut parts = version
        .split(['.', '-', '+'])
        .map(|p| p.parse::<u16>().unwrap_or(0));
    let (major, minor, patch) = (
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    );
    // Resource strings are UTF-16 in the file; the script itself is read as
    // UTF-8 thanks to the pragma, and quotes are doubled per RC rules.
    let quote = |s: &str| s.replace('"', "\"\"");
    let icon_path = icon.display().to_string().replace('\\', "\\\\");
    let script = format!(
        r#"#pragma code_page(65001)
1 ICON "{icon_path}"

1 VERSIONINFO
 FILEVERSION {major},{minor},{patch},0
 PRODUCTVERSION {major},{minor},{patch},0
 FILEFLAGSMASK 0x3fL
 FILEFLAGS 0x0L
 FILEOS 0x40004L
 FILETYPE 0x1L
 FILESUBTYPE 0x0L
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904B0"
    BEGIN
      VALUE "CompanyName", "{publisher}"
      VALUE "FileDescription", "{description}"
      VALUE "FileVersion", "{version}"
      VALUE "InternalName", "{original}"
      VALUE "LegalCopyright", "{copyright}"
      VALUE "OriginalFilename", "{original}"
      VALUE "ProductName", "{product}"
      VALUE "ProductVersion", "{version}"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
"#,
        publisher = quote(PUBLISHER),
        description = quote(description),
        copyright = quote(COPYRIGHT),
        product = quote(PRODUCT_NAME),
        original = quote(original),
    );
    let rc = out.join("app.rc");
    std::fs::write(&rc, script).expect("writing the resource script");
    embed_resource::compile(&rc, embed_resource::NONE)
        .manifest_optional()
        .expect("embedding the Windows resources");
}
