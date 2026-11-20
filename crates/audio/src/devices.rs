//! Inventory never opens a stream. Ringtone owns only an output stream.
use anyhow::Result;
use velo_core::{CallDevice, Ringtone};

pub use imp::{
    RingtonePlayback, enumerate_audio_devices, open_test_microphone, start_ringtone,
    start_ringtone_cancellable,
};

static CALL_PREFERENCES: std::sync::RwLock<(velo_core::CallDevicePreferences, bool)> =
    std::sync::RwLock::new((
        velo_core::CallDevicePreferences {
            microphone: None,
            speaker: None,
            ringtone_output: None,
            camera: None,
            camera_mode: velo_core::CameraMode {
                width: 640,
                height: 480,
                fps: 30,
            },
            ringtone: Ringtone::Classic,
        },
        false,
    ));

pub fn set_call_preferences(preferences: velo_core::CallDevicePreferences) {
    *CALL_PREFERENCES.write().unwrap_or_else(|e| e.into_inner()) = (preferences, true);
}

/// Install launch defaults without replacing legacy input routing before the first save.
pub fn set_call_preferences_from_defaults(preferences: velo_core::CallDevicePreferences) {
    *CALL_PREFERENCES.write().unwrap_or_else(|e| e.into_inner()) = (preferences, false);
}

pub fn call_preferences() -> velo_core::CallDevicePreferences {
    call_preferences_with_origin().0
}

