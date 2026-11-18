//! The installer's window: one screen at a time, each a short sentence and
//! one obvious button.
//!
//! The work runs on its own thread and reports progress over a channel the
//! window drains a few times a second, so the window keeps painting while a
//! 60 MB payload is being unpacked.

use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use gpui::{
    Animation, AnimationExt as _, App, AppContext as _, Bounds, Context, FontWeight, Hsla, Image,
    ImageFormat, IntoElement, ParentElement as _, Render, SharedString, Styled as _, Window,
    WindowBounds, WindowOptions, div, ease_out_quint, img, px, rgb, size,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::checkbox::Checkbox;
use gpui_component::progress::Progress;
use gpui_component::theme::{Theme, ThemeMode};
use gpui_component::{ActiveTheme as _, Icon, IconName, Root, Sizable as _, TitleBar};

use crate::system::{self, Options, Step};

/// Velo's green: the logo's, and the app's default accent.
const GREEN: u32 = 0x21c063;
const SURFACE: u32 = 0x161717;
const CARD: u32 = 0x242626;

static LOGO: &[u8] = include_bytes!("../../../packaging/branding/velo-logo-256.png");

pub fn run() {
    let args: Vec<String> = std::env::args().collect();
    let uninstall = args.iter().any(|a| a == "--uninstall") || system::is_installed_copy();
    let quiet = args.iter().any(|a| a == "--quiet");
    if quiet {
        // Unattended: what a package manager or "Installed apps" asks for.
        let _ = if uninstall {
            system::uninstall(false, &|_| {})
        } else {
            system::install(
                Options {
                    desktop_shortcut: true,
                    start_at_sign_in: true,
                },
                &|_| {},
            )
        };
        return;
    }

    gpui_platform::application()
        .with_assets(gpui_kit_assets::Assets)
        .run(move |cx: &mut App| {
            gpui_component::init(cx);
            style(cx);
            let bounds = Bounds::centered(None, size(px(560.0), px(520.0)), cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                is_resizable: false,
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some(SharedString::from(if uninstall {
                        "Remove Velo"
                    } else {
                        "Velo Setup"
                    })),
                    ..TitleBar::title_bar_options()
                }),
                ..Default::default()
            };
            let opened = cx.open_window(options, |window, cx| {
                let view = cx.new(|_| Setup::new(uninstall));
                cx.new(|cx| Root::new(view, window, cx))
            });
            if opened.is_err() {
                cx.quit();
            }
            cx.activate(true);
        });
}

/// Dark, with Velo's green as the one accent — the app's first impression.
fn style(cx: &mut App) {
    Theme::change(ThemeMode::Dark, None, cx);
    let theme = cx.global_mut::<Theme>();
    theme.font_family = "Segoe UI Variable Text".into();
    let green: Hsla = rgb(GREEN).into();
    let c = &mut theme.colors;
    c.background = rgb(SURFACE).into();
    c.title_bar = rgb(SURFACE).into();
    c.title_bar_border = rgb(SURFACE).into();
    c.popover = rgb(CARD).into();
    c.secondary = rgb(CARD).into();
    c.primary = green;
    c.primary_hover = green.opacity(0.88);
    c.primary_active = green.opacity(0.78);
    // Dark ink on the green: white on it is under 3:1, this is over 7.
    c.primary_foreground = rgb(0x0b2b19).into();
    c.ring = green;
    c.button_primary = green;
    c.button_primary_hover = green.opacity(0.88);
    c.button_primary_active = green.opacity(0.78);
    c.button_primary_foreground = rgb(0x0b2b19).into();
    // The controls read a derived table, not the colours directly; rebuild
    // it, or the buttons and checkboxes keep the stock theme's white.
    theme.tokens = gpui_component::theme::ThemeTokens::from(&theme.colors);
    Theme::sync_base(cx);
}

enum Screen {
    Welcome,
    Working { line: SharedString, fraction: f32 },
    Done,
    Failed(SharedString),
    ConfirmRemove,
    Removed,
}

impl Screen {
    fn key(&self) -> &'static str {
        match self {
            Self::Welcome => "welcome",
            Self::Working { .. } => "working",
            Self::Done => "done",
            Self::Failed(_) => "failed",
            Self::ConfirmRemove => "confirm",
            Self::Removed => "removed",
        }
    }
}

enum Report {
    Step(Step),
    Finished(Result<(), String>),
}

struct Setup {
    uninstall: bool,
    screen: Screen,
    existing: Option<String>,
    desktop_shortcut: bool,
    start_at_sign_in: bool,
    open_when_done: bool,
    remove_data: bool,
    /// A sign-in kept from before; read once, before anything installs.
    signed_in: bool,
    logo: Arc<Image>,
}

impl Setup {
    fn new(uninstall: bool) -> Self {
        Self {
            uninstall,
            screen: if uninstall {
                Screen::ConfirmRemove
            } else {
                Screen::Welcome
            },
            existing: system::installed_version(),
            desktop_shortcut: true,
            start_at_sign_in: true,
            open_when_done: true,
            remove_data: false,
            signed_in: system::has_existing_sign_in(),
            logo: Arc::new(Image::from_bytes(ImageFormat::Png, LOGO.to_vec())),
        }
    }

