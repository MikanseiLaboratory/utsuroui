//! Fixed-point tweening.
//!
//! Time is normalised to `Q16` (`0..=65535`) and every interpolation is done in
//! integers, so a Cortex-M0+ never pulls in soft-float. Easing curves are
//! precomputed into 257-entry tables (one extra point for interpolation) that
//! live in flash.

/// Normalised progress in `0..=65535`.
pub type Q16 = u16;

/// Easing curves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Easing {
    /// Constant speed.
    Linear,
    /// Slow start (quadratic in).
    EaseIn,
    /// Slow end (quadratic out).
    EaseOut,
    /// Slow start and end (quadratic in-out); the default for UI motion.
    #[default]
    EaseInOut,
}

const LUT_LEN: usize = 257;

const fn sq(x: u64) -> u64 {
    (x * x) / 65_535
}

/// Samples an easing curve at `i/256` in `Q16`.
const fn sample(kind: Easing, i: usize) -> u16 {
    // t = i / 256 in Q16.
    let t = i as u64 * 65_535 / 256;
    let rem = 65_535u64 - t;
    match kind {
        Easing::Linear => t as u16,
        Easing::EaseIn => sq(t) as u16,
        Easing::EaseOut => (65_535 - sq(rem)) as u16,
        Easing::EaseInOut => {
            if t < 32_768 {
                (2 * sq(t)) as u16
            } else {
                (65_535 - 2 * sq(rem)) as u16
            }
        }
    }
}

const fn build_lut(kind: Easing) -> [u16; LUT_LEN] {
    let mut lut = [0u16; LUT_LEN];
    let mut i = 0;
    while i < LUT_LEN {
        lut[i] = sample(kind, i);
        i += 1;
    }
    lut
}

static EASE_IN: [u16; LUT_LEN] = build_lut(Easing::EaseIn);
static EASE_OUT: [u16; LUT_LEN] = build_lut(Easing::EaseOut);
static EASE_IN_OUT: [u16; LUT_LEN] = build_lut(Easing::EaseInOut);

/// Returns the easing lookup table for `kind` (`None` for linear).
pub fn lut(kind: Easing) -> Option<&'static [u16; LUT_LEN]> {
    match kind {
        Easing::Linear => None,
        Easing::EaseIn => Some(&EASE_IN),
        Easing::EaseOut => Some(&EASE_OUT),
        Easing::EaseInOut => Some(&EASE_IN_OUT),
    }
}

/// Applies `kind` to progress `t`.
pub fn ease(kind: Easing, t: Q16) -> Q16 {
    match kind {
        Easing::Linear => t,
        _ => {
            let table = match kind {
                Easing::EaseIn => &EASE_IN,
                Easing::EaseOut => &EASE_OUT,
                Easing::EaseInOut => &EASE_IN_OUT,
                Easing::Linear => unreachable!(),
            };
            // pos in 0..=65280 == 256 * 255, so idx in 0..=256 and frac in 0..=254.
            let pos = t as u64 * 256 * 255 / 65_535;
            let idx = (pos / 255) as usize;
            if idx >= 256 {
                return table[256];
            }
            let frac = (pos % 255) as u32;
            let a = table[idx] as u32;
            let b = table[idx + 1] as u32;
            let value = a + ((b - a) * frac) / 255;
            value as u16
        }
    }
}

/// Linear interpolation between two `i32` values, `t` in `0..=65535`.
pub fn lerp_i32(from: i32, to: i32, t: Q16) -> i32 {
    let t = t as i64;
    let u = 65_535 - t;
    ((from as i64 * u + to as i64 * t) / 65_535) as i32
}

/// A scalar animated from `from` to `to` over `duration_us`.
///
/// The same type animates positions, fixed-point values, alpha and (via
/// [`Tween::progress`]) colours.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tween {
    from: i32,
    to: i32,
    start_us: u64,
    duration_us: u32,
    easing: Easing,
    running: bool,
}

