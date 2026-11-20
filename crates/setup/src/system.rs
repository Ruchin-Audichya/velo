//! What installing and removing actually do to the machine.
//!
//! Everything here is per user: files under `%LOCALAPPDATA%\Programs\Velo`,
//! shortcuts in the user's Start menu and desktop, and values under
//! `HKEY_CURRENT_USER`. Nothing asks for an administrator.

use std::os::windows::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use windows_registry::CURRENT_USER;

use crate::payload;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
const PUBLISHER: &str = "Ruchin Audichya";
const UNINSTALL_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Velo";
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
/// The Run value Velo's own "Start at sign-in" switch reads and writes.
const RUN_NAME: &str = "Velo";
/// Velo's taskbar and notification identity; the window sets the same one.
const APP_ID: &str = "io.github.RuchinAudichya.Velo";
const WINDOW_EXE: &str = "velo.exe";
const DAEMON_EXE: &str = "velod.exe";
const UNINSTALLER: &str = "uninstall.exe";
/// What 1.0 called the programs, before Velo used its own names. An update
/// stops and deletes them and repoints any shortcut still aimed at one; the
/// folders and registry values they used are moved by
/// `velo_platform::legacy_names`, which every Velo program also runs.
const LEGACY_EXES: [&str; 3] = ["oxidezap.exe", "oxidezapd.exe", "oxidezap-cli.exe"];
/// No console window flashing up for the helpers this runs.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const DETACHED_PROCESS: u32 = 0x0000_0008;

/// The choices on the first screen.
#[derive(Clone, Copy)]
pub struct Options {
    pub desktop_shortcut: bool,
    pub start_at_sign_in: bool,
}

/// Where the installer is, as the progress line says it.
pub enum Step {
    Stopping,
    Copying(f32),
    Shortcuts,
    Registering,
    Removing,
}

pub fn install_dir() -> PathBuf {
    local_app_data().join("Programs").join("Velo")
}

fn local_app_data() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Whether Velo is already installed here — the button says "Update" then.
pub fn installed_version() -> Option<String> {
    let key = CURRENT_USER.open(UNINSTALL_KEY).ok()?;
    install_dir().join(WINDOW_EXE).is_file().then_some(())?;
    key.get_string("DisplayVersion").ok()
}

/// Whether this PC already holds a Velo sign-in, kept from an earlier
/// install — the finish screen then says so instead of asking for a QR scan.
pub fn has_existing_sign_in() -> bool {
    // 1.0 kept it under its old name; the update moves it before the new
    // version starts.
    ["velo", "oxidezap"]
        .iter()
        .any(|name| local_app_data().join(name).join("whatsapp.db").is_file())
}

/// Whether this process is the copy that lives beside the program — the one
/// "Installed apps" starts to remove it.
pub fn is_installed_copy() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
        .is_some_and(|dir| same_path(&dir, &install_dir()))
}

fn same_path(a: &Path, b: &Path) -> bool {
    let norm = |p: &Path| p.to_string_lossy().trim_end_matches('\\').to_lowercase();
    norm(a) == norm(b)
}