    fn start(&mut self, cx: &mut Context<Self>) {
        let (tx, rx) = channel();
        let uninstall = self.uninstall;
        let remove_data = self.remove_data;
        let options = Options {
            desktop_shortcut: self.desktop_shortcut,
            start_at_sign_in: self.start_at_sign_in,
        };
        self.screen = Screen::Working {
            line: "Getting ready…".into(),
            fraction: 0.0,
        };
        std::thread::spawn(move || {
            let report = |step| {
                let _ = tx.send(Report::Step(step));
            };
            let result = if uninstall {
                system::uninstall(remove_data, &report)
            } else {
                system::install(options, &report)
            };
            let _ = tx.send(Report::Finished(result));
        });
        self.follow(rx, cx);
        cx.notify();
    }

    /// Drain the worker's reports into the screen until it finishes.
    fn follow(&self, rx: Receiver<Report>, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(60))
                    .await;
                let mut finished = None;
                let mut latest = None;
                while let Ok(report) = rx.try_recv() {
                    match report {
                        Report::Step(step) => latest = Some(step),
                        Report::Finished(result) => finished = Some(result),
                    }
                }
                let alive = this.update(cx, |setup, cx| {
                    if let Some(step) = latest {
                        let (line, fraction) = describe(&step, setup.uninstall);
                        setup.screen = Screen::Working { line, fraction };
                    }
                    if let Some(result) = finished.take() {
                        setup.screen = match result {
                            Ok(()) if setup.uninstall => Screen::Removed,
                            Ok(()) => Screen::Done,
                            Err(error) => Screen::Failed(error.into()),
                        };
                        if matches!(setup.screen, Screen::Done) && setup.open_when_done {
                            system::launch();
                        }
                        cx.notify();
                        return false;
                    }
                    cx.notify();
                    true
                });
                if !matches!(alive, Ok(true)) {
                    break;
                }
            }
        })
        .detach();
    }
}

fn describe(step: &Step, uninstall: bool) -> (SharedString, f32) {
    match step {
        Step::Stopping => ("Closing Velo if it is running…".into(), 0.04),
        Step::Copying(done) => ("Copying Velo…".into(), 0.08 + 0.80 * done),
        Step::Shortcuts => ("Adding Velo to the Start menu…".into(), 0.92),
        Step::Registering => ("Finishing up…".into(), 0.97),
        Step::Removing if uninstall => ("Removing Velo…".into(), 0.6),
        Step::Removing => ("Tidying up…".into(), 0.6),
    }
}

impl Render for Setup {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let body = match &self.screen {
            Screen::Welcome => self.welcome(cx).into_any_element(),
            Screen::Working { line, fraction } => div()
                .flex()
                .flex_col()
                .gap(px(14.0))
                .child(heading(if self.uninstall {
                    "Removing Velo"
                } else if self.existing.is_some() {
                    "Updating Velo"
                } else {
                    "Installing Velo"
                }))
                .child(Progress::new("progress").value(fraction * 100.0))
                .child(div().text_sm().text_color(muted).child(line.clone()))
                .into_any_element(),
            Screen::Done => outcome(
                IconName::CircleCheck,
                "Velo is ready",
                if self.signed_in {
                    "You're still signed in. Your chats are right where you left them."
                } else {
                    "Open it and scan the QR code with WhatsApp on your phone: Settings, then Linked devices. Your chats come with you."
                },
                cx,
            )
            .child(footer(
                Button::new("open")
                    .primary()
                    .large()
                    .label("Open Velo")
                    .on_click(|_, _, cx| {
                        system::launch();
                        cx.quit();
                    }),
                Some(Button::new("close").ghost().large().label("Close").on_click(
                    |_, _, cx| cx.quit(),
                )),
            ))
            .into_any_element(),
            Screen::Failed(error) => outcome(
                IconName::TriangleAlert,
                "That didn't work",
                error.clone(),
                cx,
            )
            .child(footer(
                Button::new("retry")
                    .primary()
                    .large()
                    .label("Try again")
                    .on_click(cx.listener(|setup, _, _, cx| setup.start(cx))),
                Some(Button::new("close").ghost().large().label("Close").on_click(
                    |_, _, cx| cx.quit(),
                )),
            ))
            .into_any_element(),
            Screen::ConfirmRemove => self.confirm_remove(cx).into_any_element(),
            Screen::Removed => outcome(
                IconName::CircleCheck,
                "Velo has been removed",
                if self.remove_data {
                    "Its data on this PC is gone too. Your chats are still on your phone."
                } else {
                    "Your sign-in and chats on this PC were kept, in case you come back."
                },
                cx,
            )
            .child(footer(
                Button::new("close").primary().large().label("Close").on_click(|_, _, cx| cx.quit()),
                None,
            ))
            .into_any_element(),
        };

