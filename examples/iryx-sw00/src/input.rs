//! SW-00 front panel buttons.
//!
//! The board exposes the eight front-panel keys of the original switcher:
//! angle/input 1-4 on GP1-GP4, mark IN/OUT on GP9/GP10, MENU on GP8 and BACK on
//! GP26 (plus SHIFT on GP15). The demo UI only needs four of them:
//!
//! | Key | GPIO | Event |
//! |---|---|---|
//! | MENU | GP8 | `Next` (auto-repeat) |
//! | MARK IN | GP9 | `Previous` (auto-repeat) |
//! | INPUT 1 | GP1 | `Select` |
//! | BACK | GP26 | `Back` |
//!
//! All keys are wired to GND with pull-ups; the pure `Debouncer` from the core
//! crate provides debounce and repeat, so the timing stays host-testable.

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
    menu_pin: Input<'static>,
    mark_in_pin: Input<'static>,
    input1_pin: Input<'static>,
    back_pin: Input<'static>,
    events: Sender<'static, CriticalSectionRawMutex, Event, EVENT_QUEUE>,
) {
    let cfg = DebounceConfig::default();
    let mut menu = Debouncer::new(cfg);
    let mut mark_in = Debouncer::new(cfg);
    let mut input1 = Debouncer::new(cfg);
    let mut back = Debouncer::new(cfg);

    loop {
        let now = Instant::now().as_micros();

        if let Some(edge) = menu.update(menu_pin.is_low(), now) {
            if matches!(edge, Edge::Pressed | Edge::Repeat) {
                events.try_send(Event::Next).ok();
            }
        }
        if let Some(edge) = mark_in.update(mark_in_pin.is_low(), now) {
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