pub fn install(options: Options, progress: &dyn Fn(Step)) -> Result<(), String> {
    let entries = payload::entries()?;
    if entries.is_empty() {
        return Err("This copy of Velo Setup was built without Velo inside it.".into());
    }
    progress(Step::Stopping);
    stop_velo();
    // With nothing of 1.0 running to hold a file open: its data folder (the
    // sign-in and chats), downloads folder and registry values take Velo's
    // names now, so the new version starts signed in. What it did is not
    // shown; a folder it could not move is moved by the program on start.
    let _ = velo_platform::legacy_names::migrate();

    let dir = install_dir();
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Could not create {}: {e}", dir.display()))?;
    let total: u64 = entries.iter().map(|e| e.size).sum::<u64>().max(1);
    let mut written = 0u64;
    for entry in &entries {
        let mut tick = |n: u64| {
            written += n;
            progress(Step::Copying(written as f32 / total as f32));
        };
        payload::extract(entry, &dir, &mut tick)
            .map_err(|e| format!("Could not write {}: {e}", entry.name))?;
    }
    // The uninstaller is this program. A copy launched from the install
    // directory is already in place.
    if let Ok(me) = std::env::current_exe() {
        let target = dir.join(UNINSTALLER);
        if !same_path(&me, &target) {
            std::fs::copy(&me, &target)
                .map_err(|e| format!("Could not write the uninstaller: {e}"))?;
        }
    }

    for old in LEGACY_EXES {
        let _ = std::fs::remove_file(dir.join(old));
    }

    progress(Step::Shortcuts);
    let window = dir.join(WINDOW_EXE);
    repoint_pins(&dir, &window);
    shortcut(&start_menu_link(), &window)?;
    if options.desktop_shortcut {
        shortcut(&desktop_link(), &window)?;
    } else {
        remove_link_into(&desktop_link(), &dir);
    }

    progress(Step::Registering);
    set_start_at_sign_in(options.start_at_sign_in, &dir.join(DAEMON_EXE))?;
    register_uninstaller(&dir, &entries)?;
    refresh_shell();
    Ok(())
}

