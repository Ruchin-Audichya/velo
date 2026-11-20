//! Windows startup and native notifications shared by the window and daemon.

use std::path::Path;
use std::ptr::null_mut;
use windows::Data::Xml::Dom::XmlDocument;
use windows::UI::Notifications::{
    NotificationSetting, ToastNotification, ToastNotificationManager, ToastNotifier,
};
use windows::core::{HSTRING, h};
use windows_registry::CURRENT_USER;
use windows_sys::Win32::Media::Audio::{PlaySoundW, SND_MEMORY, SND_NODEFAULT, SND_SYNC};
use windows_sys::Win32::UI::Shell::{QUNS_ACCEPTS_NOTIFICATIONS, SHQueryUserNotificationState};

pub const APP_ID: &str = "io.github.RuchinAudichya.Velo";
const PREFS: &str = r"Software\Velo";
const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const STARTUP_NAME: &str = "Velo";
const SOUND_NAME: &str = "MessageSound";
const VOLUME_NAME: &str = "ChimeVolume";
const DEFAULT_VOLUME: u32 = 80;
/// Messages closer together than this share one chime.
const BURST_WINDOW: std::time::Duration = std::time::Duration::from_millis(1500);

pub fn start_at_sign_in() -> bool {
    CURRENT_USER
        .open(RUN)
        .and_then(|key| key.get_string(STARTUP_NAME))
        .is_ok()
}

pub fn set_start_at_sign_in(enabled: bool, daemon: &Path) -> Result<(), String> {
    let key = CURRENT_USER
        .create(RUN)
        .map_err(|error| error.to_string())?;
    if enabled {
        if !daemon.is_file() {
            return Err("The Velo background executable is missing.".into());
        }
        key.set_string(STARTUP_NAME, format!("\"{}\"", daemon.display()))
            .map_err(|error| error.to_string())
    } else if key.get_string(STARTUP_NAME).is_ok() {
        key.remove_value(STARTUP_NAME)
            .map_err(|error| error.to_string())
    } else {
        Ok(())
    }
}

pub fn sound_enabled() -> bool {
    CURRENT_USER
        .open(PREFS)
        .and_then(|key| key.get_u32(SOUND_NAME))
        .unwrap_or(1)
        != 0
}

pub fn set_sound_enabled(enabled: bool) -> Result<(), String> {
    CURRENT_USER
        .create(PREFS)
        .and_then(|key| key.set_u32(SOUND_NAME, u32::from(enabled)))
        .map_err(|error| error.to_string())
}

/// Which sound to play. A group message is lower and falls, so the two can
/// be told apart without looking. See `packaging/windows/make-chimes.py`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chime {
    Message,
    Group,
}

impl Chime {
    fn wav(self) -> &'static [u8] {
        match self {
            Chime::Message => include_bytes!("../../../packaging/windows/soft-chime.wav"),
            Chime::Group => include_bytes!("../../../packaging/windows/group-chime.wav"),
        }
    }
}

/// Chime volume, 0–100. 80 by default: the chime is mastered quiet already.
pub fn chime_volume() -> u32 {
    CURRENT_USER
        .open(PREFS)
        .and_then(|key| key.get_u32(VOLUME_NAME))
        .unwrap_or(DEFAULT_VOLUME)
        .min(100)
}

pub fn set_chime_volume(volume: u32) -> Result<(), String> {
    CURRENT_USER
        .create(PREFS)
        .and_then(|key| key.set_u32(VOLUME_NAME, volume.min(100)))
        .map_err(|error| error.to_string())
}

/// Play the message chime now, at the saved volume, ignoring the burst
/// limit. What Settings' preview button uses.
pub fn play_chime() -> bool {
    play(Chime::Message, chime_volume())
}

/// Play `chime` at `volume`, on a short-lived thread.
///
/// PlaySound has no volume of its own, so the samples are scaled into an
/// owned copy, and played synchronously on that thread so the copy outlives
/// the playback — an asynchronous play would read a buffer already freed.
pub fn play(chime: Chime, volume: u32) -> bool {
    if volume == 0 {
        return true;
    }
    let wav = scaled(chime.wav(), volume);
    std::thread::Builder::new()
        .name("chime".into())
        .spawn(move || {
            // The buffer is owned by this thread for the whole call.
            unsafe {
                PlaySoundW(
                    wav.as_ptr().cast(),
                    null_mut(),
                    SND_SYNC | SND_MEMORY | SND_NODEFAULT,
                )
            };
        })
        .is_ok()
}

