//! Audio and camera selection, tests, and the pre-call preview.
use super::panes::{chosen, group, label};
use crate::app::WhatsAppApp;
use crate::theme::Metrics;
use gpui::{
    App, Entity, IntoElement, ParentElement, Styled, div, img, prelude::FluentBuilder as _,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::{ActiveTheme as _, Disableable as _, Sizable as _};
use velo_core::{CallDevice, CallDevicePreferences, CameraMode, Ringtone};

#[derive(Clone, Copy)]
enum Kind {
    Microphone,
    Speaker,
    RingtoneOutput,
    Camera,
}
impl Kind {
    fn value(self, p: &CallDevicePreferences) -> &Option<String> {
        match self {
            Self::Microphone => &p.microphone,
            Self::Speaker => &p.speaker,
            Self::RingtoneOutput => &p.ringtone_output,
            Self::Camera => &p.camera,
        }
    }
    fn assign(self, p: &mut CallDevicePreferences, id: Option<String>) {
        match self {
            Self::Microphone => p.microphone = id,
            Self::Speaker => p.speaker = id,
            Self::RingtoneOutput => p.ringtone_output = id,
            Self::Camera => p.camera = id,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn choices(
    title: &'static str,
    kind: Kind,
    devices: &[CallDevice],
    preferences: &CallDevicePreferences,
    entity: Entity<WhatsAppApp>,
    busy: bool,
    m: Metrics,
    cx: &App,
) -> impl IntoElement + use<> {
    let mut options = vec![(None, crate::tr!(SystemDefault).to_string())];
    options.extend(devices.iter().map(|d| {
        (
            Some(d.id.clone()),
            if d.is_default {
                crate::tr!(system_default_device, d.label)
            } else {
                d.label.clone()
            },
        )
    }));
    let selected = kind.value(preferences).clone();
    if let Some(id) = &selected
        && !devices.iter().any(|d| &d.id == id)
    {
        options.push((
            Some(id.clone()),
            crate::tr!(SavedDeviceUnavailableRefreshOrChooseAnotherDevice).into(),
        ));
    }
    group(
        label(title, m, cx),
        // A list in one card, left-aligned, with a radio mark: what a choice
        // of one-of-several looks like in a native settings page. Buttons,
        // still, so every device stays reachable from the keyboard.
        div()
            .flex()
            .flex_col()
            .rounded(m.radius_lg())
            .overflow_hidden()
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().secondary)
            .children(options.into_iter().enumerate().map(|(index, (id, text))| {
                let entity = entity.clone();
                let chosen = selected == id;
                let mut preferences = preferences.clone();
                kind.assign(&mut preferences, id);
                let ring = if chosen {
                    cx.theme().primary
                } else {
                    cx.theme().muted_foreground
                };
                Button::new(gpui::SharedString::from(format!("device-{title}-{index}")))
                    .ghost()
                    .w_full()
                    .h_auto()
                    .px(m.space_lg())
                    .py(m.space_md())
                    .rounded_none()
                    .disabled(busy)
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .items_center()
                            .justify_start()
                            .gap(m.space_lg())
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .size(m.icon_small())
                                    .rounded_full()
                                    .border_2()
                                    .border_color(ring)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .when(chosen, |el| {
                                        el.child(div().size(m.space_sm()).rounded_full().bg(ring))
                                    }),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(m.text_secondary())
                                    .text_color(cx.theme().foreground)
                                    .when(chosen, |el| el.font_weight(gpui::FontWeight::MEDIUM))
                                    .child(text),
                            ),
                    )
                    .on_click(move |_, _, cx| {
                        let p = preferences.clone();
                        entity.update(cx, |app, cx| app.save_call_devices(p, cx));
                    })
            })),
        m,
    )
}

/// What the camera settings offer instead of every mode a camera lists: a
/// webcam such as the C920 reports dozens, most of which a call cannot use or
/// nobody would choose between.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Preset {
    /// Small and light: the least bandwidth and CPU, for a weak connection.
    Saver,
    /// VGA, the default, and what every webcam can do.
    Standard,
    /// 720p, the most a WhatsApp call carries.
    Hd,
}

impl Preset {
    const ALL: [Self; 3] = [Self::Saver, Self::Standard, Self::Hd];

    fn id(self) -> &'static str {
        match self {
            Self::Saver => "saver",
            Self::Standard => "standard",
            Self::Hd => "hd",
        }
    }

    fn label(self) -> gpui::SharedString {
        match self {
            Self::Saver => crate::tr!(DataSaver),
            Self::Standard => crate::tr!(StandardQuality),
            Self::Hd => crate::tr!(HdQuality),
        }
        .into()
    }

    /// The mode this preset means for a camera offering `modes`, or `None`
    /// when it offers nothing in that range: the height nearest the ideal,
    /// then the frame rate nearest it, then the widest picture.
    fn pick(self, modes: &[CameraMode]) -> Option<CameraMode> {
        let (heights, ideal_height, ideal_fps) = match self {
            Self::Saver => (180..=400, 360, 15),
            Self::Standard => (420..=600, 480, 30),
            Self::Hd => (601..=1080, 720, 30),
        };
        modes
            .iter()
            .copied()
            .filter(|mode| heights.contains(&mode.height))
            .min_by_key(|mode| {
                (
                    mode.height.abs_diff(ideal_height),
                    mode.fps.abs_diff(ideal_fps),
                    std::cmp::Reverse(mode.width),
                )
            })
    }
}

