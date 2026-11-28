//! Optional U8g2 backend smoke test (`--features u8g2`).
//!
//! The demo and the firmware deliberately use only the `embedded-graphics`
//! fonts; this test proves the optional backend draws through the same band
//! pipeline.

#![cfg(feature = "u8g2")]

use utsuroui::app::App;
use utsuroui::band::MemorySink;
use utsuroui::dirty::{Dirty, DirtyList};
use utsuroui::event::Event;
use utsuroui::flush::{RenderCfg, flush};
use utsuroui::geom::Rect;
use utsuroui::paint::PaintCx;
use utsuroui::u8g2::{Baseline, U8g2TextStyle, fonts};
use utsuroui::{BLACK, WHITE};

const W: u16 = 240;
const H: u16 = 320;

struct TextApp;

impl App for TextApp {
    fn event(&mut self, _event: &Event, _now_us: u64, _dirty: &mut DirtyList) {}

    fn tick(&mut self, _now_us: u64, _dirty: &mut DirtyList) -> bool {
        false
    }

    fn paint(&self, cx: &mut PaintCx<'_>, _now_us: u64) {
        cx.fill_rect(Rect::new(0, 0, W, H), BLACK);
        let style = U8g2TextStyle::new(fonts::u8g2_font_6x10_tf, WHITE);
        cx.text_with("utsuroui", utsuroui::Point::new(8, 8), style, Baseline::Top);
    }
}

#[test]
fn u8g2_text_draws_pixels() {
    let app = TextApp;
    let mut frame = vec![0u8; W as usize * H as usize * 2];
    let mut buffers = utsuroui::BandBuffers::<{ W as usize * 16 * 2 }>::new();
    let mut dirty = Dirty::new(Rect::new(0, 0, W, H));
    dirty.mark_full();
    let cfg = RenderCfg::new(Rect::new(0, 0, W, H), 16);
    {
        let mut sink = MemorySink::new(&mut frame, W);
        pollster::block_on(flush(&app, &mut sink, &dirty, 0, &mut buffers, &cfg)).unwrap();
    }
    let non_black = frame.chunks_exact(2).filter(|px| px != &[0, 0]).count();
    assert!(
        non_black > 20,
        "expected U8g2 glyph pixels, got {non_black}"
    );
}