/// A copy of a 16-bit PCM WAV with its samples scaled to `volume` percent.
///
/// Walks the RIFF chunks for `data` rather than assuming the canonical
/// 44-byte header, so a re-rendered file with extra chunks still plays.
fn scaled(wav: &[u8], volume: u32) -> Vec<u8> {
    let mut out = wav.to_vec();
    if volume >= 100 {
        return out;
    }
    let gain = volume as f32 / 100.0;
    let mut at = 12;
    while at + 8 <= out.len() {
        let id = &out[at..at + 4];
        let len = u32::from_le_bytes([out[at + 4], out[at + 5], out[at + 6], out[at + 7]]) as usize;
        let body = at + 8;
        if id == b"data" {
            let end = (body + len).min(out.len());
            let (samples, _) = out[body..end].as_chunks_mut::<2>();
            for sample in samples {
                let value = i16::from_le_bytes(*sample) as f32 * gain;
                *sample = (value as i16).to_le_bytes();
            }
            break;
        }
        at = body + len + (len & 1);
    }
    out
}

/// Whether a chime may sound now, and if so, claim the slot.
///
/// A burst of messages — a busy group, a reconnect catching up — is one
/// sound, not a drum roll. Anything within the window of the last chime is
/// silent; its banner still shows.
fn claim_chime_slot() -> bool {
    use std::sync::Mutex;
    // Windows-only and never compiled for a page, which is the reason the
    // workspace routes clocks through wacore; this crate sits below it.
    #[allow(clippy::disallowed_methods)]
    let now = std::time::Instant::now();
    static LAST: Mutex<Option<std::time::Instant>> = Mutex::new(None);
    let Ok(mut last) = LAST.lock() else {
        return true;
    };
    if last.is_some_and(|at| now.duration_since(at) < BURST_WINDOW) {
        return false;
    }
    *last = Some(now);
    true
}

/// The logo a toast shows beside "Velo", and Settings beside the app's
/// notification switch.
const LOGO: &[u8] = include_bytes!("../../../packaging/windows/velo.png");

pub fn notifier() -> windows::core::Result<ToastNotifier> {
    register_identity()?;
    ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(APP_ID))
}

/// Tell Windows what Velo's notifications are called and what they look
/// like. Done when a notifier is made, and up front by the daemon and the
/// installer, so the very first toast — and Settings' notification list —
/// already carry the name and logo.
pub fn register_identity() -> windows::core::Result<()> {
    let key = CURRENT_USER.create(format!(r"Software\Classes\AppUserModelId\{APP_ID}"))?;
    key.set_string("DisplayName", "Velo")?;
    // An unpackaged app has no manifest to carry its logo, so Windows reads
    // it from this file path instead; without one, every toast shows a
    // blank square. A failure to write it costs the picture, not the toast.
    if let Some(logo) = logo_file() {
        key.set_string("IconUri", logo.display().to_string())?;
        // The tile behind the logo; transparent lets the logo's own tile
        // stand on whatever the toast is drawn on.
        key.set_string("IconBackgroundColor", "00000000")?;
    }
    Ok(())
}

/// The logo, written where Windows can read it: beside Velo's data, and
/// rewritten only when this build carries a different one.
fn logo_file() -> Option<std::path::PathBuf> {
    let dir =
        std::path::PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join(crate::legacy_names::NEW);
    let path = dir.join("notification-logo.png");
    let current = std::fs::metadata(&path).is_ok_and(|meta| meta.len() == LOGO.len() as u64)
        && std::fs::read(&path).is_ok_and(|bytes| bytes == LOGO);
    if !current {
        std::fs::create_dir_all(&dir).ok()?;
        std::fs::write(&path, LOGO).ok()?;
    }
    Some(path)
}

/// Run the local Windows delivery path without fetching or sending a message.
pub fn test_message_alert() -> Result<(), String> {
    let notifier = notifier().map_err(|error| error.to_string())?;
    let setting = notifier.Setting().map_err(|error| error.to_string())?;
    if let Some(reason) = notification_block_reason(setting) {
        return Err(reason.to_owned());
    }

    let notice = toast("Velo", "Your Windows notification test is working.")
        .map_err(|error| error.to_string())?;
    notifier.Show(&notice).map_err(|error| error.to_string())?;
    chime_for(&notifier);
    Ok(())
}