fn supported_mode(mode: CameraMode) -> bool {
    velo_video::VideoQuality {
        width: mode.width,
        height: mode.height,
        fps: mode.fps,
        ..Default::default()
    }
    .checked()
    .is_ok()
}

pub fn render(
    app: &WhatsAppApp,
    entity: Entity<WhatsAppApp>,
    m: Metrics,
    cx: &App,
) -> gpui::AnyElement {
    let preferences = app.call_device_preferences(cx);
    let inventory = app.call_device_inventory(cx).cloned().unwrap_or_default();
    let busy = app.call_device_busy(cx);
    let ready = app.call_device_preferences_ready(cx);
    let edits_disabled = busy || !ready;
    let refresh = entity.clone();
    let mic_test = entity.clone();
    let speaker_test = entity.clone();
    let ringtone_test = entity.clone();
    let preview = entity.clone();
    let confirm = entity.clone();
    let cancel = entity.clone();
    let active = app.camera_preview_active(cx);
    let camera = preferences
        .camera
        .as_ref()
        .and_then(|id| inventory.cameras.iter().find(|d| &d.id == id))
        .or_else(|| inventory.cameras.iter().find(|d| d.is_default));
    let mut modes = camera.map(|d| d.modes.clone()).unwrap_or_default();
    modes.retain(|mode| supported_mode(*mode));
    modes.sort_by_key(|m| (m.width, m.height, m.fps));
    modes.dedup();
    if modes.is_empty() {
        modes.push(CameraMode::default());
    }
    // A mode saved before the presets, or picked for a camera since
    // replaced, is shown as what it is rather than silently re-chosen.
    let custom = !Preset::ALL
        .iter()
        .any(|preset| preset.pick(&modes) == Some(preferences.camera_mode));
    let mut content = div().flex().flex_col().gap(m.space_xl())
        .child(div().text_size(m.text_small()).text_color(cx.theme().muted_foreground).child(crate::tr!(PreferencesApplyToYourNextCallTestEachDeviceBeforeCallingDeviceTestsAccessOnlyTheDeviceYou)))
        .child(div().flex().child(Button::new("refresh-call-devices").label(crate::tr!(RefreshDevices)).outline().disabled(busy || active).on_click(move |_, _, cx| refresh.update(cx, |app, cx| app.refresh_call_devices(cx)))))
        .child(choices(crate::tr!(Microphone), Kind::Microphone, &inventory.microphones, &preferences, entity.clone(), edits_disabled, m, cx))
        .child(div().flex().child(Button::new("test-call-microphone").label(crate::tr!(TestMicrophone)).outline().disabled(busy).on_click(move |_, _, cx| mic_test.update(cx, |app, cx| app.test_call_microphone(cx)))))
        .child(choices(crate::tr!(Speaker), Kind::Speaker, &inventory.speakers, &preferences, entity.clone(), edits_disabled, m, cx))
        .child(div().flex().child(Button::new("test-call-speaker").label(crate::tr!(TestSpeaker)).outline().disabled(busy).on_click(move |_, _, cx| speaker_test.update(cx, |app, cx| app.test_call_speaker(cx)))))
        .child(choices(crate::tr!(RingtoneOutput), Kind::RingtoneOutput, &inventory.speakers, &preferences, entity.clone(), edits_disabled, m, cx))
        .child(group(label(crate::tr!(Ringtone), m, cx), div().flex().flex_wrap().gap(m.space_sm()).children([(Ringtone::Signature, crate::tr!(VeloSignature)), (Ringtone::GlassChime, crate::tr!(GlassChime)), (Ringtone::Classic, crate::tr!(Classic)), (Ringtone::Gentle, crate::tr!(Gentle)), (Ringtone::Silent, crate::tr!(Silent))].into_iter().map(|(tone, label)| {
            let entity = entity.clone(); let mut p = preferences.clone(); p.ringtone = tone;
            Button::new(gpui::SharedString::from(format!("ringtone-{label}"))).label(label).small().map(|b| chosen(b, preferences.ringtone == tone)).disabled(edits_disabled).on_click(move |_, _, cx| { let p = p.clone(); entity.update(cx, |app, cx| app.save_call_devices(p, cx)); })
        })), m))
        .child(div().flex().child(Button::new("test-call-ringtone").label(crate::tr!(TestRingtone)).outline().disabled(busy).on_click(move |_, _, cx| ringtone_test.update(cx, |app, cx| app.test_call_ringtone(cx)))))
        .child(choices(crate::tr!(Camera), Kind::Camera, &inventory.cameras, &preferences, entity.clone(), edits_disabled, m, cx))
        .child(group(label(crate::tr!(VideoQuality), m, cx), div().flex().flex_col().gap(m.space_sm())
            .child(div().flex().flex_wrap().gap(m.space_sm()).children(Preset::ALL.into_iter().map(|preset| {
                let picked = preset.pick(&modes);
                let entity = entity.clone();
                let mut p = preferences.clone();
                if let Some(mode) = picked { p.camera_mode = mode; }
                Button::new(gpui::SharedString::from(format!("camera-preset-{}", preset.id())))
                    .label(preset.label())
                    .small()
                    .map(|b| chosen(b, picked.is_some() && picked == Some(preferences.camera_mode)))
                    .disabled(edits_disabled || picked.is_none())
                    .on_click(move |_, _, cx| { let p = p.clone(); entity.update(cx, |app, cx| app.save_call_devices(p, cx)); })
            })).when(custom, |row| row.child(
                Button::new("camera-preset-custom")
                    .label(crate::tr!(custom_camera, preferences.camera_mode.width, preferences.camera_mode.height, preferences.camera_mode.fps))
                    .small()
                    .map(|b| chosen(b, true))
                    .disabled(true),
            )))
            .child(div().text_size(m.text_small()).text_color(cx.theme().muted_foreground).child(crate::tr!(VideoQualityNote))), m))
        .child(div().text_size(m.text_small()).child(crate::tr!(requested_camera, preferences.camera_mode.width, preferences.camera_mode.height, preferences.camera_mode.fps)))
        .child(div().flex().child(Button::new("camera-preview").label(if active { crate::tr!(StopCameraPreview) } else { crate::tr!(PreviewCamera) }).outline().disabled((busy || !ready) && !active).on_click(move |_, _, cx| preview.update(cx, |app, cx| if active { app.cancel_camera_preview(cx); } else { app.start_camera_preview(cx); }))));
    if let Some(image) = app.camera_preview_image(cx) {
        content = content.child(
            img(crate::video::video_image(image))
                .w_full()
                .max_w(m.reading_width())
                .rounded(m.radius_md()),
        );
    }
    if let Some(mode) = app.camera_preview_mode(cx) {
        content = content.child(div().text_size(m.text_small()).child(crate::tr!(
            opened_camera,
            mode.width,
            mode.height,
            mode.fps
        )));
    }
    content = content.child(
        div()
            .text_size(m.text_small())
            .text_color(cx.theme().muted_foreground)
            .child(app.call_device_status(cx)),
    );
    if app.preparing_call(cx) {
        content = content.child(
            div()
                .flex()
                .gap(m.space_md())
                .child(
                    Button::new("confirm-device-call")
                        .label(crate::tr!(StartCall))
                        .primary()
                        .disabled(busy || !ready)
                        .on_click(move |_, _, cx| {
                            confirm.update(cx, |app, cx| app.confirm_prepared_call(cx))
                        }),
                )
                .child(
                    Button::new("cancel-device-call")
                        .label(crate::tr!(Cancel))
                        .outline()
                        .on_click(move |_, _, cx| {
                            cancel.update(cx, |app, cx| {
                                app.close_settings(cx);
                            });
                        }),
                ),
        );
    }
    content.into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mode(width: u32, height: u32, fps: u32) -> CameraMode {
        CameraMode { width, height, fps }
    }

    /// What a C920 or C922 offers once the call path's limits are applied:
    /// MJPEG up to 720p at 30, and the smaller modes at 30 and lower.
    #[test]
    fn presets_find_the_modes_a_c920_offers() {
        let offered = [
            mode(320, 240, 30),
            mode(640, 360, 15),
            mode(640, 360, 30),
            mode(640, 480, 15),
            mode(640, 480, 30),
            mode(800, 600, 30),
            mode(960, 540, 30),
            mode(1280, 720, 15),
            mode(1280, 720, 30),
        ];
        assert_eq!(Preset::Saver.pick(&offered), Some(mode(640, 360, 15)));
        assert_eq!(Preset::Standard.pick(&offered), Some(mode(640, 480, 30)));
        assert_eq!(Preset::Hd.pick(&offered), Some(mode(1280, 720, 30)));
    }

    #[test]
    fn a_preset_the_camera_cannot_meet_is_offered_as_unavailable() {
        let vga_only = [mode(640, 480, 30)];
        assert_eq!(Preset::Hd.pick(&vga_only), None);
        assert_eq!(Preset::Saver.pick(&vga_only), None);
        assert_eq!(Preset::Standard.pick(&vga_only), Some(mode(640, 480, 30)));
    }
    #[test]
    fn choices_follow_the_capture_codecs_mode_limits() {
        assert!(!supported_mode(CameraMode {
            width: 1280,
            height: 720,
            fps: 60
        }));
        assert!(supported_mode(CameraMode {
            width: 1280,
            height: 720,
            fps: 30
        }));
        assert!(supported_mode(CameraMode {
            width: 640,
            height: 480,
            fps: 60
        }));
        assert!(!supported_mode(CameraMode {
            width: 640,
            height: 480,
            fps: 29
        }));
    }
}
