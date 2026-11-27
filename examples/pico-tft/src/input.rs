//! Two-button input task.
//!
//! Polls both buttons at 200 Hz and feeds the pure [`Debouncer`] from the core
//! crate, so the timing logic stays host-testable. Button A is "next" (long
//! press = back, repeat = next), button B is "select".

use embassy_rp::gpio::Input;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Sender;
use embassy_time::{Duration, Instant, Timer};
use utsuroui::event::Event;
use utsuroui::input::{DebounceConfig, Debouncer, Edge};

use crate::EVENT_QUEUE;

/// Poll interval for the buttons.
const POLL_MS: u64 = 5;

#[embassy_executor::task]
pub async fn task(
    button_a: Input<'static>,
    button_b: Input<'static>,
    events: Sender<'static, CriticalSectionRawMutex, Event, EVENT_QUEUE>,
) {
    let cfg = DebounceConfig::default();
    let mut a = Debouncer::new(cfg);
    let mut b = Debouncer::new(cfg);

    loop {
        let now = Instant::now().as_micros();

        // Buttons are wired to GND with pull-ups, so pressed == low.
        if let Some(edge) = a.update(button_a.is_low(), now) {
            let event = match edge {
                Edge::Pressed => Some(Event::Next),
                Edge::Repeat => Some(Event::Next),
                Edge::LongPress => Some(Event::Back),
                Edge::Released => None,
            };
            if let Some(event) = event {
                events.try_send(event).ok();
            }
        }
        if let Some(edge) = b.update(button_b.is_low(), now) {
            let event = match edge {
                Edge::Pressed => Some(Event::Select),
                Edge::LongPress => Some(Event::LongPress),
                Edge::Repeat | Edge::Released => None,
            };
            if let Some(event) = event {
                events.try_send(event).ok();
            }
        }

        Timer::after(Duration::from_millis(POLL_MS)).await;
    }
}