fn notification_block_reason(setting: NotificationSetting) -> Option<&'static str> {
    match setting {
        NotificationSetting::Enabled => None,
        NotificationSetting::DisabledForUser => {
            Some("Turn on Notifications in Windows Settings → System → Notifications.")
        }
        NotificationSetting::DisabledForApplication => {
            Some("Turn on Velo in Windows Settings → System → Notifications.")
        }
        NotificationSetting::DisabledByGroupPolicy => {
            Some("A Windows administrator policy blocks notifications on this device.")
        }
        _ => Some("Windows is blocking Velo notifications."),
    }
}

pub fn chime_for(notifier: &ToastNotifier) {
    chime_kind_for(notifier, Chime::Message);
}

/// Sound `chime` for an incoming message, if every switch that governs it
/// says yes: the app's own toggle, Windows' notification setting for this
/// app and its sound, Focus / presentation mode, and the burst limit.
pub fn chime_kind_for(notifier: &ToastNotifier, chime: Chime) {
    if !sound_enabled() || notifier.Setting().ok() != Some(NotificationSetting::Enabled) {
        return;
    }
    let settings =
        format!(r"Software\Microsoft\Windows\CurrentVersion\Notifications\Settings\{APP_ID}");
    if CURRENT_USER
        .open(settings)
        .and_then(|key| key.get_u32("AllowSound"))
        .ok()
        == Some(0)
    {
        return;
    }
    let mut state = 0;
    // A valid writable output for the duration of the platform call.
    if unsafe { SHQueryUserNotificationState(&mut state) } >= 0
        && state == QUNS_ACCEPTS_NOTIFICATIONS
        && claim_chime_slot()
    {
        play(chime, chime_volume());
    }
}

pub fn toast(title: &str, body: &str) -> windows::core::Result<ToastNotification> {
    let document = XmlDocument::new()?;
    let toast = document.CreateElement(h!("toast"))?;
    document.AppendChild(&toast)?;
    let visual = document.CreateElement(h!("visual"))?;
    toast.AppendChild(&visual)?;
    let binding = document.CreateElement(h!("binding"))?;
    binding.SetAttribute(h!("template"), h!("ToastGeneric"))?;
    visual.AppendChild(&binding)?;
    for value in [title, body] {
        let text = document.CreateElement(h!("text"))?;
        text.SetInnerText(&HSTRING::from(value))?;
        binding.AppendChild(&text)?;
    }
    let audio = document.CreateElement(h!("audio"))?;
    audio.SetAttribute(h!("silent"), h!("true"))?;
    toast.AppendChild(&audio)?;
    ToastNotification::CreateToastNotification(&document)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distinguishes_windows_and_app_notification_settings() {
        assert_eq!(
            notification_block_reason(NotificationSetting::Enabled),
            None
        );
        let global = notification_block_reason(NotificationSetting::DisabledForUser).unwrap();
        let individual =
            notification_block_reason(NotificationSetting::DisabledForApplication).unwrap();
        assert!(global.contains("Turn on Notifications"));
        assert!(individual.contains("Turn on Velo"));
        assert!(notification_block_reason(NotificationSetting::DisabledByGroupPolicy).is_some());
        assert!(notification_block_reason(NotificationSetting(i32::MAX)).is_some());
    }

    fn data_samples(wav: &[u8]) -> Vec<i16> {
        let at = wav
            .windows(4)
            .position(|w| w == b"data")
            .expect("data chunk")
            + 8;
        wav[at..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|s| i16::from_le_bytes(*s))
            .collect()
    }

    #[test]
    fn volume_scales_samples_and_leaves_the_header_alone() {
        for chime in [Chime::Message, Chime::Group] {
            let full = chime.wav();
            let half = scaled(full, 50);
            assert_eq!(half.len(), full.len());
            let header = full.windows(4).position(|w| w == b"data").unwrap() + 8;
            assert_eq!(&half[..header], &full[..header], "header untouched");
            let peak = |s: &[i16]| s.iter().map(|v| v.unsigned_abs()).max().unwrap();
            let (full_peak, half_peak) = (peak(&data_samples(full)), peak(&data_samples(&half)));
            assert!(full_peak > 1000, "the chime is not silent");
            assert!(half_peak.abs_diff(full_peak / 2) <= 1);
            assert_eq!(scaled(full, 100), full);
        }
    }

    #[test]
    fn a_burst_of_messages_gets_one_chime() {
        assert!(claim_chime_slot());
        assert!(
            !claim_chime_slot(),
            "a second message right behind is silent"
        );
    }
}
