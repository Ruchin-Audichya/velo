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

/// How long one ring lasts, phrase and pause together, in seconds.
pub const RING_CYCLE: f64 = 2.6;
/// Every note has died away by here; the rest of the cycle is silence.
pub const RING_PHRASE: f64 = 1.9;
/// The loudest a ring gets: about −14 dBFS, loud enough to hear across a
/// room on laptop speakers and still well clear of clipping.
pub const RING_PEAK: i16 = 6500;

/// One note of a phrase: when it is struck, its pitch, and how hard.
struct Note {
    at: f64,
    hz: f64,
    gain: f64,
}

/// What a tone is: its partials as (ratio to the fundamental, level, decay
/// time constant in seconds), its mallet, and its phrase.
struct Voice {
    partials: &'static [(f64, f64, f64)],
    /// The strike's attack, in seconds: shorter is a harder mallet.
    attack: f64,
    notes: &'static [Note],
}

const C5: f64 = 523.25;
const E5: f64 = 659.26;
const G5: f64 = 783.99;
const A5: f64 = 880.0;
const B5: f64 = 987.77;
const C6: f64 = 1046.5;
const D6: f64 = 1174.66;
const E6: f64 = 1318.51;

/// Classic: a bright marimba phrase climbing a G major arpeggio and
/// answering itself. A marimba bar's overtones sit near 4× and 10× the
/// fundamental and fade far faster than it, which is the woody "tock".
const MARIMBA: Voice = Voice {
    partials: &[(1.0, 1.0, 0.32), (3.93, 0.32, 0.055), (9.2, 0.08, 0.018)],
    attack: 0.003,
    notes: &[
        Note {
            at: 0.00,
            hz: G5,
            gain: 0.95,
        },
        Note {
            at: 0.13,
            hz: B5,
            gain: 0.85,
        },
        Note {
            at: 0.26,
            hz: D6,
            gain: 0.90,
        },
        Note {
            at: 0.39,
            hz: E6,
            gain: 0.80,
        },
        Note {
            at: 0.65,
            hz: D6,
            gain: 0.85,
        },
        Note {
            at: 0.78,
            hz: C6,
            gain: 0.70,
        },
        Note {
            at: 0.91,
            hz: D6,
            gain: 0.95,
        },
    ],
};

/// Gentle: a kalimba, lower and slower. The tine's next partial sits a
/// little sharp of 3× and soft, so it rings like a plucked bar, not a sine.
const KALIMBA: Voice = Voice {
    partials: &[(1.0, 1.0, 0.55), (3.08, 0.12, 0.09), (5.4, 0.04, 0.03)],
    attack: 0.006,
    notes: &[
        Note {
            at: 0.00,
            hz: C5,
            gain: 0.90,
        },
        Note {
            at: 0.24,
            hz: E5,
            gain: 0.80,
        },
        Note {
            at: 0.48,
            hz: G5,
            gain: 0.85,
        },
        Note {
            at: 0.76,
            hz: A5,
            gain: 0.75,
        },
        Note {
            at: 1.00,
            hz: G5,
            gain: 0.80,
        },
    ],
};

/// The longest any note sounds before its fade has taken it to silence.
const NOTE_LENGTH: f64 = 0.8;
/// The end of each note: a raised cosine to exactly zero, so a note never
/// stops mid-wave. A bare cut was the click the first ringtone had on every
/// ring.
const NOTE_FADE: f64 = 0.25;
/// One early reflection, as from a nearby wall: enough room that the ring
/// does not sound like it comes from inside the speaker.
const ECHO_DELAY: f64 = 0.083;
const ECHO_GAIN: f64 = 0.22;

