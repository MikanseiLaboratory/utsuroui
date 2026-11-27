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
use utsuroui_demo::latency::synthetic;

use crate::LATENCY_SLOTS;

/// Sample period of the synthetic measurement, in milliseconds.
const PERIOD_MS: u64 = 5;

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
