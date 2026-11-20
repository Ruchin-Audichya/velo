//! The one-time move from the names Velo shipped under before 1.1.
//!
//! Velo began as a fork of a client called *oxidezap*, and 1.0 still kept that
//! name everywhere a user could not see it: the data folder, the downloads
//! folder, the registry, the sign-in entry and the notification identity.
//! 1.1 uses Velo's own names. This module is the bridge, and the one place in
//! the tree that still has to spell the old ones.
//!
//! Every Velo program calls [`migrate`] first thing, before it opens a file,
//! so whichever starts first does the move and the others find it done. Each
//! step only renames into a name that does not exist yet — nothing is ever
//! overwritten or merged — and is idempotent, so a second call, a second
//! program, or a crash halfway through all leave a consistent state. A folder
//! that cannot be moved (another program holding a file in it) is left where
//! it is and logged; the installer stops every Velo process before it runs
//! the new version, so in practice nothing holds them.

/// Move anything still under the pre-1.1 names to Velo's. Cheap when there is
/// nothing to do: a directory listing per base folder, a registry probe.
///
/// Returns what it did, one line per move, for the caller to log: it runs
/// before logging is set up, because a logger that opens its file under the
/// new data folder would create that folder and so block the move into it.
#[must_use = "log what the migration did once logging is up"]
pub fn migrate() -> Vec<String> {
    imp::migrate();
    std::mem::take(&mut *notes())
}

fn notes() -> std::sync::MutexGuard<'static, Vec<String>> {
    static NOTES: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
    NOTES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg_attr(target_family = "wasm", allow(dead_code))]
fn note(line: String) {
    notes().push(line);
}

/// The data folder's new name, and the prefix per-account folders share.
pub const NEW: &str = "velo";
const OLD: &str = "oxidezap";

/// What an entry in a base folder is called after the move, if it is one of
/// ours: the folder itself, or a per-account `<name>-<account>` beside it.
fn renamed(name: &str) -> Option<String> {
    if name == OLD {
        return Some(NEW.to_owned());
    }
    let account = name.strip_prefix(OLD)?.strip_prefix('-')?;
    (!account.is_empty()).then(|| format!("{NEW}-{account}"))
}

#[cfg(target_family = "wasm")]
mod imp {
    pub(super) fn migrate() {}
}

#[cfg(not(target_family = "wasm"))]
mod imp {
    use std::path::{Path, PathBuf};

    pub(super) fn migrate() {
        for base in bases() {
            rename_children(&base);
        }
        #[cfg(windows)]
        {
            downloads();
            windows::registry();
        }
    }

    /// The folders Velo keeps state under, on each platform.
    fn bases() -> Vec<PathBuf> {
        let mut bases = Vec::new();
        let var = |name: &str| {
            std::env::var_os(name)
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
        };
        #[cfg(windows)]
        {
            bases.extend(var("LOCALAPPDATA"));
            bases.extend(var("APPDATA"));
        }
        #[cfg(not(windows))]
        {
            let home = var("HOME");
            let xdg = |name: &str, fallback: &str| {
                var(name).or_else(|| home.as_ref().map(|home| home.join(fallback)))
            };
            bases.extend(xdg("XDG_DATA_HOME", ".local/share"));
            bases.extend(xdg("XDG_CONFIG_HOME", ".config"));
            bases.extend(xdg("XDG_STATE_HOME", ".local/state"));
            bases.extend(xdg("XDG_CACHE_HOME", ".cache"));
            #[cfg(target_os = "macos")]
            if let Some(home) = &home {
                bases.push(home.join("Library/Application Support"));
                bases.push(home.join("Library/Caches"));
            }
        }
        bases
    }

    /// Rename every old-named entry directly under `base`.
    pub(super) fn rename_children(base: &Path) {
        let Ok(entries) = std::fs::read_dir(base) else {
            return;
        };
        for entry in entries.flatten() {
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if let Some(new) = super::renamed(&name) {
                move_into_place(&entry.path(), &base.join(new));
            }
        }
    }

    /// Rename `from` to `to` unless `to` already exists. A file held open by
    /// a program that is still shutting down is retried briefly.
    pub(super) fn move_into_place(from: &Path, to: &Path) {
        if to.exists() {
            super::note(format!(
                "{} was not moved: {} already exists",
                from.display(),
                to.display()
            ));
            return;
        }
        let mut last = None;
        for _ in 0..10 {
            match std::fs::rename(from, to) {
                Ok(()) => {
                    super::note(format!("moved {} to {}", from.display(), to.display()));
                    return;
                }
                // Another Velo program starting at the same moment got there
                // first.
                Err(_) if !from.exists() && to.exists() => return,
                Err(error) => {
                    last = Some(error);
                    std::thread::sleep(std::time::Duration::from_millis(200));
                }
            }
        }
        if let Some(error) = last {
            super::note(format!("{} could not be moved: {error}", from.display()));
        }
    }

