//! Partial redraws must converge to what a full redraw would show.
//!
//! The firmware only repaints the rectangles the app marks dirty, so any state
//! change the app does not fully cover leaves stale pixels on the panel. The
//! golden tests always redraw the whole screen and therefore cannot catch this.
//!
//! Each case replays an event sequence, flushing **only** the dirty rectangles
//! over the previous frame, and compares the result with a full redraw at the
//! same instant.

use utsuroui::app::App;
use utsuroui::band::MemorySink;
use utsuroui::dirty::Dirty;
use utsuroui::event::Event;
use utsuroui::flush::{RenderCfg, flush};
use utsuroui_demo::app::DemoApp;
use utsuroui_demo::render::{BAND_LEN, FRAME_LEN};
use utsuroui_demo::style::{SCREEN, W};

const BAND_H: u16 = 16;
const START_US: u64 = 100_000;
const STEP_US: u64 = 4_000;

fn render_full(app: &DemoApp, now_us: u64, buf: &mut [u8]) {
    let mut dirty = Dirty::new(SCREEN);
    dirty.mark_full();
    let cfg = RenderCfg::new(SCREEN, BAND_H);
    let mut band_a = vec![0u8; BAND_LEN];
    let mut band_b = vec![0u8; BAND_LEN];
    let mut sink = MemorySink::new(buf, W);
    pollster::block_on(flush(
        app,
        &mut sink,
        &dirty,
        now_us,
        &mut band_a,
        Some(&mut band_b),
        &cfg,
    ))
    .unwrap();
}

fn render_dirty(
    app: &DemoApp,
    now_us: u64,
    dirty: &Dirty,
    buf: &mut [u8],
    band_a: &mut [u8],
    band_b: &mut [u8],
) {
    let cfg = RenderCfg::new(SCREEN, BAND_H);
    let mut sink = MemorySink::new(buf, W);
    pollster::block_on(flush(
        app,
        &mut sink,
        dirty,
        now_us,
        band_a,
        Some(band_b),
        &cfg,
    ))
    .unwrap();
}

/// Replays `events`, then ticks for `steps` frames, and asserts that the
/// incrementally updated frame always equals a full redraw.
fn assert_converges(name: &str, events: &[(u64, Event)], steps: u64) {
    let mut app = DemoApp::new();
    let mut dirty = Dirty::new(SCREEN);
    let mut band_a = vec![0u8; BAND_LEN];
    let mut band_b = vec![0u8; BAND_LEN];
    let mut frame = vec![0u8; FRAME_LEN];

    dirty.mark_full();
    render_dirty(&app, 0, &dirty, &mut frame, &mut band_a, &mut band_b);
    dirty.clear();

    for &(at, event) in events {
        app.event(&event, at, &mut dirty);
    }

    for step in 0..steps {
        let now = START_US + step * STEP_US;

        if let Some((_, event)) = events.iter().rev().find(|(at, _)| *at == now) {
            app.event(event, now, &mut dirty);
        }

        app.tick(now, &mut dirty);
        if !dirty.is_empty() {
            render_dirty(&app, now, &dirty, &mut frame, &mut band_a, &mut band_b);
            dirty.clear();
        }

        let mut reference = vec![0u8; FRAME_LEN];
        render_full(&app, now, &mut reference);

        if frame != reference {
            let diff = frame.iter().zip(&reference).filter(|(a, b)| a != b).count();
            panic!("{name}: {diff} bytes diverged from a full redraw at step {step} (t={now} us)");
        }
    }
}

#[test]
fn menu_highlight_tween_converges() {
    assert_converges(
        "menu next",
        &[(START_US, Event::Next)],
        40,
    );
}

#[test]
fn menu_retarget_converges() {
    assert_converges(
        "menu next twice",
        &[(START_US, Event::Next), (START_US + 40_000, Event::Next)],
        60,
    );
}

#[test]
fn menu_previous_converges() {
    assert_converges(
        "menu previous",
        &[(START_US + 60_000, Event::Previous)],
        60,
    );
}
