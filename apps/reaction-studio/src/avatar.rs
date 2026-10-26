//! How the figure moves for each reaction mood. Pure maths shared by the renderer (keyframes) and
//! the style cards' live previews, so what you pick is what you get.
use crate::style::Mood;

/// Offsets from the neutral face, in sequence pixels / degrees. Positive `bob` moves down.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub bob: f32,
    pub brow_raise: [f32; 2],
    pub brow_tilt: [f32; 2],
    pub eye_open: f32,
}
pub const NEUTRAL: Pose = Pose { bob: 0.0, brow_raise: [0.0, 0.0], brow_tilt: [0.0, 0.0], eye_open: 1.0 };
/// Length of the "reaction beat": the figure's pose is exaggerated right as the video pauses.
pub const BEAT_SECONDS: f64 = 0.3;

fn wave(t: f64, hz: f64) -> f32 {
    (t * hz * std::f64::consts::TAU).sin() as f32
}
/// Eyes close for ~0.12 s every 3.4 s (offset so a hold never starts mid-blink).
pub fn blink(t: f64) -> f32 {
    let phase = (t + 1.0).rem_euclid(3.4);
    if phase < 0.12 { 0.1 } else { 1.0 }
}
/// Pose at `t` seconds into a hold. `gain` (0..=1) scales the motion; non-finite input is neutral.
pub fn pose(mood: Mood, gain: f32, t: f64) -> Pose {
    if !t.is_finite() || !gain.is_finite() {
        return NEUTRAL;
    }
    let t = t.max(0.0);
    let beat = if t < BEAT_SECONDS { 1.6 } else { 1.0 };
    let g = gain.clamp(0.0, 1.0) * beat;
    let mut p = match mood {
        Mood::Excited => Pose { bob: -wave(t, 2.2).abs() * 8.0 * g, brow_raise: [5.0 * g; 2], brow_tilt: [0.0, 0.0], eye_open: 1.0 },
        Mood::Skeptical => Pose { bob: wave(t, 0.5) * 1.0 * g, brow_raise: [-1.0 * g, 6.0 * g], brow_tilt: [10.0 * g, -8.0 * g], eye_open: 0.85 },
        Mood::Amused => Pose { bob: wave(t, 4.0) * 3.0 * g, brow_raise: [2.0 * g; 2], brow_tilt: [-6.0 * g, 6.0 * g], eye_open: 1.0 - 0.45 * g.min(1.0) },
        Mood::Fierce => Pose { bob: (2.0 + wave(t, 1.5) * 1.5) * g, brow_raise: [-3.0 * g; 2], brow_tilt: [18.0 * g, -18.0 * g], eye_open: 0.9 },
        Mood::Calm => Pose { bob: wave(t, 0.6) * 2.5 * g, brow_raise: [0.0, 0.0], brow_tilt: [0.0, 0.0], eye_open: 1.0 },
    };
    p.eye_open = (p.eye_open * blink(t)).clamp(0.05, 1.0);
    p.bob = p.bob.clamp(-12.0, 12.0);
    for v in p.brow_raise.iter_mut() {
        *v = v.clamp(-8.0, 8.0);
    }
    for v in p.brow_tilt.iter_mut() {
        *v = v.clamp(-25.0, 25.0);
    }
    p
}
/// Mouth height (px) from speech loudness, as before, but louder moods open wider.
pub fn mouth_height(rms: f64, gain: f32) -> f64 {
    let rms = if rms.is_finite() { rms.max(0.0) } else { 0.0 };
    let g = if gain.is_finite() { f64::from(gain.clamp(0.0, 1.0)) } else { 0.0 };
    (4.0 + rms * (90.0 + 60.0 * g)).clamp(4.0, 26.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    const MOODS: [Mood; 5] = [Mood::Calm, Mood::Excited, Mood::Skeptical, Mood::Amused, Mood::Fierce];
    fn finite(p: &Pose) -> bool {
        [p.bob, p.brow_raise[0], p.brow_raise[1], p.brow_tilt[0], p.brow_tilt[1], p.eye_open].iter().all(|v| v.is_finite())
    }
    #[test]
    fn poses_stay_bounded_for_any_input() {
        for mood in MOODS {
            for gain in [-5.0, 0.0, 0.5, 1.0, 50.0, f32::NAN, f32::INFINITY] {
                for t in [-1.0, 0.0, 0.1, 1.7, 59.9, 1e9, f64::NAN, f64::INFINITY] {
                    let p = pose(mood, gain, t);
                    assert!(finite(&p), "{mood:?} {gain} {t}");
                    assert!(p.bob.abs() <= 12.0 && p.eye_open >= 0.05 && p.eye_open <= 1.0);
                    assert!(p.brow_raise.iter().all(|v| v.abs() <= 8.0) && p.brow_tilt.iter().all(|v| v.abs() <= 25.0));
                }
            }
        }
    }
    #[test]
    fn zero_gain_is_still_apart_from_blinking() {
        for mood in [Mood::Calm, Mood::Excited, Mood::Fierce] {
            let p = pose(mood, 0.0, 1.0);
            assert_eq!((p.bob, p.brow_raise, p.brow_tilt), (0.0, [0.0, 0.0], [0.0, 0.0]));
        }
    }
    #[test]
    fn moods_have_their_own_faces() {
        let at = |m| pose(m, 1.0, 1.0);
        assert!(at(Mood::Excited).brow_raise.iter().all(|b| *b > 3.0), "excited raises both brows");
        let s = at(Mood::Skeptical);
        assert!(s.brow_raise[1] - s.brow_raise[0] > 4.0, "skeptic raises one brow");
        let f = at(Mood::Fierce);
        assert!(f.brow_raise.iter().all(|b| *b < 0.0) && f.brow_tilt[0] > 10.0 && f.brow_tilt[1] < -10.0, "fierce frowns");
        assert!(at(Mood::Amused).eye_open < 0.7, "amused squints");
        let excited_bounce = (0..50).map(|i| pose(Mood::Excited, 1.0, 1.0 + i as f64 * 0.02).bob.abs()).fold(0.0, f32::max);
        let calm_bounce = (0..50).map(|i| pose(Mood::Calm, 1.0, 1.0 + i as f64 * 0.02).bob.abs()).fold(0.0, f32::max);
        assert!(excited_bounce > calm_bounce * 2.0);
    }
    #[test]
    fn reaction_beat_exaggerates_the_start_of_a_hold() {
        let early = pose(Mood::Skeptical, 1.0, 0.1);
        let later = pose(Mood::Skeptical, 1.0, 1.0);
        assert!(early.brow_raise[1] > later.brow_raise[1]);
    }
    #[test]
    fn blinks_are_short_and_regular() {
        let closed = (0..340).filter(|i| blink(*i as f64 * 0.01) < 0.5).count();
        assert!((10..=13).contains(&closed), "{closed}");
        assert_eq!(blink(0.0), 1.0, "holds start with open eyes");
    }
    #[test]
    fn mouth_height_is_bounded_and_mood_scaled() {
        assert_eq!(mouth_height(0.0, 1.0), 4.0);
        assert_eq!(mouth_height(f64::NAN, f32::NAN), 4.0);
        assert_eq!(mouth_height(10.0, 1.0), 26.0);
        assert!(mouth_height(0.1, 1.0) > mouth_height(0.1, 0.0));
    }
}
