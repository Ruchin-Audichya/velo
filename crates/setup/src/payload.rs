//! The files the installer carries, packed by `build.rs`.

use std::io::{self, Read as _, Write as _};
use std::path::Path;

static PAYLOAD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/payload.bin"));

/// One packed file.
pub struct Entry {
    pub name: &'static str,
    /// Its size once written.
    pub size: u64,
    packed: &'static [u8],
}

/// Everything in the payload, in the order it should be written.
pub fn entries() -> Result<Vec<Entry>, String> {
    let bad = || "the installer's payload is damaged".to_string();
    let rest = PAYLOAD.strip_prefix(b"VELO1").ok_or_else(bad)?;
    let (count, mut rest) = take::<4>(rest).ok_or_else(bad)?;
    let mut entries = Vec::new();
    for _ in 0..u32::from_le_bytes(count) {
        let (len, after) = take::<2>(rest).ok_or_else(bad)?;
        let len = usize::from(u16::from_le_bytes(len));
        let name = after.get(..len).ok_or_else(bad)?;
        let name = std::str::from_utf8(name).map_err(|_| bad())?;
        let (size, after) = take::<8>(&after[len..]).ok_or_else(bad)?;
        let (packed_len, after) = take::<8>(after).ok_or_else(bad)?;
        let packed_len = usize::try_from(u64::from_le_bytes(packed_len)).map_err(|_| bad())?;
        let packed = after.get(..packed_len).ok_or_else(bad)?;
        entries.push(Entry {
            name,
            size: u64::from_le_bytes(size),
            packed,
        });
        rest = &after[packed_len..];
    }
    Ok(entries)
}

fn take<const N: usize>(bytes: &'static [u8]) -> Option<([u8; N], &'static [u8])> {
    let head = bytes.get(..N)?;
    Some((head.try_into().ok()?, &bytes[N..]))
}

/// Write `entry` to `dir`, beside any older copy and then over it, so a
/// failure halfway leaves the old file rather than half of a new one.
pub fn extract(entry: &Entry, dir: &Path, on_bytes: &mut dyn FnMut(u64)) -> io::Result<()> {
    let target = dir.join(entry.name);
    let staging = dir.join(format!("{}.new", entry.name));
    {
        let mut decoder = flate2::read::ZlibDecoder::new(entry.packed);
        let mut file = std::fs::File::create(&staging)?;
        let mut buffer = vec![0u8; 256 * 1024];
        loop {
            let read = decoder.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            file.write_all(&buffer[..read])?;
            on_bytes(read as u64);
        }
        file.sync_all()?;
    }
    // A program that has only just exited can hold its file for a moment.
    let mut last = None;
    for _ in 0..20 {
        match std::fs::rename(&staging, &target) {
            Ok(()) => return Ok(()),
            Err(error) => {
                last = Some(error);
                std::thread::sleep(std::time::Duration::from_millis(250));
            }
        }
    }
    let _ = std::fs::remove_file(&staging);
    Err(last.unwrap_or_else(|| io::Error::other("could not replace the file")))
}
