//! Velo Setup: installs Velo for the person running it, and removes it again.
//!
//! One binary for both directions. Run as downloaded, it installs; run as
//! the `uninstall.exe` it leaves beside the program (which is what Windows'
//! "Installed apps" page starts), or with `--uninstall`, it removes. The
//! window is drawn with the same toolkit as Velo, so the first thing a new
//! user sees already looks like the app they are about to get.
//!
//! Per user and without elevation: everything lands under
//! `%LOCALAPPDATA%\Programs\Velo` and the current user's registry hive.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

#[cfg(windows)]
mod payload;
#[cfg(windows)]
mod system;
#[cfg(windows)]
mod ui;

fn main() {
    #[cfg(windows)]
    ui::run();
}