impl Default for Tween {
    fn default() -> Self {
        Self::settled(0)
    }
}

impl Tween {
    /// A tween that is already at `value`.
    pub const fn settled(value: i32) -> Self {
        Self {
            from: value,
            to: value,
            start_us: 0,
            duration_us: 0,
            easing: Easing::EaseInOut,
            running: false,
        }
    }

    /// Starts a new animation from `from` to `to`.
    pub fn start(&mut self, now_us: u64, from: i32, to: i32, duration_us: u32, easing: Easing) {
        self.from = from;
        self.to = to;
        self.start_us = now_us;
        self.duration_us = duration_us;
        self.easing = easing;
        self.running = from != to && duration_us > 0;
    }

    /// Retargets a running (or settled) tween without a jump.
    ///
    /// The current value becomes the new starting value. This is what makes a
    /// rapidly pressed "next" button feel continuous instead of restarting from
    /// the previous row.
    pub fn retarget(&mut self, now_us: u64, new_to: i32, duration_us: u32) {
        self.retarget_with(now_us, new_to, duration_us, self.easing);
    }

    /// Retargets with a new easing curve.
    pub fn retarget_with(&mut self, now_us: u64, new_to: i32, duration_us: u32, easing: Easing) {
        let current = self.value_at(now_us);
        self.from = current;
        self.to = new_to;
        self.start_us = now_us;
        self.duration_us = duration_us;
        self.easing = easing;
        self.running = current != new_to && duration_us > 0;
    }

    /// Stops the animation at its current value.
    pub fn stop(&mut self, now_us: u64) {
        let current = self.value_at(now_us);
        self.from = current;
        self.to = current;
        self.running = false;
        self.duration_us = 0;
    }

    /// Value at `now_us`; the target value once the animation is over.
    pub fn value_at(&self, now_us: u64) -> i32 {
        if !self.running || self.duration_us == 0 {
            return self.to;
        }
        let elapsed = now_us.saturating_sub(self.start_us);
        if elapsed >= self.duration_us as u64 {
            return self.to;
        }
        let t = ((elapsed * 65_535) / self.duration_us as u64) as u16;
        lerp_i32(self.from, self.to, ease(self.easing, t))
    }

    /// Whether the animation still has work to do at `now_us`.
    pub fn is_running(&self, now_us: u64) -> bool {
        self.running && now_us.saturating_sub(self.start_us) < self.duration_us as u64
    }

    /// Eased progress at `now_us` (`0..=65535`).
    pub fn progress(&self, now_us: u64) -> Q16 {
        if !self.running || self.duration_us == 0 {
            return 65_535;
        }
        let elapsed = now_us.saturating_sub(self.start_us);
        if elapsed >= self.duration_us as u64 {
            return 65_535;
        }
        let t = ((elapsed * 65_535) / self.duration_us as u64) as u16;
        ease(self.easing, t)
    }

    /// Starting value of the current animation.
    pub const fn from(&self) -> i32 {
        self.from
    }

    /// Target value.
    pub const fn target(&self) -> i32 {
        self.to
    }

    /// Whether a `duration_us` was assigned.
    pub const fn is_moving(&self) -> bool {
        self.running
    }