pub fn uninstall(remove_data: bool, progress: &dyn Fn(Step)) -> Result<(), String> {
    progress(Step::Stopping);
    stop_velo();
    progress(Step::Removing);
    let dir = install_dir();
    remove_link_into(&start_menu_link(), &dir);
    remove_link_into(&desktop_link(), &dir);
    if let Ok(key) = CURRENT_USER.create(RUN_KEY)
        && key.get_string(RUN_NAME).is_ok_and(|value| {
            value
                .to_lowercase()
                .contains(&dir.to_string_lossy().to_lowercase())
        })
    {
        let _ = key.remove_value(RUN_NAME);
    }
    let _ = CURRENT_USER.remove_tree(UNINSTALL_KEY);
    // Every file but the one running this; a short-lived helper takes the
    // directory, with this program in it, once this has exited.
    if let Ok(read) = std::fs::read_dir(&dir) {
        for entry in read.flatten() {
            let path = entry.path();
            if path
                .file_name()
                .is_some_and(|n| n.eq_ignore_ascii_case(UNINSTALLER))
            {
                continue;
            }
            if path.is_dir() {
                let _ = std::fs::remove_dir_all(&path);
            } else {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
    if remove_data {
        // The store, the media, the sign-in: one directory, deleted whole
        // so nothing is left orphaned behind a new device. Then Velo's own
        // preferences and its notification registration.
        // Under 1.0's name as well, for an install never started since the
        // update.
        for name in ["velo", "oxidezap"] {
            let _ = std::fs::remove_dir_all(local_app_data().join(name));
            if let Some(roaming) = std::env::var_os("APPDATA") {
                let _ = std::fs::remove_dir_all(PathBuf::from(roaming).join(name));
            }
        }
        let _ = CURRENT_USER.remove_tree(r"Software\Velo");
        let _ = CURRENT_USER.remove_tree(format!(r"Software\Classes\AppUserModelId\{APP_ID}"));
    }
    refresh_shell();
    // Retried once a second for up to ten minutes: this program is still
    // running, showing "removed", until the person closes it, and the
    // directory cannot go while it does. One attempt after a fixed pause
    // left the folder behind whenever the window stayed open.
    let quoted = dir.display().to_string();
    let _ = std::process::Command::new("cmd")
        .raw_arg(format!(
            "/d /c for /l %i in (1,1,600) do @(ping -n 2 127.0.0.1 >nul & rmdir /s /q \"{quoted}\" 2>nul & if not exist \"{quoted}\" exit /b)"
        ))
        .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
        .spawn();
    Ok(())
}

/// Start the installed window, detached from this process.
pub fn launch() {
    let window = install_dir().join(WINDOW_EXE);
    let _ = std::process::Command::new(&window)
        .current_dir(install_dir())
        .creation_flags(DETACHED_PROCESS)
        .spawn();
}

/// Ask a running Velo to stop, the way its own tray does, and close the
/// window. Only if it does not answer is it ended outright: the store is
/// SQLite in WAL mode, which survives that, but a clean stop is kinder.
fn stop_velo() {
    use velo_wire::request::ClientRequest;
    if let Ok(mut client) = velo_ipc::IpcClient::connect() {
        let hello = ClientRequest::Hello {
            protocol: velo_wire::envelope::CURRENT_PROTOCOL_VERSION,
            client_name: "velo-setup".into(),
            local_account: None,
            account_selection: false,
            read_only: false,
            session_events: false,
        };
        if client.request(hello).is_ok() {
            let _ = client.request(ClientRequest::Shutdown);
        }
        drop(client);
        for _ in 0..48 {
            if velo_ipc::IpcClient::connect().is_err() {
                break;
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    }
    // 1.0's programs listen on a pipe of their old name, which the request
    // above cannot reach; they are ended outright, once.
    for image in [WINDOW_EXE, DAEMON_EXE, LEGACY_EXES[0], LEGACY_EXES[1]] {
        let _ = std::process::Command::new("taskkill")
            .args(["/F", "/T", "/IM", image])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
    }
    std::thread::sleep(Duration::from_millis(400));
}

fn set_start_at_sign_in(enabled: bool, daemon: &Path) -> Result<(), String> {
    let key = CURRENT_USER
        .create(RUN_KEY)
        .map_err(|e| format!("Could not open the sign-in list: {e}"))?;
    if enabled {
        key.set_string(RUN_NAME, format!("\"{}\"", daemon.display()))
            .map_err(|e| format!("Could not start Velo at sign-in: {e}"))
    } else {
        let _ = key.remove_value(RUN_NAME);
        Ok(())
    }
}

fn register_uninstaller(dir: &Path, entries: &[payload::Entry]) -> Result<(), String> {
    let key = CURRENT_USER
        .create(UNINSTALL_KEY)
        .map_err(|e| format!("Could not register Velo with Windows: {e}"))?;
    let uninstaller = dir.join(UNINSTALLER).display().to_string();
    let size_kb = entries.iter().map(|e| e.size).sum::<u64>() / 1024;
    let set = |name: &str, value: String| {
        key.set_string(name, value)
            .map_err(|e| format!("Could not register Velo with Windows: {e}"))
    };
    set("DisplayName", "Velo".into())?;
    set("DisplayVersion", VERSION.into())?;
    set("Publisher", PUBLISHER.into())?;
    set(
        "DisplayIcon",
        format!("{},0", dir.join(WINDOW_EXE).display()),
    )?;
    set("InstallLocation", dir.display().to_string())?;
    set("UninstallString", format!("\"{uninstaller}\" --uninstall"))?;
    set(
        "QuietUninstallString",
        format!("\"{uninstaller}\" --uninstall --quiet"),
    )?;
    set(
        "URLInfoAbout",
        "https://github.com/Ruchin-Audichya/velo".into(),
    )?;
    let _ = key.set_u32("EstimatedSize", u32::try_from(size_kb).unwrap_or(u32::MAX));
    let _ = key.set_u32("NoModify", 1);
    let _ = key.set_u32("NoRepair", 1);
    Ok(())
}

fn start_menu_link() -> PathBuf {
    PathBuf::from(std::env::var_os("APPDATA").unwrap_or_default())
        .join(r"Microsoft\Windows\Start Menu\Programs\Velo.lnk")
}

fn desktop_link() -> PathBuf {
    PathBuf::from(std::env::var_os("USERPROFILE").unwrap_or_default())
        .join("Desktop")
        .join("Velo.lnk")
}

/// A taskbar pin made from 1.0's running window is named after that program
/// and aimed at it, and the update deletes it. Each such pin is aimed at the
/// new window in place, under the same file name, so the pin keeps its spot.
fn repoint_pins(dir: &Path, window: &Path) {
    let pins = PathBuf::from(std::env::var_os("APPDATA").unwrap_or_default())
        .join(r"Microsoft\Internet Explorer\Quick Launch\User Pinned\TaskBar");
    let Ok(read) = std::fs::read_dir(pins) else {
        return;
    };
    for link in read.flatten().map(|entry| entry.path()) {
        let is_old = link_target(&link).is_some_and(|target| {
            target.parent().is_some_and(|parent| same_path(parent, dir))
                && target.file_name().is_some_and(|name| {
                    LEGACY_EXES.iter().any(|old| name.eq_ignore_ascii_case(old))
                })
        });
        if is_old {
            let _ = shortcut(&link, window);
        }
    }
}

/// Remove a shortcut only if it is ours: a "Velo.lnk" the user made to
/// something else is left alone.
fn remove_link_into(link: &Path, dir: &Path) {
    if link.is_file() && link_target(link).is_some_and(|target| target.starts_with(dir)) {
        let _ = std::fs::remove_file(link);
    }
}

fn link_target(link: &Path) -> Option<PathBuf> {
    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
        IPersistFile, STGM_READ,
    };
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
    use windows::core::{HSTRING, Interface as _};
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let link_object: IShellLinkW =
            CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
        let file: IPersistFile = link_object.cast().ok()?;
        file.Load(&HSTRING::from(link.as_os_str()), STGM_READ)
            .ok()?;
        let mut buffer = [0u16; 1024];
        link_object
            .GetPath(&mut buffer, std::ptr::null_mut(), 0)
            .ok()?;
        let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
        Some(PathBuf::from(String::from_utf16_lossy(&buffer[..len])))
    }
}

/// A shortcut to `target`, carrying Velo's app ID so the taskbar groups the
/// pinned icon with the running window instead of showing two.
fn shortcut(link: &Path, target: &Path) -> Result<(), String> {
    use windows::Win32::Storage::EnhancedStorage::PKEY_AppUserModel_ID;
    use windows::Win32::System::Com::StructuredStorage::{
        PROPVARIANT, PROPVARIANT_0, PROPVARIANT_0_0, PROPVARIANT_0_0_0,
    };
    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
        IPersistFile,
    };
    use windows::Win32::System::Variant::VT_LPWSTR;
    use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
    use windows::core::{HSTRING, Interface as _, PWSTR};

    let fail = |e: windows::core::Error| format!("Could not create {}: {e}", link.display());
    if let Some(parent) = link.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let link_object: IShellLinkW =
            CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).map_err(fail)?;
        link_object
            .SetPath(&HSTRING::from(target.as_os_str()))
            .map_err(fail)?;
        if let Some(dir) = target.parent() {
            link_object
                .SetWorkingDirectory(&HSTRING::from(dir.as_os_str()))
                .map_err(fail)?;
        }
        link_object
            .SetDescription(&HSTRING::from("Velo — WhatsApp for Windows"))
            .map_err(fail)?;
        link_object
            .SetIconLocation(&HSTRING::from(target.as_os_str()), 0)
            .map_err(fail)?;

        let store: IPropertyStore = link_object.cast().map_err(fail)?;
        let mut id: Vec<u16> = APP_ID.encode_utf16().chain(std::iter::once(0)).collect();
        let value = PROPVARIANT {
            Anonymous: PROPVARIANT_0 {
                Anonymous: std::mem::ManuallyDrop::new(PROPVARIANT_0_0 {
                    vt: VT_LPWSTR,
                    wReserved1: 0,
                    wReserved2: 0,
                    wReserved3: 0,
                    Anonymous: PROPVARIANT_0_0_0 {
                        pwszVal: PWSTR(id.as_mut_ptr()),
                    },
                }),
            },
        };
        store
            .SetValue(&PKEY_AppUserModel_ID, &value)
            .map_err(fail)?;
        store.Commit().map_err(fail)?;
        // The string is ours, not the variant's: nothing to clear.
        std::mem::forget(value);

        let file: IPersistFile = link_object.cast().map_err(fail)?;
        file.Save(&HSTRING::from(link.as_os_str()), true)
            .map_err(fail)?;
    }
    Ok(())
}

/// Tell Explorer icons and shortcuts changed, so the Start menu and taskbar
/// show the new ones without a sign-out.
fn refresh_shell() {
    use windows::Win32::UI::Shell::{SHCNE_ASSOCCHANGED, SHCNF_IDLIST, SHChangeNotify};
    unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None) };
}
