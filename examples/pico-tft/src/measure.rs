//! Synthetic measurement producer on core 1.
//!
//! The real product timestamps a GPIO edge (and an output pulse) here; the demo
//! generates a deterministic triangle wave so the UI has something to display.
//!
//! Core 1 runs a bare loop: it never takes the UI's locks, it only writes the
//! latest value into an `embassy_sync::watch::Watch` slot. That is the
//! isolation described in DESIGN.md section 12.1.

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::watch::Sender;
use embassy_time::{Duration, Instant};

use crate::LATENCY_SLOTS;

/// Sample period of the synthetic measurement, in milliseconds.
const PERIOD_MS: u64 = 5;
/// Bottom of the synthetic waveform, in tenths of a millisecond (25.0 ms).
const MIN_X10: u16 = 250;
/// Height of the triangle wave, in tenths of a millisecond.
const SPAN_X10: u16 = 200;

/// Runs forever on core 1.
pub fn run(sender: Sender<'static, CriticalSectionRawMutex, u16, LATENCY_SLOTS>) -> ! {
    let mut step = 0u32;
    loop {
        sender.send(synthetic(step));
        step = step.wrapping_add(1);

        let deadline = Instant::now() + Duration::from_millis(PERIOD_MS);
        while Instant::now() < deadline {
            core::hint::spin_loop();
        }
    }
}

/// The synthetic latency value for step `n`, in tenths of a millisecond.
///
/// A triangle wave with a slow period plus a one-tenth dither, so the tween and
/// the bar graph both move without ever jumping.
pub fn synthetic(n: u32) -> u16 {
    let period = 120u32;
    let phase = n % period;
    let triangle = if phase < period / 2 {
        phase * 2
    } else {
        (period - phase) * 2
    };
    let dither = (n % 3) as u16;
    MIN_X10 + ((triangle * SPAN_X10 as u32) / period) as u16 + dither
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
                "value {v} out of range at {n}"
            );
        }
    }
}