    /// When the current animation ends (`None` when settled).
    pub fn end_us(&self) -> Option<u64> {
        if self.running {
            Some(self.start_us + self.duration_us as u64)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settled_returns_value() {
        let t = Tween::settled(42);
        assert_eq!(t.value_at(0), 42);
        assert_eq!(t.value_at(u64::MAX), 42);
        assert!(!t.is_running(0));
        assert_eq!(t.end_us(), None);
    }

    #[test]
    fn linear_endpoints() {
        let mut t = Tween::settled(0);
        t.start(1_000, 10, 90, 100_000, Easing::Linear);
        assert_eq!(t.value_at(1_000), 10);
        assert_eq!(t.value_at(101_000), 90);
        assert_eq!(t.value_at(500_000), 90);
        assert_eq!(t.value_at(51_000), 49);
    }

    #[test]
    fn is_running_boundaries() {
        let mut t = Tween::settled(0);
        t.start(0, 0, 100, 100_000, Easing::Linear);
        assert!(t.is_running(99_999));
        assert!(!t.is_running(100_000));
        assert_eq!(t.end_us(), Some(100_000));
    }

    #[test]
    fn retarget_keeps_value_continuous() {
        let mut t = Tween::settled(0);
        t.start(0, 0, 100, 100_000, Easing::Linear);
        let mid = t.value_at(50_000);
        assert_eq!(mid, 49);
        t.retarget(50_000, 200, 100_000);
        assert_eq!(t.value_at(50_000), mid);
        assert_eq!(t.value_at(150_000), 200);
        assert_eq!(t.from(), 49);
    }

    #[test]
    fn retarget_to_same_value_settles_immediately() {
        let mut t = Tween::settled(7);
        t.retarget(0, 7, 100_000);
        assert!(!t.is_moving());
        assert_eq!(t.value_at(0), 7);
    }

    #[test]
    fn zero_duration_is_instant() {
        let mut t = Tween::settled(0);
        t.start(0, 0, 10, 0, Easing::EaseInOut);
        assert!(!t.is_running(0));
        assert_eq!(t.value_at(0), 10);
    }

    #[test]
    fn stop_freezes_value() {
        let mut t = Tween::settled(0);
        t.start(0, 0, 100, 100_000, Easing::Linear);
        t.stop(50_000);
        assert_eq!(t.value_at(50_000), 49);
        assert_eq!(t.value_at(200_000), 49);
        assert!(!t.is_running(60_000));
    }

    #[test]
    fn ease_curves_are_monotonic() {
        for kind in [
            Easing::Linear,
            Easing::EaseIn,
            Easing::EaseOut,
            Easing::EaseInOut,
        ] {
            let mut prev = ease(kind, 0);
            assert_eq!(prev, 0, "{kind:?} must start at 0");
            for step in 1..=256u32 {
                let t = (step * 65_535 / 256) as u16;
                let v = ease(kind, t);
                assert!(v >= prev, "{kind:?} is not monotonic at {t}");
                prev = v;
            }
            assert_eq!(ease(kind, 65_535), 65_535, "{kind:?} must end at 1");
        }
    }

    #[test]
    fn ease_in_out_is_symmetric() {
        // The 1/256 lookup grid is not mirrored exactly, so allow a few LSBs
        // of sampling error between `t` and `1 - t`.
        for t in [0u16, 8_192, 16_384, 32_768, 49_152] {
            let a = ease(Easing::EaseInOut, t) as i32;
            let mirrored = ease(Easing::EaseInOut, 65_535 - t) as i32;
            assert!((a + mirrored - 65_535).abs() <= 64, "t={t}");
        }
    }

    #[test]
    fn ease_out_is_ahead_of_linear_halfway() {
        assert!(ease(Easing::EaseOut, 32_768) > 32_768);
        assert!(ease(Easing::EaseIn, 32_768) < 32_768);
    }

    #[test]
    fn lerp_extremes_do_not_overflow() {
        assert_eq!(lerp_i32(i32::MIN, i32::MAX, 0), i32::MIN);
        assert_eq!(lerp_i32(i32::MIN, i32::MAX, 65_535), i32::MAX);
        let v = lerp_i32(i32::MIN, i32::MAX, 32_768);
        // The midpoint of i32::MIN..i32::MAX is about zero, but the weight
        // split is asymmetric by one LSB, so the result is one half-range step.
        assert!(v.abs() <= 32_768);
    }

    #[test]
    fn progress_tracks_easing() {
        let mut t = Tween::settled(0);
        t.start(0, 0, 1000, 100_000, Easing::Linear);
        assert_eq!(t.progress(0), 0);
        assert_eq!(t.progress(100_000), 65_535);
        assert_eq!(t.progress(50_000), 32_767);
    }
}
