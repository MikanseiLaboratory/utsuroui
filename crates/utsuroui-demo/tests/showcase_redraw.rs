//! Every showcase scene must produce the same pixels whether it is reached by
//! incremental dirty-rect updates or by a full redraw.
//!
//! This is the invariant the whole renderer rests on: a scene may only draw
//! inside the rectangle it marked dirty. A scene that paints outside its dirty
//! rect leaves stale fragments on the panel and shows up here as a mismatch.

#[path = "../../../examples/anim-showcase/src/scenes.rs"]
mod scenes;

use utsuroui::app::App;
use utsuroui::band::MemorySink;
use utsuroui::dirty::Dirty;
use utsuroui::event::Event;
use utsuroui::flush::{RenderCfg, flush};

const W: u16 = 240;
const H: u16 = 320;
const BAND_H: u16 = 16;
const BAND_LEN: usize = W as usize * BAND_H as usize * 2;
const FRAME_LEN: usize = W as usize * H as usize * 2;
const STEP_US: u64 = 16_000;

fn render(app: &scenes::Showcase, dirty: &Dirty, now: u64, frame: &mut [u8]) {
    let cfg = RenderCfg::new(utsuroui::SCREEN, BAND_H);
    let mut buffers = utsuroui::BandBuffers::<BAND_LEN>::new();
    let mut sink = MemorySink::new(frame, W);
    pollster::block_on(flush(
        app,
        &mut sink,
        dirty,
        now,
        &mut buffers,
        &cfg,
    ))
    .unwrap();
}

fn check_scene(index: usize, name: &str) {
    let mut app = scenes::Showcase::new();
    let mut dirty = Dirty::new(utsuroui::SCREEN);
    for _ in 0..index {
        app.event(&Event::Next, 0, &mut dirty);
    }

    let mut frame = vec![0u8; FRAME_LEN];
    // The firmware starts with a full redraw; do the same here.
    dirty.mark_full();
    render(&app, &dirty, STEP_US, &mut frame);
    dirty.clear();
    let mut now = STEP_US;

    for step in 0..150u64 {
        app.tick(now, &mut dirty);
        if !dirty.is_empty() {
            render(&app, &dirty, now, &mut frame);
            dirty.clear();
        }

        let mut reference = vec![0u8; FRAME_LEN];
        let mut full = Dirty::new(utsuroui::SCREEN);
        full.mark_full();
        render(&app, &full, now, &mut reference);

        let diff: Vec<usize> = frame
            .iter()
            .zip(reference.iter())
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(i, _)| i)
            .collect();
        if !diff.is_empty() {
            let (mut x0, mut y0, mut x1, mut y1) = (u16::MAX, u16::MAX, 0u16, 0u16);
            for i in diff.iter().step_by(2) {
                let px = (*i / 2) as u16;
                let (x, y) = (px % W, px / W);
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
            }
            panic!(
                "{name}: {} bytes diverged at step {step} (t={now}), bbox x{x0}..{x1} y{y0}..{y1}",
                diff.len()
            );
        }

        if step == 20 {
            app.event(&Event::Select, now, &mut dirty); // pause
            app.event(&Event::Select, now, &mut dirty); // resume
            dirty.clear();
            let mut full = Dirty::new(utsuroui::SCREEN);
            full.mark_full();
            render(&app, &full, now, &mut frame);
        }

        now += STEP_US;
    }
}

#[test]
fn waveform_converges() {
    check_scene(0, "Waveform");
}

#[test]
fn ticker_converges() {
    check_scene(1, "Ticker");
}

#[test]
fn float_converges() {
    check_scene(2, "Float");
}

#[test]
fn spectrum_converges() {
    check_scene(3, "Spectrum");
}