/// One consistent snapshot for consumers that distinguish launch defaults from saved choices.
pub fn call_preferences_with_origin() -> (velo_core::CallDevicePreferences, bool) {
    CALL_PREFERENCES
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

/// A deterministic, quiet ring: two soft bell notes in the first second of
/// every three, then silence.
///
/// Each note has a short attack, a natural exponential decay and a faded
/// tail, so it never stops mid-wave. The earlier tone was one bare sine that
/// was cut off at the end of its second — a click on every ring that read
/// as distortion through laptop speakers.
pub fn ringtone_sample(tone: Ringtone, frame: u64, rate: u32) -> i16 {
    if tone == Ringtone::Silent || rate == 0 || frame % (u64::from(rate) * 3) >= u64::from(rate) {
        return 0;
    }
    // (start, length) in seconds and the note's pitch, per tone.
    let notes: [(f64, f64, f64); 2] = match tone {
        Ringtone::Gentle => [(0.0, 0.46, 523.25), (0.48, 0.5, 659.25)], // C5, E5
        _ => [(0.0, 0.46, 783.99), (0.48, 0.5, 1046.5)],                // G5, C6
    };
    let t = (frame % u64::from(rate)) as f64 / f64::from(rate);
    let mut sample = 0.0;
    for (start, length, hz) in notes {
        let age = t - start;
        if !(0.0..length).contains(&age) {
            continue;
        }
        let attack = (age / 0.008).min(1.0);
        let decay = (-age / 0.16).exp();
        // A raised-cosine fade over the note's last 60 ms: zero at the end.
        let fade_at = length - 0.06;
        let release = if age > fade_at {
            0.5 * (1.0 + (std::f64::consts::PI * (age - fade_at) / 0.06).cos())
        } else {
            1.0
        };
        let phase = age * hz * std::f64::consts::TAU;
        // A little of the octave above, fading faster: a bell, not a beep.
        let voice = phase.sin() + 0.18 * (-age / 0.05).exp() * (2.0 * phase).sin();
        sample += voice * attack * decay * release;
    }
    // Peak stays inside the 2000 the call path was tuned for.
    (sample / 1.18 * 2000.0).clamp(-2000.0, 2000.0) as i16
}

#[cfg(not(target_family = "wasm"))]
mod imp {
    use super::*;
    use anyhow::anyhow;
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use portable_atomic::AtomicU64;
    use ringbuf::traits::{Producer, Split};
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    use std::time::Duration;

    #[derive(Debug, PartialEq, Eq)]
    pub(crate) enum MicrophoneChoice {
        SystemDefault,
        Device(String),
        LegacyName(String),
    }
    fn choose_microphone(
        device: Option<String>,
        saved: bool,
        legacy: Option<String>,
    ) -> MicrophoneChoice {
        if let Some(id) = device {
            return MicrophoneChoice::Device(id);
        }
        if !saved && let Some(name) = legacy.filter(|s| !s.is_empty()) {
            return MicrophoneChoice::LegacyName(name);
        }
        MicrophoneChoice::SystemDefault
    }
    pub(crate) fn microphone_choice(legacy: Option<String>) -> MicrophoneChoice {
        let preferences = super::CALL_PREFERENCES
            .read()
            .unwrap_or_else(|e| e.into_inner());
        choose_microphone(preferences.0.microphone.clone(), preferences.1, legacy)
    }

    /// Dropping this guard cancels startup as well as playback.
    pub struct RingtonePlayback(Arc<AtomicBool>);
    impl Drop for RingtonePlayback {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Release);
        }
    }

    pub async fn open_test_microphone() -> Result<async_channel::Receiver<Vec<i16>>> {
        tokio::task::spawn_blocking(crate::call_device::spawn_mic)
            .await
            .map_err(|e| anyhow!("microphone setup failed: {e}"))?
    }

    pub fn enumerate_audio_devices() -> Result<(Vec<CallDevice>, Vec<CallDevice>)> {
        let host = cpal::default_host();
        let input_default = host.default_input_device().and_then(|d| d.id().ok());
        let output_default = host.default_output_device().and_then(|d| d.id().ok());
        fn entries(
            devices: impl Iterator<Item = cpal::Device>,
            default: Option<cpal::DeviceId>,
        ) -> Vec<CallDevice> {
            devices
                .filter_map(|d| {
                    let id = d.id().ok()?;
                    Some(CallDevice {
                        is_default: default.as_ref() == Some(&id),
                        id: id.to_string(),
                        label: d.description().ok()?.name().to_owned(),
                        modes: Vec::new(),
                    })
                })
                .collect()
        }
        Ok((
            entries(host.input_devices()?, input_default),
            entries(host.output_devices()?, output_default),
        ))
    }

    pub(crate) fn select(id: Option<&str>, input: bool) -> Result<cpal::Device> {
        let host = cpal::default_host();
        match id {
            Some(id) => {
                let id = id
                    .parse::<cpal::DeviceId>()
                    .map_err(|_| anyhow!("invalid audio device id"))?;
                host.device_by_id(&id).ok_or_else(|| anyhow!("selected audio device is unavailable; refresh devices or choose System default"))
            }
            None => if input {
                host.default_input_device()
            } else {
                host.default_output_device()
            }
            .ok_or_else(|| anyhow!("no system default audio device")),
        }
    }

    pub fn start_ringtone(output: Option<String>, tone: Ringtone) -> Result<RingtonePlayback> {
        start_ringtone_cancellable(output, tone, Arc::new(AtomicBool::new(false)))
    }

    pub fn start_ringtone_cancellable(
        output: Option<String>,
        tone: Ringtone,
        stop: Arc<AtomicBool>,
    ) -> Result<RingtonePlayback> {
        if tone == Ringtone::Silent {
            return Ok(RingtonePlayback(stop));
        }
        start_worker(
            stop,
            Duration::from_secs(5),
            move |stop| {
                let device = select(output.as_deref(), false)?;
                let (config, format) = crate::call_device::default_config(&device, false)?;
                let (producer, consumer) =
                    ringbuf::HeapRb::<i16>::new(config.sample_rate as usize).split();
                let on_error = {
                    let stop = stop.clone();
                    Arc::new(move || stop.store(true, Ordering::Release))
                };
                let stream = crate::call_device::build_output_stream(
                    &device,
                    &config,
                    format,
                    config.channels as usize,
                    consumer,
                    crate::call_device::SpeakerCounters {
                        pops: Arc::new(AtomicU64::new(0)),
                        underruns: Arc::new(AtomicU64::new(0)),
                    },
                    on_error,
                )?;
                if stop.load(Ordering::Acquire) {
                    anyhow::bail!("ringtone cancelled");
                }
                stream.play()?;
                Ok((stream, config.sample_rate, producer))
            },
            move |(_stream, rate, mut producer), stop| {
                let mut frame = 0;
                while !stop.load(Ordering::Acquire) {
                    for _ in 0..(rate / 100) {
                        if producer
                            .try_push(ringtone_sample(tone, frame, rate))
                            .is_err()
                        {
                            break;
                        }
                        frame += 1;
                    }
                    std::thread::park_timeout(Duration::from_millis(5));
                }
            },
        )
    }

    fn start_worker<T: 'static>(
        stop: Arc<AtomicBool>,
        timeout: Duration,
        open: impl FnOnce(Arc<AtomicBool>) -> Result<T> + Send + 'static,
        run: impl FnOnce(T, Arc<AtomicBool>) + Send + 'static,
    ) -> Result<RingtonePlayback> {
        let guard = RingtonePlayback(stop.clone());
        if stop.load(Ordering::Acquire) {
            anyhow::bail!("ringtone cancelled");
        }
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || match open(stop.clone()) {
            Err(error) => {
                let _ = ready_tx.send(Err(error));
            }
            Ok(output) => {
                if stop.load(Ordering::Acquire) {
                    let _ = ready_tx.send(Err(anyhow!("ringtone cancelled")));
                    return;
                }
                if ready_tx.send(Ok(())).is_err() {
                    return;
                }
                run(output, stop);
            }
        });
        ready_rx
            .recv_timeout(timeout)
            .map_err(|_| anyhow!("ringtone output startup timed out"))??;
        Ok(guard)
    }

    #[cfg(test)]
    mod lifecycle_tests {
        use super::*;
        use std::sync::mpsc;
        struct FakeOutput(mpsc::Sender<()>);
        impl Drop for FakeOutput {
            fn drop(&mut self) {
                let _ = self.0.send(());
            }
        }

        #[test]
        fn saved_system_default_overrides_the_legacy_environment_selection() {
            assert_eq!(
                choose_microphone(None, true, Some("legacy-input".into())),
                MicrophoneChoice::SystemDefault
            );
        }
        #[test]
        fn missing_settings_preserve_the_legacy_environment_selection() {
            assert_eq!(
                choose_microphone(None, false, Some("legacy-input".into())),
                MicrophoneChoice::LegacyName("legacy-input".into())
            );
            assert_eq!(
                choose_microphone(None, false, None),
                MicrophoneChoice::SystemDefault
            );
        }
        #[test]
        fn an_explicit_endpoint_wins_over_legacy_routing() {
            assert_eq!(
                choose_microphone(Some("fixture-id".into()), true, Some("legacy-input".into())),
                MicrophoneChoice::Device("fixture-id".into())
            );
        }
        #[test]
        fn cancellation_while_opening_drops_late_output() {
            let stop = Arc::new(AtomicBool::new(false));
            let token = stop.clone();
            let (entered_tx, entered_rx) = mpsc::channel();
            let (release_tx, release_rx) = mpsc::channel();
            let (dropped_tx, dropped_rx) = mpsc::channel();
            let worker = std::thread::spawn(move || {
                start_worker(
                    token,
                    Duration::from_secs(5),
                    move |_| {
                        entered_tx.send(()).unwrap();
                        release_rx.recv().unwrap();
                        Ok(FakeOutput(dropped_tx))
                    },
                    |_, _| panic!("cancelled output must never run"),
                )
            });
            entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            stop.store(true, Ordering::Release);
            release_tx.send(()).unwrap();
            assert!(worker.join().unwrap().is_err());
            dropped_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        }

        #[test]
        fn startup_timeout_disowns_the_output_that_finishes_later() {
            let stop = Arc::new(AtomicBool::new(false));
            let token = stop.clone();
            let (release_tx, release_rx) = mpsc::channel();
            let (dropped_tx, dropped_rx) = mpsc::channel();
            let result = start_worker(
                token,
                Duration::from_millis(10),
                move |_| {
                    release_rx.recv().unwrap();
                    Ok(FakeOutput(dropped_tx))
                },
                |_, _| panic!("timed out output must never run"),
            );
            assert!(result.is_err());
            assert!(stop.load(Ordering::Acquire));
            release_tx.send(()).unwrap();
            dropped_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        }

        #[test]
        fn dropping_successful_playback_releases_its_output() {
            let stop = Arc::new(AtomicBool::new(false));
            let (dropped_tx, dropped_rx) = mpsc::channel();
            let playback = start_worker(
                stop.clone(),
                Duration::from_secs(5),
                move |_| Ok(FakeOutput(dropped_tx)),
                |output, stop| {
                    let _output = output;
                    while !stop.load(Ordering::Acquire) {
                        std::thread::park_timeout(Duration::from_millis(1));
                    }
                },
            )
            .unwrap();
            drop(playback);
            assert!(stop.load(Ordering::Acquire));
            dropped_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        }

        #[test]
        fn output_failure_preserves_the_original_cause() {
            let error = start_worker::<()>(
                Arc::new(AtomicBool::new(false)),
                Duration::from_secs(5),
                |_| Err(anyhow!("selected audio device is unavailable")),
                |_, _| {},
            )
            .err()
            .unwrap();
            assert!(
                error
                    .to_string()
                    .contains("selected audio device is unavailable")
            );
        }
    }
}

