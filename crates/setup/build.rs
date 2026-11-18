//! Two jobs: give the installer its icon, version and manifest, and pack
//! the program it installs into it.
//!
//! The manifest matters more than it looks. Windows guesses that any
//! executable with "setup" in its name is an installer that needs an
//! administrator, and raises a UAC prompt for it — unless the manifest says
//! `asInvoker`. gpui's own manifest, linked into every app it builds, says
//! exactly that; Velo installs per user and asks for nothing more.
//!
//! The payload is read from `VELO_PAYLOAD_DIR` when it is set (the release
//! task sets it after building the program) and compressed into
//! `OUT_DIR/payload.bin`. Unset, the payload is empty and the installer says
//! so instead of installing nothing.

use std::io::Write as _;
use std::path::PathBuf;

include!("../../packaging/windows/version_resource.rs");

/// What the installer carries, in the order it writes them.
const FILES: &[&str] = &[
    "oxidezap.exe",
    "oxidezapd.exe",
    "oxidezap-cli.exe",
    "LICENSE.txt",
    "THIRD-PARTY-NOTICES.txt",
];

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=VELO_PAYLOAD_DIR");
    embed_installer_resources();
    pack_payload();
}

fn embed_installer_resources() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    embed_windows_resources(
        "../../packaging/windows/velo.ico",
        "Velo Setup",
        "velo-setup.exe",
    );
    // No manifest of ours: gpui links one into every app it builds, and it
    // already says `asInvoker` (so the "setup" in this file's name does not
    // trigger Windows' installer detection and a UAC prompt) and per-monitor
    // DPI awareness. A second one fails the link with "duplicate resource".
}

/// `VELO1`, a count, then for each file: its name, its size, its
/// compressed size and the zlib stream.
fn pack_payload() {
    let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("payload.bin");
    let mut payload = Vec::new();
    payload.extend_from_slice(b"VELO1");
    let Some(dir) = std::env::var_os("VELO_PAYLOAD_DIR").map(PathBuf::from) else {
        payload.extend_from_slice(&0u32.to_le_bytes());
        std::fs::write(&out, payload).unwrap();
        return;
    };
    payload.extend_from_slice(&(FILES.len() as u32).to_le_bytes());
    for name in FILES {
        let path = dir.join(name);
        println!("cargo:rerun-if-changed={}", path.display());
        let bytes = std::fs::read(&path)
            .unwrap_or_else(|e| panic!("the payload needs {}: {e}", path.display()));
        let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
        encoder.write_all(&bytes).unwrap();
        let packed = encoder.finish().unwrap();
        payload.extend_from_slice(&(name.len() as u16).to_le_bytes());
        payload.extend_from_slice(name.as_bytes());
        payload.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        payload.extend_from_slice(&(packed.len() as u64).to_le_bytes());
        payload.extend_from_slice(&packed);
    }
    std::fs::write(&out, payload).unwrap();
}