    /// Saved files: `Downloads\OxideZap` becomes `Downloads\Velo`.
    #[cfg(windows)]
    fn downloads() {
        let Some(profile) = std::env::var_os("USERPROFILE").filter(|v| !v.is_empty()) else {
            return;
        };
        let downloads = PathBuf::from(profile).join("Downloads");
        let old = downloads.join("OxideZap");
        if old.is_dir() {
            move_into_place(&old, &downloads.join("Velo"));
        }
    }

    #[cfg(windows)]
    mod windows {
        use windows_registry::CURRENT_USER;

        const OLD_PREFS: &str = r"Software\OxideZap";
        const NEW_PREFS: &str = r"Software\Velo";
        const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
        const OLD_RUN: &str = "OxideZap";
        const NEW_RUN: &str = "Velo";
        const OLD_APP_ID: &str = r"Software\Classes\AppUserModelId\org.oxidezap.client.local";

        pub(super) fn registry() {
            preferences();
            sign_in_entry();
            // The old notification identity: Windows keeps showing its name
            // in Settings until the key is gone.
            let _ = CURRENT_USER.remove_tree(OLD_APP_ID);
        }

        /// Copy each saved preference the new key does not have yet, then
        /// drop the old key.
        fn preferences() {
            let Ok(old) = CURRENT_USER.open(OLD_PREFS) else {
                return;
            };
            let Ok(new) = CURRENT_USER.create(NEW_PREFS) else {
                return;
            };
            if let Ok(values) = old.values() {
                for (name, value) in values {
                    if new.get_value(&name).is_err() {
                        let _ = new.set_value(&name, &value);
                    }
                }
            }
            drop(old);
            let _ = CURRENT_USER.remove_tree(OLD_PREFS);
        }

        /// "Start Velo when you sign in" was stored under the old name and
        /// pointed at the old program. Carry the choice over to the new name
        /// and program; the installer writes the same, this covers a portable
        /// copy that never ran it.
        fn sign_in_entry() {
            let Ok(run) = CURRENT_USER.create(RUN) else {
                return;
            };
            if run.get_string(OLD_RUN).is_err() {
                return;
            }
            let _ = run.remove_value(OLD_RUN);
            if run.get_string(NEW_RUN).is_ok() {
                return;
            }
            let service = std::env::current_exe()
                .ok()
                .and_then(|exe| exe.parent().map(|dir| dir.join("velod.exe")))
                .filter(|path| path.is_file());
            if let Some(service) = service {
                let _ = run.set_string(NEW_RUN, format!("\"{}\"", service.display()));
            }
        }
    }
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use super::*;

    #[test]
    fn the_data_folder_and_account_folders_take_the_new_name() {
        assert_eq!(renamed("oxidezap").as_deref(), Some("velo"));
        assert_eq!(renamed("oxidezap-work").as_deref(), Some("velo-work"));
        assert_eq!(renamed("oxidezap-"), None);
        assert_eq!(renamed("oxidezapper"), None);
        assert_eq!(renamed("velo"), None);
        assert_eq!(renamed("Something else"), None);
    }

    #[test]
    fn folders_move_once_and_never_over_an_existing_one() {
        let base = std::env::temp_dir().join(format!("velo-legacy-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("oxidezap")).unwrap();
        std::fs::write(base.join("oxidezap").join("whatsapp.db"), b"store").unwrap();
        std::fs::create_dir_all(base.join("oxidezap-work")).unwrap();
        std::fs::create_dir_all(base.join("velo-home")).unwrap();
        std::fs::create_dir_all(base.join("oxidezap-home")).unwrap();

        imp::rename_children(&base);

        // Moved, contents intact.
        assert_eq!(
            std::fs::read(base.join("velo").join("whatsapp.db")).unwrap(),
            b"store"
        );
        assert!(!base.join("oxidezap").exists());
        assert!(base.join("velo-work").is_dir());
        // A new-named folder already there is never replaced: the old one is
        // left for a person to look at rather than merged or lost.
        assert!(base.join("oxidezap-home").is_dir());

        // Running again changes nothing.
        imp::rename_children(&base);
        assert!(base.join("velo").join("whatsapp.db").is_file());
        let _ = std::fs::remove_dir_all(&base);
    }
}