/// The ring at `frame` of a stream at `rate` Hz.
///
/// Deterministic and stateless, so the stream can start at any frame and
/// every ring sounds the same: a phrase of struck notes in the first
/// [`RING_PHRASE`] seconds of every [`RING_CYCLE`], then silence.
pub fn ringtone_sample(tone: Ringtone, frame: u64, rate: u32) -> i16 {
    let voice = match tone {
        Ringtone::Silent => return 0,
        Ringtone::Gentle => &KALIMBA,
        Ringtone::Classic => &MARIMBA,
    };
    if rate == 0 {
        return 0;
    }
    let t = (frame as f64 / f64::from(rate)) % RING_CYCLE;
    if t >= RING_PHRASE {
        return 0;
    }
    let dry = |t: f64| -> f64 {
        voice
            .notes
            .iter()
            .map(|note| struck(voice, note, t - note.at))
            .sum()
    };
    let wet = dry(t) + ECHO_GAIN * dry(t - ECHO_DELAY);
    // A gentle saturator rather than a hard clip: where notes overlap the
    // peaks round off instead of squaring, and the curve is smooth, so it
    // adds no click of its own.
    let shaped = (wet * 0.9).tanh() / 0.9_f64.tanh();
    let peak = f64::from(RING_PEAK);
    (shaped * peak).clamp(-peak, peak) as i16
}

/// One note, `age` seconds after it was struck.
fn struck(voice: &Voice, note: &Note, age: f64) -> f64 {
    if !(0.0..NOTE_LENGTH).contains(&age) {
        return 0.0;
    }
    // A raised-cosine attack: smooth at both ends of the rise.
    let attack = if age < voice.attack {
        0.5 * (1.0 - (std::f64::consts::PI * age / voice.attack).cos())
    } else {
        1.0
    };
    let fade_at = NOTE_LENGTH - NOTE_FADE;
    let release = if age > fade_at {
        0.5 * (1.0 + (std::f64::consts::PI * (age - fade_at) / NOTE_FADE).cos())
    } else {
        1.0
    };
    let body: f64 = voice
        .partials
        .iter()
        .map(|&(ratio, level, decay)| {
            level * (-age / decay).exp() * (age * note.hz * ratio * std::f64::consts::TAU).sin()
        })
        .sum();
    note.gain * 0.55 * attack * release * body
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
        let rate = 16_000;
        let cycle = (RING_CYCLE * f64::from(rate)) as u64;
        let phrase = (RING_PHRASE * f64::from(rate)) as u64 + 1;
        for tone in [Ringtone::Classic, Ringtone::Gentle, Ringtone::Silent] {
            assert!((0..cycle * 2).all(|i| ringtone_sample(tone, i, rate).abs() <= RING_PEAK));
            assert!(
                (phrase..cycle).all(|i| ringtone_sample(tone, i, rate) == 0),
                "{tone:?} is silent between rings"
            );
        }
        assert!((0..cycle).all(|i| ringtone_sample(Ringtone::Silent, i, rate) == 0));
    }

    /// The click that made the first ring sound broken: no sample-to-sample
    /// jump larger than the notes' own waves could make, every note has
    /// faded out before the pause, and the ring is loud enough to hear.
    #[test]
    fn the_ring_has_no_clicks() {
        let rate = 48_000;
        for tone in [Ringtone::Classic, Ringtone::Gentle] {
            let samples: Vec<i32> = (0..(RING_CYCLE * f64::from(rate)) as u64)
                .map(|i| i32::from(ringtone_sample(tone, i, rate)))
                .collect();
            // About the steepest the partials summed at full scale could
            // move in one sample: past it is a discontinuity, not a note.
            let worst = samples
                .windows(2)
                .map(|w| (w[1] - w[0]).abs())
                .max()
                .unwrap();
            assert!(worst < 3200, "{tone:?}: a {worst}-step jump is a click");
            let phrase_end = (RING_PHRASE * f64::from(rate)) as usize;
            assert!(
                samples[phrase_end - 40..phrase_end]
                    .iter()
                    .all(|s| s.abs() < 20),
                "{tone:?} has died away before the pause"
            );
            let loudest = samples.iter().map(|s| s.abs()).max().unwrap();
            assert!(loudest > 3000, "{tone:?} is audible: peak {loudest}");
        }
    }

    /// Starting anywhere in the stream gives the same ring: the sample is a
    /// function of the frame alone.
    #[test]
    fn every_ring_is_the_same() {
        let rate = 16_000;
        let cycle = (RING_CYCLE * f64::from(rate)) as u64;
        for i in (0..cycle).step_by(97) {
            let first = ringtone_sample(Ringtone::Classic, i, rate);
            let later = ringtone_sample(Ringtone::Classic, i + cycle * 5, rate);
            assert!((first - later).abs() <= 1, "frame {i}: {first} vs {later}");
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
