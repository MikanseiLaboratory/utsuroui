//! Frame-budget checks for the dirty-rect renderer.
//!
//! A highlight move from row N to row N+1 must transfer a band-sized region,
//! not a screen. The numbers here are the host-measured byte counts; on hardware
//! the same byte count divided by the SPI clock gives the transfer time.

use utsuroui::app::App;
use utsuroui::color::{BLACK, WHITE};
use utsuroui::dirty::{Dirty, DirtyList};
use utsuroui::event::Event;
use utsuroui::flush::RenderCfg;
use utsuroui::flush::flush;
use utsuroui::geom::Rect;
use utsuroui_demo::app::DemoApp;
use utsuroui_demo::render::{BAND_H, BAND_LEN};
use utsuroui_demo::style::SCREEN;

#[test]
fn highlight_move_transfers_only_the_highlight_path() {
    let mut app = DemoApp::new();
    let now = 1_000_000u64;
    let mut dirty = Dirty::new(SCREEN);
    app.event(&Event::Next, now, &mut dirty);
    // One frame while the bar is moving: both the old and the new row are dirty.
    let mut moved = Dirty::new(SCREEN);
    app.tick(now + 16_000, &mut moved);
    assert!(!moved.is_empty());

    let mut frame = vec![0u8; 240 * 320 * 2];
    let mut band_a = vec![0u8; BAND_LEN];
    let mut band_b = vec![0u8; BAND_LEN];
    let cfg = RenderCfg::new(SCREEN, BAND_H);
    let stats = {
        let mut sink = utsuroui::band::MemorySink::new(&mut frame, 240);
        pollster::block_on(flush(
            &app,
            &mut sink,
            &moved,
            now + 16_000,
            &mut band_a,
            Some(&mut band_b),
            &cfg,
        ))
        .unwrap()
    };
    // Row 0 + row 1 with the 8 px side margins: 224 x 56 px x 2 bytes = 25,088.
    assert!(
        stats.bytes <= 32 * 1024,
        "highlight move should stay under 32 KiB, got {} bytes in {} bands",
        stats.bytes,
        stats.bands
    );
    assert!(stats.bytes >= 20 * 1024, "expected a meaningful dirty area");
    assert!(
        stats.bands <= 4,
        "expected a handful of bands, got {}",
        stats.bands
    );
}

#[test]
fn full_redraw_is_one_screen() {
    let app = DemoApp::new();
    let mut dirty = Dirty::new(SCREEN);
    dirty.mark_full();
    let mut band_a = vec![0u8; BAND_LEN];
    let mut frame = vec![0u8; 240 * 320 * 2];
    let cfg = RenderCfg::new(SCREEN, BAND_H);
    let stats = {
        let mut sink = utsuroui::band::MemorySink::new(&mut frame, 240);
        pollster::block_on(flush(&app, &mut sink, &dirty, 0, &mut band_a, None, &cfg)).unwrap()
    };
    assert_eq!(stats.bands, 20);
    assert_eq!(stats.bytes, 240 * 320 * 2);
}

/// A minimal app used to prove the renderer works on arbitrary screens too.
struct Solid;
impl App for Solid {
    fn event(&mut self, _e: &Event, _n: u64, _d: &mut DirtyList) {}
    fn tick(&mut self, _n: u64, _d: &mut DirtyList) -> bool {
        false
    }
    fn paint(&self, cx: &mut utsuroui::PaintCx<'_>, _n: u64) {
        cx.fill_rect(Rect::new(0, 0, 240, 320), BLACK);
        cx.fill_rect(Rect::new(8, 44, 224, 28), WHITE);
    }
}

#[test]
fn arbitrary_app_renders() {
    let mut frame = vec![0u8; 240 * 320 * 2];
    let mut band = vec![0u8; BAND_LEN];
    let mut dirty = Dirty::new(SCREEN);
    dirty.add(Rect::new(0, 0, 240, 80));
    let cfg = RenderCfg::new(SCREEN, BAND_H);
    let stats = {
        let mut sink = utsuroui::band::MemorySink::new(&mut frame, 240);
        pollster::block_on(flush(&Solid, &mut sink, &dirty, 0, &mut band, None, &cfg)).unwrap()
    };
    assert_eq!(stats.bytes, 240 * 80 * 2);
}
