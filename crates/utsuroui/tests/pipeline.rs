//! The pipelined flush must overlap painting with the transfer, and produce
//! exactly the same pixels as the serial one.
#![cfg(feature = "pipeline")]

use std::sync::Mutex;

use utsuroui::band::{BandBuffers, BandSink, MemorySink};
use utsuroui::color::BLACK;
use utsuroui::dirty::Dirty;
use utsuroui::flush::{RenderCfg, flush, flush_pipelined};
use utsuroui::{App, DirtyList, Event, PaintCx, Rect, SCREEN};

const BAND_H: u16 = 16;
const LEN: usize = 240 * BAND_H as usize * 2;

/// Ordered record of what happened, so the test can prove interleaving.
static LOG: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());

fn log(event: &'static str) {
    LOG.lock().expect("log lock").push(event);
}

/// A sink whose transfer is pending for one poll, like a DMA transfer.
struct SlowSink;

impl BandSink for SlowSink {
    type Error = core::convert::Infallible;

    async fn write_band(&mut self, _band: Rect, _data: &[u8]) -> Result<(), Self::Error> {
        log("transfer-start");
        embassy_futures::yield_now().await;
        log("transfer-end");
        Ok(())
    }
}

struct Painter;

impl App for Painter {
    fn event(&mut self, _event: &Event, _now_us: u64, _dirty: &mut DirtyList) {}

    fn tick(&mut self, _now_us: u64, _dirty: &mut DirtyList) -> bool {
        true
    }

    fn paint(&self, cx: &mut PaintCx<'_>, _now_us: u64) {
        log("paint");
        cx.fill_rect(SCREEN, BLACK);
    }
}

#[test]
fn transfer_and_paint_overlap() {
    LOG.lock().expect("log lock").clear();

    let mut dirty = Dirty::new(SCREEN);
    dirty.mark_full();
    let cfg = RenderCfg::new(SCREEN, BAND_H);
    let mut buffers = BandBuffers::<LEN>::new();

    pollster::block_on(flush_pipelined(
        &Painter,
        &mut SlowSink,
        &dirty,
        0,
        &mut buffers,
        &cfg,
    ))
    .expect("flush");

    let events = LOG.lock().expect("log lock").clone();

    // A paint must happen while a transfer is in flight: start, paint, end.
    let overlapped = events.windows(3).any(|w| {
        w[0] == "transfer-start" && w[1] == "paint" && w[2] == "transfer-end"
            || w[0] == "transfer-start" && w[1] == "paint"
    });
    assert!(
        overlapped,
        "no paint happened between a transfer start and its end: {events:?}"
    );

    // Every transfer that started must also finish.
    let starts = events.iter().filter(|e| **e == "transfer-start").count();
    let ends = events.iter().filter(|e| **e == "transfer-end").count();
    assert_eq!(starts, ends, "unbalanced transfers: {events:?}");
    assert_eq!(starts, 20, "a full 320-row flush is 20 bands of 16 rows");
}

#[test]
fn pipelined_and_serial_flush_agree() {
    let app = Painter;
    let mut dirty = Dirty::new(SCREEN);
    dirty.mark_full();
    let cfg = RenderCfg::new(SCREEN, BAND_H);

    let mut serial_frame = vec![0u8; 240 * 320 * 2];
    {
        let mut buffers = BandBuffers::<LEN>::new();
        let mut sink = MemorySink::new(&mut serial_frame, 240);
        pollster::block_on(flush(&app, &mut sink, &dirty, 0, &mut buffers, &cfg)).expect("flush");
    }

    let mut pipelined_frame = vec![0u8; 240 * 320 * 2];
    {
        let mut buffers = BandBuffers::<LEN>::new();
        let mut sink = MemorySink::new(&mut pipelined_frame, 240);
        pollster::block_on(flush_pipelined(
            &app,
            &mut sink,
            &dirty,
            0,
            &mut buffers,
            &cfg,
        ))
        .expect("flush");
    }

    assert_eq!(serial_frame, pipelined_frame, "same pixels from both paths");
}
