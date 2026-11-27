//! Front panel buttons.
//!
//! | Key | GPIO | Event |
//! |---|---|---|
//! | MENU | GP8 | `Next` (auto-repeat) |
//! | PREV | GP9 | `Previous` (auto-repeat) |
//! | INPUT 1 | GP1 | `Select` |
//! | BACK | GP26 | `Back` |
//!
//! All keys are wired to GND with pull-ups. The debounce is short (5 ms) so a
//! press reaches the UI in roughly 10 ms, including the 5 ms poll interval.

use embassy_rp::gpio::Input;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Sender;
use embassy_time::{Duration, Instant, Timer};
use utsuroui::event::Event;
use utsuroui::input::{DebounceConfig, Debouncer, Edge};

use crate::EVENT_QUEUE;

/// Poll interval for the buttons.
const POLL_MS: u64 = 3;

#[embassy_executor::task]
pub async fn task(
    menu_pin: Input<'static>,
    prev_pin: Input<'static>,
    input1_pin: Input<'static>,
    back_pin: Input<'static>,
    events: Sender<'static, CriticalSectionRawMutex, Event, EVENT_QUEUE>,
) {
    let cfg = DebounceConfig::new(5_000, 500_000, 200_000, 100_000);
    let mut menu = Debouncer::new(cfg);
    let mut prev = Debouncer::new(cfg);
    let mut input1 = Debouncer::new(cfg);
    let mut back = Debouncer::new(cfg);

    loop {
        let now = Instant::now().as_micros();

        if let Some(edge) = menu.update(menu_pin.is_low(), now) {
            if matches!(edge, Edge::Pressed | Edge::Repeat) {
                events.try_send(Event::Next).ok();
            }
        }
        if let Some(edge) = prev.update(prev_pin.is_low(), now) {
            if matches!(edge, Edge::Pressed | Edge::Repeat) {
                events.try_send(Event::Previous).ok();
            }
        }
        if let Some(edge) = input1.update(input1_pin.is_low(), now) {
            match edge {
                Edge::Pressed => {
                    events.try_send(Event::Select).ok();
                }
                Edge::LongPress => {
                    events.try_send(Event::LongPress).ok();
                }
                _ => {}
            }
        }
        if let Some(edge) = back.update(back_pin.is_low(), now) {
            if matches!(edge, Edge::Pressed) {
                events.try_send(Event::Back).ok();
            }
        }

        Timer::after(Duration::from_millis(POLL_MS)).await;
    }
}