        let key = SharedString::from(format!("screen-{}", self.screen.key()));
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(TitleBar::new().bg(cx.theme().background).border_b_0())
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .px(px(40.0))
                    .pb(px(32.0))
                    .gap(px(22.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(16.0))
                            .child(img(self.logo.clone()).size(px(64.0)))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .text_size(px(28.0))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child("Velo"),
                                    )
                                    .child(div().text_sm().text_color(muted).child(format!(
                                        "WhatsApp, at home on Windows · v{}",
                                        system::VERSION
                                    ))),
                            ),
                    )
                    .child(
                        div().flex_1().flex().flex_col().child(
                            div().flex_1().flex().flex_col().child(body).with_animation(
                                key,
                                Animation::new(Duration::from_millis(220))
                                    .with_easing(ease_out_quint()),
                                |el, t| el.opacity(t).mt(px(8.0 * (1.0 - t))),
                            ),
                        ),
                    ),
            )
    }
}

impl Setup {
    fn welcome(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let muted = cx.theme().muted_foreground;
        let card = cx.theme().secondary;
        let updating = self.existing.is_some();
        div()
            .flex_1()
            .flex()
            .flex_col()
            .gap(px(18.0))
            .child(heading(if updating {
                "Update Velo"
            } else {
                "Install Velo"
            }))
            .child(
                div().text_sm().text_color(muted).child(if updating {
                    "Your chats and sign-in stay exactly where they are. Velo restarts on the new version."
                } else {
                    "A fast, native WhatsApp client for Windows. Installs for you only — no administrator needed."
                }),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(14.0))
                    .p(px(18.0))
                    .rounded(px(14.0))
                    .bg(card)
                    .child(
                        Checkbox::new("desktop")
                            .label("Add a desktop shortcut")
                            .checked(self.desktop_shortcut)
                            .on_click(cx.listener(|setup, checked: &bool, _, cx| {
                                setup.desktop_shortcut = *checked;
                                cx.notify();
                            })),
                    )
                    .child(
                        Checkbox::new("signin")
                            .label("Start Velo when I sign in, so messages and calls reach me")
                            .checked(self.start_at_sign_in)
                            .on_click(cx.listener(|setup, checked: &bool, _, cx| {
                                setup.start_at_sign_in = *checked;
                                cx.notify();
                            })),
                    )
                    .child(
                        Checkbox::new("open")
                            .label("Open Velo when setup finishes")
                            .checked(self.open_when_done)
                            .on_click(cx.listener(|setup, checked: &bool, _, cx| {
                                setup.open_when_done = *checked;
                                cx.notify();
                            })),
                    ),
            )
            .child(div().flex_1())
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(16.0))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_xs()
                            .text_color(muted)
                            .truncate()
                            .child(system::install_dir().display().to_string()),
                    )
                    .child(
                        Button::new("install")
                            .primary()
                            .large()
                            .label(if updating { "Update" } else { "Install" })
                            .on_click(cx.listener(|setup, _, _, cx| setup.start(cx))),
                    ),
            )
    }

    fn confirm_remove(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let muted = cx.theme().muted_foreground;
        let card = cx.theme().secondary;
        div()
            .flex_1()
            .flex()
            .flex_col()
            .gap(px(18.0))
            .child(heading("Remove Velo?"))
            .child(div().text_sm().text_color(muted).child(
                "Velo will be closed and taken off this PC. Your WhatsApp account and chats stay on your phone.",
            ))
            .child(
                div().p(px(18.0)).rounded(px(14.0)).bg(card).child(
                    Checkbox::new("data")
                        .label("Also delete Velo's data on this PC — the chat cache, media and sign-in")
                        .checked(self.remove_data)
                        .on_click(cx.listener(|setup, checked: &bool, _, cx| {
                            setup.remove_data = *checked;
                            cx.notify();
                        })),
                ),
            )
            .child(div().flex_1())
            .child(footer(
                Button::new("remove")
                    .danger()
                    .large()
                    .label("Remove")
                    .on_click(cx.listener(|setup, _, _, cx| setup.start(cx))),
                Some(Button::new("keep").ghost().large().label("Keep Velo").on_click(
                    |_, _, cx| cx.quit(),
                )),
            ))
    }
}

fn heading(text: &'static str) -> impl IntoElement {
    div()
        .text_size(px(20.0))
        .font_weight(FontWeight::SEMIBOLD)
        .child(text)
}

fn outcome(
    icon: IconName,
    title: &'static str,
    detail: impl Into<SharedString>,
    cx: &App,
) -> gpui::Div {
    let tint = if matches!(icon, IconName::TriangleAlert) {
        cx.theme().danger
    } else {
        cx.theme().primary
    };
    div()
        .flex_1()
        .flex()
        .flex_col()
        .gap(px(14.0))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(12.0))
                .child(Icon::new(icon).size(px(28.0)).text_color(tint))
                .child(heading(title)),
        )
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(detail.into()),
        )
        .child(div().flex_1())
}

fn footer(primary: Button, secondary: Option<Button>) -> impl IntoElement {
    div()
        .flex()
        .justify_end()
        .gap(px(10.0))
        .children(secondary)
        .child(primary)
}