#[cfg(target_family = "wasm")]
mod imp {
    use super::*;
    pub struct RingtonePlayback;
    pub async fn open_test_microphone() -> Result<async_channel::Receiver<Vec<i16>>> {
        anyhow::bail!("Microphone testing is unavailable from this browser connection")
    }

    pub fn enumerate_audio_devices() -> Result<(Vec<CallDevice>, Vec<CallDevice>)> {
        anyhow::bail!("Device inventory is unavailable from this browser connection")
    }
    pub fn start_ringtone_cancellable(
        output: Option<String>,
        tone: Ringtone,
        stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<RingtonePlayback> {
        if stop.load(std::sync::atomic::Ordering::Acquire) {
            anyhow::bail!("ringtone cancelled");
        }
        start_ringtone(output, tone)
    }
    pub fn start_ringtone(_output: Option<String>, tone: Ringtone) -> Result<RingtonePlayback> {
        if tone == Ringtone::Silent {
            Ok(RingtonePlayback)
        } else {
            anyhow::bail!("Ringtone playback is unavailable from this browser connection")
        }
    }
}

#[cfg(not(target_family = "wasm"))]
pub(crate) use imp::{MicrophoneChoice, microphone_choice, select};

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tones_are_bounded_and_have_silent_gaps() {
        for tone in [Ringtone::Classic, Ringtone::Gentle, Ringtone::Silent] {
            assert!((0..16000).all(|i| ringtone_sample(tone, i, 16000).unsigned_abs() <= 2000));
            assert!((16000..48000).all(|i| ringtone_sample(tone, i, 16000) == 0));
        }
        assert!((0..16000).all(|i| ringtone_sample(Ringtone::Silent, i, 16000) == 0));
    }

