//! Embassy frame scheduler (`embassy` feature).
//!
//! The loop is the one described in DESIGN.md section 12.4:
//!
//! ```text
//! loop {
//!     now = Instant::now()
//!     drain input events -> app.event
//!     latest latency value -> app.latency (the app throttles the display rate)
//!     animating = app.tick
//!     if nothing to do: await an event, a latency update, or a deadline
//!     flush dirty rectangles as bands
//!     if animating: sleep to the frame budget
//! }
//! ```
//!
//! The measurement side lives on core 1 and only writes a `Watch` slot, so the
//! UI never blocks it (DESIGN.md section 12.1).

use embassy_futures::select::{Either, select};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Receiver;
use embassy_sync::watch::Receiver as WatchReceiver;
use embassy_time::{Duration, Timer};

use crate::app::App;
use crate::band::BandSink;
use crate::dirty::Dirty;
use crate::event::Event;
use crate::flush::{RenderCfg, flush};

/// Number of queued input events in [`ui_task`].
pub const EVENT_QUEUE: usize = 8;
/// Number of latency watch slots in [`ui_task`].
pub const LATENCY_SLOTS: usize = 4;

/// Configuration for [`ui_task`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UiConfig {
    /// Panel geometry and band height.
    pub render: RenderCfg,
    /// Target frame interval while animating, in microseconds (22 ms ≈ 45 fps).
    pub frame_budget_us: u32,
}

impl UiConfig {
    /// Creates a configuration.
    pub const fn new(render: RenderCfg, frame_budget_us: u32) -> Self {
        Self {
            render,
            frame_budget_us,
        }
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            render: RenderCfg::new(crate::SCREEN, 16),
            frame_budget_us: 22_000,
        }
    }
}

/// Runs the UI frame loop forever.
///
/// `band_b` is only used while the app reports a crossfade; pass a second
/// stripe. Both buffers must be at least
/// [`RenderCfg::band_buffer_len`](crate::flush::RenderCfg::band_buffer_len)
/// bytes.
pub async fn ui_task<A, S>(
    app: &mut A,
    sink: &mut S,
    band_a: &mut [u8],
    band_b: &mut [u8],
    events: &mut Receiver<'static, CriticalSectionRawMutex, Event, EVENT_QUEUE>,
    latency: &mut WatchReceiver<'static, CriticalSectionRawMutex, u16, LATENCY_SLOTS>,
    cfg: UiConfig,
) -> Result<(), S::Error>
where
    A: App,
    S: BandSink,
{
    let mut dirty = Dirty::new(cfg.render.screen);
    dirty.mark_full();

    loop {
        let now_us = embassy_time::Instant::now().as_micros();

        while let Ok(event) = events.try_receive() {
            app.event(&event, now_us, &mut dirty);
        }
        if let Some(value) = latency.try_get() {
            app.latency(value, now_us, &mut dirty);
        }

        let animating = app.tick(now_us, &mut dirty);

        if dirty.is_empty() && !animating {
            // Nothing to paint: sleep until input, a new measurement or a
            // tween deadline wakes us. No SPI traffic while idle.
            match select(events.receive(), latency.get()).await {
                Either::First(_) | Either::Second(_) => {}
            }
            continue;
        }

        flush(app, sink, &dirty, now_us, band_a, Some(band_b), &cfg.render).await?;
        dirty.clear();

        if animating {
            if let Some(deadline) = app.next_wakeup_us() {
                let now = embassy_time::Instant::now().as_micros();
                if deadline > now {
                    Timer::after(Duration::from_micros(deadline - now)).await;
                }
            }
            Timer::after(Duration::from_micros(cfg.frame_budget_us as u64)).await;
        }
    }
}
