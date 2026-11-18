pub struct DesktopOptions {
    pub startup: bool,
    pub sound: bool,
    /// Chime volume, 0–100.
    pub volume: u32,
}

pub fn desktop_options() -> Option<DesktopOptions> {
    imp::desktop_options()
}

pub fn set_start_at_sign_in(enabled: bool) -> Result<(), String> {
    imp::set_start_at_sign_in(enabled)
}

pub fn set_message_sound(enabled: bool) -> Result<(), String> {
    imp::set_message_sound(enabled)
}

pub fn set_chime_volume(volume: u32) -> Result<(), String> {
    imp::set_chime_volume(volume)
}

/// Play the chime now, at the saved volume — Settings' preview. `group`
/// picks the group sound.
pub fn preview_chime(group: bool) -> bool {
    imp::preview_chime(group)
}

pub fn test_message_alert() -> Result<(), String> {
    imp::test_message_alert()
}

pub fn play_message_chime(group: bool) {
    imp::play_message_chime(group);
}

#[cfg(windows)]
mod imp {
    use super::DesktopOptions;
    use oxidezap_platform::windows_desktop::{self, Chime};

    fn chime(group: bool) -> Chime {
        if group { Chime::Group } else { Chime::Message }
    }

    pub(super) fn desktop_options() -> Option<DesktopOptions> {
        Some(DesktopOptions {
            startup: windows_desktop::start_at_sign_in(),
            sound: windows_desktop::sound_enabled(),
            volume: windows_desktop::chime_volume(),
        })
    }

    pub(super) fn set_start_at_sign_in(enabled: bool) -> Result<(), String> {
        let daemon = std::env::current_exe()
            .map_err(|error| error.to_string())?
            .with_file_name("oxidezapd.exe");
        windows_desktop::set_start_at_sign_in(enabled, &daemon)
    }

    pub(super) fn set_message_sound(enabled: bool) -> Result<(), String> {
        windows_desktop::set_sound_enabled(enabled)
    }

    pub(super) fn set_chime_volume(volume: u32) -> Result<(), String> {
        windows_desktop::set_chime_volume(volume)
    }

    pub(super) fn preview_chime(group: bool) -> bool {
        windows_desktop::play(chime(group), windows_desktop::chime_volume())
    }

    pub(super) fn test_message_alert() -> Result<(), String> {
        windows_desktop::test_message_alert()
    }

    pub(super) fn play_message_chime(group: bool) {
        if let Ok(notifier) = windows_desktop::notifier() {
            windows_desktop::chime_kind_for(&notifier, chime(group));
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::DesktopOptions;
    pub(super) fn desktop_options() -> Option<DesktopOptions> {
        None
    }
    pub(super) fn set_start_at_sign_in(_: bool) -> Result<(), String> {
        Err("Startup controls are currently available on Windows.".into())
    }
    pub(super) fn set_message_sound(_: bool) -> Result<(), String> {
        Err("The custom chime is currently available on Windows.".into())
    }
    pub(super) fn set_chime_volume(_: u32) -> Result<(), String> {
        Err("The custom chime is currently available on Windows.".into())
    }
    pub(super) fn preview_chime(_: bool) -> bool {
        false
    }
    pub(super) fn test_message_alert() -> Result<(), String> {
        Err("Windows notification testing is available on Windows.".into())
    }
    pub(super) fn play_message_chime(_: bool) {}
}
