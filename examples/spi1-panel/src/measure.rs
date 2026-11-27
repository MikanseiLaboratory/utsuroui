//! Synthetic measurement producer on core 1 (shared waveform).

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