    /// The click that made the ring sound broken: no sample-to-sample jump
    /// larger than a sine at the note's own pitch could make, and the ring
    /// has reached silence by the time the silent gap begins.
    #[test]
    fn the_ring_has_no_clicks() {
        let rate = 48_000;
        for tone in [Ringtone::Classic, Ringtone::Gentle] {
            let samples: Vec<i32> = (0..rate as u64)
                .map(|i| i32::from(ringtone_sample(tone, i, rate)))
                .collect();
            // Max slope of 2000·sin(2π·1046.5·t) per sample at 48 kHz ≈ 274,
            // plus headroom for the harmonic and the attack.
            let worst = samples
                .windows(2)
                .map(|w| (w[1] - w[0]).abs())
                .max()
                .unwrap();
            assert!(worst < 600, "{tone:?}: a {worst}-step jump is a click");
            assert!(
                samples[samples.len() - 1].abs() < 20,
                "{tone:?} ends at silence"
            );
            assert!(
                samples.iter().any(|s| s.abs() > 1000),
                "{tone:?} is audible"
            );
        }
    }
}

#[cfg(all(test, not(target_family = "wasm")))]
mod cancellation_tests {
    use super::*;
    #[test]
    fn cancelled_startup_never_looks_up_an_output() {
        let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        assert!(
            start_ringtone_cancellable(Some("invalid-id".into()), Ringtone::Classic, cancelled)
                .is_err()
        );
    }
    #[test]
    fn silent_ringtone_does_not_need_an_audio_device() {
        assert!(start_ringtone(Some("invalid-id".into()), Ringtone::Silent).is_ok());
    }
}
