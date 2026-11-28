//! The GUI showcase must render the same pixels whether it is reached through
//! incremental dirty-rect updates or a full redraw.
//!
//! This is the invariant the renderer rests on: an app may only draw inside the
//! rectangles it marked dirty. Anything else leaves stale fragments on the panel.

#[path = "../../../examples/gui-showcase/src/style.rs"]
mod style;

#[path = "../../../examples/gui-showcase/src/app.rs"]
mod app;

use utsuroui::app::App;
use utsuroui::band::MemorySink;
use utsuroui::dirty::Dirty;
use utsuroui::event::Event;
use utsuroui::flush::{RenderCfg, flush};

const BAND_H: u16 = 16;
const BAND_LEN: usize = style::W as usize * BAND_H as usize * 2;
const FRAME_LEN: usize = style::W as usize * style::H as usize * 2;
const STEP_US: u64 = 16_000;

fn render(app: &app::GuiApp, dirty: &Dirty, now: u64, frame: &mut [u8]) {
    let cfg = RenderCfg::new(utsuroui::SCREEN, BAND_H);
    let mut buffers = utsuroui::BandBuffers::<BAND_LEN>::new();
    let mut sink = MemorySink::new(frame, style::W);
    pollster::block_on(flush(app, &mut sink, dirty, now, &mut buffers, &cfg)).expect("flush");
}

fn check(name: &str, events: &[(u64, Event)], steps: u64) {
    let mut app = app::GuiApp::new();
    let mut dirty = Dirty::new(utsuroui::SCREEN);
    let mut frame = vec![0u8; FRAME_LEN];

    dirty.mark_full();
    render(&app, &dirty, 0, &mut frame);
    dirty.clear();

    let mut now = 0;
    for step in 0..steps {
        now += STEP_US;
        for &(at, event) in events {
            if at == step * STEP_US {
                app.event(&event, now, &mut dirty);
            }
        }

        app.tick(now, &mut dirty);
        if !dirty.is_empty() {
            render(&app, &dirty, now, &mut frame);
            dirty.clear();
        }

        let mut reference = vec![0u8; FRAME_LEN];
        let mut full = Dirty::new(utsuroui::SCREEN);
        full.mark_full();
        render(&app, &full, now, &mut reference);

        if frame != reference {
            let diff = frame.iter().zip(&reference).filter(|(a, b)| a != b).count();
            let mut bbox = (u16::MAX, u16::MAX, 0u16, 0u16);
            for i in (0..frame.len()).step_by(2) {
                if frame[i] != reference[i] || frame[i + 1] != reference[i + 1] {
                    let px = (i / 2) as u16;
                    let (x, y) = (px % style::W, px / style::W);
                    bbox = (bbox.0.min(x), bbox.1.min(y), bbox.2.max(x), bbox.3.max(y));
                }
            }
            panic!(
                "{name}: {diff} bytes diverged at step {step} (t={now}), bbox x{}..{} y{}..{}",
                bbox.0, bbox.2, bbox.1, bbox.3
            );
        }
    }
}

#[test]
fn home_selection_converges() {
    check(
        "home next",
        &[(16_000, Event::Next), (64_000, Event::Next)],
        30,
    );
}

#[test]
fn slide_to_controls_converges() {
    check("open controls", &[(16_000, Event::Select)], 40);
}

#[test]
fn toggling_and_sliding_converges() {
    check(
        "controls",
        &[
            (16_000, Event::Select),     // open controls
            (320_000, Event::Select),    // flip the focused toggle
            (480_000, Event::Next),      // focus moves
            (640_000, Event::LongPress), // nothing on a toggle
            (800_000, Event::Back),      // back home
        ],
        60,
    );
}
