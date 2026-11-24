use std::collections::VecDeque;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{OnceLock, Weak, mpsc};
use windows::Foundation::TypedEventHandler;
use windows::UI::Notifications::{ToastNotification, ToastNotifier};
use windows::Win32::System::WinRT::{RO_INIT_SINGLETHREADED, RoInitialize, RoUninitialize};
use windows::core::IInspectable;

use crate::state::StateHub;
use velo_platform::windows_desktop;

struct Notice {
    hub: Weak<StateHub>,
    jid: String,
    title: String,
    body: String,
}

impl Notice {
    /// Groups get their own, lower chime.
    fn chime(&self) -> windows_desktop::Chime {
        if self.jid.ends_with("@g.us") {
            windows_desktop::Chime::Group
        } else {
            windows_desktop::Chime::Message
        }
    }
}

static NOTICES: OnceLock<mpsc::SyncSender<Notice>> = OnceLock::new();

pub(crate) fn deliver(hub: Weak<StateHub>, jid: &str, title: String, body: String) {
    if let Some(notices) = NOTICES.get() {
        let notice = Notice {
            hub,
            jid: jid.to_owned(),
            title: title.chars().take(128).collect(),
            body: body.chars().take(240).collect(),
        };
        if let Err(error) = notices.try_send(notice) {
            // Log queue state without logging the incoming message.
            log::warn!("could not queue a background notification: {error}");
        }
    }
}

pub(super) struct Receiver {
    notices: mpsc::Receiver<Notice>,
    notifier: Option<ToastNotifier>,
    toasts: VecDeque<ToastNotification>,
    initialized: bool,
}

impl Receiver {
    pub(super) fn new() -> Self {
        let (sender, notices) = mpsc::sync_channel(128);
        let _ = NOTICES.set(sender);
        // The receiver and its WinRT objects stay on this tray thread.
        let initialized = unsafe { RoInitialize(RO_INIT_SINGLETHREADED) }.is_ok();
        Self {
            notices,
            notifier: None,
            toasts: VecDeque::new(),
            initialized,
        }
    }

    pub(super) fn drain(&mut self) {
        for _ in 0..16 {
            let Ok(notice) = self.notices.try_recv() else {
                break;
            };
            if let Err(error) = self.show(notice) {
                log::warn!("could not show a background notification: {error}");
            }
        }
    }

    fn show(&mut self, notice: Notice) -> windows::core::Result<()> {
        let Some(hub) = notice.hub.upgrade() else {
            return Ok(());
        };
        if hub.is_logged_out() {
            return Ok(());
        }
        if self.notifier.is_none() {
            self.notifier = Some(windows_desktop::notifier()?);
        }
        let Some(notifier) = self.notifier.as_ref() else {
            return Ok(());
        };
        let toast = windows_desktop::toast(&notice.title, &notice.body)?;
        let mut tag = DefaultHasher::new();
        hub.account_id().hash(&mut tag);
        notice.jid.hash(&mut tag);
        toast.SetTag(&format!("{:016x}", tag.finish()).as_str().into())?;
        let generation = hub.account_generation();
        let chime = notice.chime();
        toast.Activated(&TypedEventHandler::<ToastNotification, IInspectable>::new(
            move |_, _| {
                if let Some(hub) = notice.hub.upgrade()
                    && hub.account_generation() == generation
                {
                    // Before `show`: a window started by it reads the chat
                    // on attach, and a running one is told now.
                    hub.open_chat(notice.jid.clone());
                    crate::window::show(&hub);
                }
                Ok(())
            },
        ))?;
        notifier.Show(&toast)?;
        windows_desktop::chime_kind_for(notifier, chime);
        self.toasts.push_back(toast);
        if self.toasts.len() > 48 {
            self.toasts.pop_front();
        }
        Ok(())
    }
}

impl Drop for Receiver {
    fn drop(&mut self) {
        self.toasts.clear();
        self.notifier.take();
        if self.initialized {
            // Balanced with this thread's successful RoInitialize.
            unsafe { RoUninitialize() };
        }
    }
}
