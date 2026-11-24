//! Motion: the few short animations the interface uses, in one place.
//!
//! Every one is brief (well under a quarter of a second) and eases out, so it
//! reads as the interface settling rather than performing. GPUI's animation
//! element already honours the system's reduce-motion preference — the
//! element simply renders at its end state — so nothing here checks for it.
//!
//! Offsets are paint-only (`top`/`left` on a relatively positioned element),
//! never layout: the virtual message list measures rows, and a row whose
//! height changed while it animated would make the whole timeline jump.

use std::time::Duration;

use gpui::{Animation, AnimationExt as _, ElementId, IntoElement, Pixels, Styled, ease_out_quint};

/// How much the interface moves, chosen in Settings → Appearance.
///
/// Every helper here asks [`duration`] and [`travel`], so one setting scales
/// all of them together. Off keeps the interface whole and working, with
/// each animation finished the moment it starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum MotionLevel {
    Off,
    Minimal,
    #[default]
    Balanced,
    Expressive,
}

impl MotionLevel {
    pub const ALL: [Self; 4] = [Self::Off, Self::Minimal, Self::Balanced, Self::Expressive];

    pub fn id(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Minimal => "minimal",
            Self::Balanced => "balanced",
            Self::Expressive => "expressive",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Minimal => "Minimal",
            Self::Balanced => "Balanced",
            Self::Expressive => "Expressive",
        }
    }

    pub fn parse(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|level| level.id() == id)
    }
}

static LEVEL: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(2);

/// Set the motion level for every animation drawn from now on.
pub fn set_level(level: MotionLevel) {
    LEVEL.store(level as u8, std::sync::atomic::Ordering::Relaxed);
}

/// The motion level in force.
pub fn level() -> MotionLevel {
    match LEVEL.load(std::sync::atomic::Ordering::Relaxed) {
        0 => MotionLevel::Off,
        1 => MotionLevel::Minimal,
        3 => MotionLevel::Expressive,
        _ => MotionLevel::Balanced,
    }
}

/// `base` at the motion level in force: shorter for Minimal, a little longer
/// for Expressive, and a single millisecond (done on its first frame) for Off.
pub fn duration(base: Duration) -> Duration {
    match level() {
        MotionLevel::Off => Duration::from_millis(1),
        MotionLevel::Minimal => base * 3 / 5,
        MotionLevel::Balanced => base,
        MotionLevel::Expressive => base * 27 / 20,
    }
}

/// How far something travels as it animates: nothing for Off and Minimal,
/// which fade only, and half again as far for Expressive.
pub fn travel(distance: Pixels) -> Pixels {
    match level() {
        MotionLevel::Off | MotionLevel::Minimal => Pixels::ZERO,
        MotionLevel::Balanced => distance,
        MotionLevel::Expressive => distance * 1.5,
    }
}

/// A view appearing: a chat being switched to, a panel opening.
pub const ENTER: Duration = Duration::from_millis(180);
/// Something arriving in a list: a new message.
pub const ARRIVE: Duration = Duration::from_millis(220);
/// A small acknowledgement: a badge changing.
pub const POP: Duration = Duration::from_millis(260);
/// One control taking another's place: send for record. Short, because it
/// happens on every first keystroke and must never be in the way.
pub const SWAP: Duration = Duration::from_millis(120);
/// One in-and-out of the typing pulse.
pub const BREATH: Duration = Duration::from_millis(1400);

/// How recent a message has to be for its row to animate in.
///
/// The list mounts rows as they scroll into view, and an animation keyed to
/// mounting would replay on every scroll. Only a message that has *just*
/// arrived gets one.
pub const FRESH_WINDOW: chrono::TimeDelta = chrono::TimeDelta::seconds(4);

pub fn is_fresh(at: chrono::DateTime<chrono::Utc>) -> bool {
    let age = wacore::time::now_utc() - at;
    age >= chrono::TimeDelta::zero() && age < FRESH_WINDOW
}

/// Fade in from transparent.
pub fn fade_in<E>(element: E, id: impl Into<ElementId>) -> impl IntoElement
where
    E: IntoElement + Styled + 'static,
{
    element.with_animation(
        id,
        Animation::new(duration(ENTER)).with_easing(ease_out_quint()),
        |el, t| el.opacity(t),
    )
}

/// Fade in while rising `distance` into place.
pub fn rise_in<E>(element: E, id: impl Into<ElementId>, distance: Pixels) -> impl IntoElement
where
    E: IntoElement + Styled + 'static,
{
    let distance = travel(distance);
    element.with_animation(
        id,
        Animation::new(duration(ARRIVE)).with_easing(ease_out_quint()),
        move |el, t| el.relative().top(distance * (1.0 - t)).opacity(t),
    )
}

/// Fade in while sliding `distance` in from the right — a side panel.
pub fn slide_in_right<E>(element: E, id: impl Into<ElementId>, distance: Pixels) -> impl IntoElement
where
    E: IntoElement + Styled + 'static,
{
    let distance = travel(distance);
    element.with_animation(
        id,
        Animation::new(duration(ENTER)).with_easing(ease_out_quint()),
        move |el, t| el.relative().left(distance * (1.0 - t)).opacity(t),
    )
}

/// A quick fade for a control that has just replaced another in the same
/// place. Opacity only: the control's box is the one it replaced.
pub fn swap_in<E>(element: E, id: impl Into<ElementId>) -> impl IntoElement
where
    E: IntoElement + Styled + 'static,
{
    element.with_animation(
        id,
        Animation::new(duration(SWAP)).with_easing(ease_out_quint()),
        |el, t| el.opacity(t),
    )
}

/// A slow, endless pulse for something happening right now — someone
/// typing. The one looping motion in the app, and it only runs while the
/// thing it describes does.
pub fn breathe<E>(element: E, id: impl Into<ElementId>) -> impl IntoElement
where
    E: IntoElement + Styled + 'static,
{
    element.with_animation(
        id,
        Animation::new(duration(BREATH))
            .repeat()
            .with_easing(gpui::pulsating_between(0.55, 1.0)),
        |el, t| el.opacity(t),
    )
}

/// A brief overshoot in opacity and a settle — a badge or count changing.
pub fn pop_in<E>(element: E, id: impl Into<ElementId>) -> impl IntoElement
where
    E: IntoElement + Styled + 'static,
{
    element.with_animation(
        id,
        Animation::new(duration(POP)).with_easing(ease_out_quint()),
        |el, t| el.opacity(0.35 + 0.65 * t),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_just_arrived_messages_are_fresh() {
        let now = wacore::time::now_utc();
        assert!(is_fresh(now));
        assert!(!is_fresh(now - chrono::TimeDelta::seconds(30)));
        // A clock skewed into the future does not animate forever.
        assert!(!is_fresh(now + chrono::TimeDelta::seconds(30)));
    }
}
