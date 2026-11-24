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

/// Unpack `entry` into `dir` under a staging name, beside the live file and
/// without touching it, and check it came out at the size it was packed at.
/// Nothing a running Velo uses changes until [`activate`] swaps the whole
/// set in.
pub fn stage(entry: &Entry, dir: &Path, on_bytes: &mut dyn FnMut(u64)) -> io::Result<()> {
    let staging = staged(dir, entry.name);
    let mut decoder = flate2::read::ZlibDecoder::new(entry.packed);
    let mut file = std::fs::File::create(&staging)?;
    let mut buffer = vec![0u8; 256 * 1024];
    let mut total = 0u64;
    loop {
        let read = decoder.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        file.write_all(&buffer[..read])?;
        total += read as u64;
        on_bytes(read as u64);
    }
    file.sync_all()?;
    drop(file);
    if total != entry.size {
        let _ = std::fs::remove_file(&staging);
        return Err(io::Error::other(format!(
            "unpacked {total} bytes where {} were packed",
            entry.size
        )));
    }
    Ok(())
}

/// Swap every staged file in at once, or none of them.
///
/// The window and the daemon speak one protocol version, so a new window
/// beside an old daemon cannot attach: replacing them one at a time, with
/// any failure in between leaving the earlier ones live, could leave Velo
/// unable to start. Each live file is first moved aside as `.old`; if any
/// step fails, every file already swapped is put back and the staged ones
/// removed, so the install is either wholly new or wholly as it was.
pub fn activate(dir: &Path, names: &[&str]) -> io::Result<()> {
    let mut swapped: Vec<&str> = Vec::new();
    let mut result = Ok(());
    for name in names {
        let target = dir.join(name);
        let backup = backup(dir, name);
        let _ = std::fs::remove_file(&backup);
        if target.exists()
            && let Err(error) = rename_retrying(&target, &backup)
        {
            result = Err(error);
            break;
        }
        if let Err(error) = rename_retrying(&staged(dir, name), &target) {
            // Put this file's old copy straight back before unwinding.
            if backup.exists() {
                let _ = rename_retrying(&backup, &target);
            }
            result = Err(error);
            break;
        }
        swapped.push(name);
    }
    if let Err(error) = result {
        for name in swapped.iter().rev() {
            let target = dir.join(name);
            let backup = backup(dir, name);
            let _ = std::fs::remove_file(&target);
            if backup.exists() {
                let _ = rename_retrying(&backup, &target);
            }
        }
        discard(dir, names);
        return Err(error);
    }
    for name in names {
        let _ = std::fs::remove_file(backup(dir, name));
    }
    Ok(())
}

/// Remove any staged files: after a failure, or a run that never got as far
/// as [`activate`].
pub fn discard(dir: &Path, names: &[&str]) {
    for name in names {
        let _ = std::fs::remove_file(staged(dir, name));
    }
}

fn staged(dir: &Path, name: &str) -> std::path::PathBuf {
    dir.join(format!("{name}.new"))
}

fn backup(dir: &Path, name: &str) -> std::path::PathBuf {
    dir.join(format!("{name}.old"))
}

/// A program that has only just exited can hold its file for a moment.
fn rename_retrying(from: &Path, to: &Path) -> io::Result<()> {
    let mut last = None;
    for attempt in 0..20 {
        match std::fs::rename(from, to) {
            Ok(()) => return Ok(()),
            // Nothing to move will stay nothing to move.
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Err(error),
            Err(error) => {
                last = Some(error);
                if attempt < 19 {
                    std::thread::sleep(std::time::Duration::from_millis(250));
                }
            }
        }
    }
    Err(last.unwrap_or_else(|| io::Error::other("could not move the file")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("velo-setup-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_complete_set_replaces_every_file() {
        let d = dir("ok");
        for (name, old) in [("a.exe", "old a"), ("b.exe", "old b")] {
            std::fs::write(d.join(name), old).unwrap();
            std::fs::write(staged(&d, name), format!("new {name}")).unwrap();
        }
        activate(&d, &["a.exe", "b.exe"]).unwrap();
        assert_eq!(
            std::fs::read_to_string(d.join("a.exe")).unwrap(),
            "new a.exe"
        );
        assert_eq!(
            std::fs::read_to_string(d.join("b.exe")).unwrap(),
            "new b.exe"
        );
        assert!(!backup(&d, "a.exe").exists() && !staged(&d, "b.exe").exists());
    }

    /// The second file has nothing staged, so its swap fails: the first,
    /// already swapped, must be put back, and nothing staged left behind.
    #[test]
    fn a_failure_partway_leaves_the_old_set_in_place() {
        let d = dir("fail");
        std::fs::write(d.join("a.exe"), "old a").unwrap();
        std::fs::write(d.join("b.exe"), "old b").unwrap();
        std::fs::write(staged(&d, "a.exe"), "new a").unwrap();
        assert!(activate(&d, &["a.exe", "b.exe"]).is_err());
        assert_eq!(std::fs::read_to_string(d.join("a.exe")).unwrap(), "old a");
        assert_eq!(std::fs::read_to_string(d.join("b.exe")).unwrap(), "old b");
        assert!(!staged(&d, "a.exe").exists());
    }
}
