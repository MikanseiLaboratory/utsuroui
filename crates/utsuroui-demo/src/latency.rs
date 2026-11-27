//! Deterministic synthetic latency source for the firmware examples.
//!
//! A real product timestamps GPIO edges; the examples generate a triangle wave
//! on core 1 so the UI has something to display. The waveform is a pure
//! function of the step counter, which keeps the firmware demos reproducible.

/// Bottom of the synthetic waveform, in tenths of a millisecond (25.0 ms).
pub const MIN_X10: u16 = 250;
/// Height of the triangle wave, in tenths of a millisecond.
pub const SPAN_X10: u16 = 200;
/// Number of steps in one triangle period.
pub const PERIOD: u32 = 120;

/// The synthetic latency value for step `n`, in tenths of a millisecond.
///
/// A triangle wave with a slow period plus a small dither, so the tween and the
/// bar graph both move without ever jumping.
pub fn synthetic(n: u32) -> u16 {
    let phase = n % PERIOD;
    let triangle = if phase < PERIOD / 2 {
        phase * 2
    } else {
        (PERIOD - phase) * 2
    };
    let dither = (n % 3) as u16;
    MIN_X10 + ((triangle * SPAN_X10 as u32) / PERIOD) as u16 + dither
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waveform_stays_in_range() {
        for n in 0..1000 {
            let v = synthetic(n);
            assert!(
                (MIN_X10..=MIN_X10 + SPAN_X10 + 2).contains(&v),
                "value {v} out of range at step {n}"
            );
        }
    }

    #[test]
    fn waveform_is_periodic_with_dither() {
        assert_eq!(synthetic(0), MIN_X10);
        assert_eq!(synthetic(PERIOD), MIN_X10);
        // The peak sits at the middle of the period.
        assert!(synthetic(PERIOD / 2) >= MIN_X10 + SPAN_X10);
    }

    #[test]
    fn waveform_moves_smoothly() {
        let mut previous = synthetic(0);
        for n in 1..PERIOD {
            let v = synthetic(n);
            // The triangle moves ~3.3 tenths per step; the dither adds one.
            assert!(
                v.abs_diff(previous) <= 6,
                "jump of {} at step {n}",
                v.abs_diff(previous)
            );
            previous = v;
        }
    }
}
