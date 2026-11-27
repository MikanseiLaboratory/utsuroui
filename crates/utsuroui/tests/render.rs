//! End-to-end renderer tests without a panel.
//!
//! These exercise the real `App -> DirtyList -> band buffer -> BandSink` path
//! with a `MemorySink`, so the byte order, clipping, band splitting and
//! crossfade blending are all covered on the host.

use utsuroui::app::App;
use utsuroui::band::{MemorySink, read_px};
use utsuroui::color::{BLACK, WHITE, YELLOW, blend};
use utsuroui::dirty::{Dirty, DirtyList};
use utsuroui::event::Event;
use utsuroui::flush::{RenderCfg, flush};
use utsuroui::geom::{Point, Rect, Size};
use utsuroui::paint::PaintCx;
use utsuroui::text::TextStyle;

const W: u16 = 240;
const H: u16 = 320;
const SCREEN: Rect = Rect::new(0, 0, W, H);
const BAND: u16 = 16;
const BAND_LEN: usize = W as usize * BAND as usize * 2;

struct SolidApp {
    color: utsuroui::color::Rgb565,
    under: utsuroui::color::Rgb565,
    alpha: Option<u8>,
}

impl SolidApp {
    fn new(color: utsuroui::color::Rgb565) -> Self {
        Self {
            color,
            under: BLACK,
            alpha: None,
        }
    }
}

impl App for SolidApp {
    fn event(&mut self, _event: &Event, _now_us: u64, _dirty: &mut DirtyList) {}

    fn tick(&mut self, _now_us: u64, _dirty: &mut DirtyList) -> bool {
        false
    }

    fn paint(&self, cx: &mut PaintCx<'_>, _now_us: u64) {
        cx.fill_rect(SCREEN, self.color);
    }

    fn paint_under(&self, cx: &mut PaintCx<'_>, _now_us: u64) -> bool {
        cx.fill_rect(SCREEN, self.under);
        true
    }

    fn crossfade_alpha(&self, _now_us: u64) -> Option<u8> {
        self.alpha
    }
}

struct TextApp;

impl App for TextApp {
    fn event(&mut self, _event: &Event, _now_us: u64, _dirty: &mut DirtyList) {}

    fn tick(&mut self, _now_us: u64, _dirty: &mut DirtyList) -> bool {
        false
    }

    fn paint(&self, cx: &mut PaintCx<'_>, _now_us: u64) {
        cx.fill_rect(SCREEN, BLACK);
        cx.text("31.2 ms", Point::new(8, 8), TextStyle::Medium, YELLOW);
        cx.stroke_rect(Rect::new(8, 40, 100, 20), WHITE);
    }
}

fn run<A: App>(
    app: &A,
    dirty: &DirtyList,
    now: u64,
    band_b: bool,
) -> (Vec<u8>, utsuroui::flush::FlushStats) {
    let mut frame = vec![0u8; W as usize * H as usize * 2];
    let mut buf_a = vec![0u8; BAND_LEN];
    let mut buf_b = vec![0u8; BAND_LEN];
    let cfg = RenderCfg::new(SCREEN, BAND);
    let stats = {
        let mut sink = MemorySink::new(&mut frame, W);
        pollster::block_on(flush(
            app,
            &mut sink,
            dirty,
            now,
            &mut buf_a,
            if band_b { Some(&mut buf_b) } else { None },
            &cfg,
        ))
        .unwrap()
    };
    (frame, stats)
}

fn px(frame: &[u8], x: u16, y: u16) -> utsuroui::color::Rgb565 {
    read_px(frame, (y as usize * W as usize + x as usize) * 2)
}

#[test]
fn empty_dirty_paints_nothing() {
    let app = SolidApp::new(WHITE);
    let dirty = Dirty::new(SCREEN);
    let (frame, stats) = run(&app, &dirty, 0, false);
    assert_eq!(stats.bands, 0);
    assert_eq!(stats.bytes, 0);
    assert!(frame.iter().all(|b| *b == 0));
}

#[test]
fn full_dirty_paints_every_pixel() {
    let app = SolidApp::new(YELLOW);
    let mut dirty = Dirty::new(SCREEN);
    dirty.mark_full();
    let (frame, stats) = run(&app, &dirty, 0, false);
    assert_eq!(stats.rects, 1);
    assert_eq!(stats.bands, 20); // 320 / 16
    assert_eq!(stats.bytes, W as u32 * H as u32 * 2);
    assert_eq!(px(&frame, 0, 0), YELLOW);
    assert_eq!(px(&frame, W - 1, H - 1), YELLOW);
    // Big-endian bytes on the wire.
    assert_eq!(&frame[0..2], &[0xFF, 0xE0]);
}

#[test]
fn partial_dirty_only_sends_that_rectangle() {
    let app = SolidApp::new(WHITE);
    let mut dirty = Dirty::new(SCREEN);
    dirty.add(Rect::new(10, 20, 30, 8));
    let (frame, stats) = run(&app, &dirty, 0, false);
    assert_eq!(stats.rects, 1);
    assert_eq!(stats.bands, 1);
    assert_eq!(stats.bytes, 30 * 8 * 2);
    assert_eq!(px(&frame, 10, 20), WHITE);
    assert_eq!(px(&frame, 39, 27), WHITE);
    assert_eq!(px(&frame, 9, 20), BLACK);
    assert_eq!(px(&frame, 40, 27), BLACK);
    assert_eq!(px(&frame, 10, 28), BLACK);
}

#[test]
fn bands_split_on_the_band_height() {
    let app = SolidApp::new(WHITE);
    let mut dirty = Dirty::new(SCREEN);
    dirty.add(Rect::new(0, 5, 240, 40));
    let (_, stats) = run(&app, &dirty, 0, false);
    assert_eq!(stats.bands, 3); // 16 + 16 + 8
    assert_eq!(stats.bytes, 240 * 40 * 2);
}

#[test]
fn text_and_outline_are_visible() {
    let app = TextApp;
    let mut dirty = Dirty::new(SCREEN);
    dirty.mark_full();
    let (frame, _) = run(&app, &dirty, 0, false);
    // Some glyph pixels are yellow inside the text area.
    let text_area = Rect::new(8, 8, 60, 13);
    let mut yellow = 0;
    for y in text_area.y..text_area.y + text_area.h as i16 {
        for x in text_area.x..text_area.x + text_area.w as i16 {
            if px(&frame, x as u16, y as u16) == YELLOW {
                yellow += 1;
            }
        }
    }
    assert!(yellow > 10, "expected glyph pixels, got {yellow}");
    // The 1 px outline is drawn inside the rectangle.
    assert_eq!(px(&frame, 8, 40), WHITE);
    assert_eq!(px(&frame, 107, 40), WHITE);
    assert_eq!(px(&frame, 8, 59), WHITE);
    assert_eq!(px(&frame, 60, 50), BLACK);
}

#[test]
fn crossfade_blends_under_and_over_layers() {
    let mut app = SolidApp::new(BLACK);
    app.under = WHITE;
    app.alpha = Some(128);
    let mut dirty = Dirty::new(SCREEN);
    dirty.mark_full();
    let (frame, stats) = run(&app, &dirty, 0, true);
    assert_eq!(stats.bands, 20);
    let expected = blend(BLACK, WHITE, 128);
    assert_eq!(px(&frame, 0, 0), expected);
    assert_eq!(px(&frame, 100, 200), expected);
}

#[test]
fn crossfade_without_scratch_buffer_falls_back() {
    let mut app = SolidApp::new(BLACK);
    app.under = WHITE;
    app.alpha = Some(128);
    let mut dirty = Dirty::new(SCREEN);
    dirty.mark_full();
    let (frame, _) = run(&app, &dirty, 0, false);
    assert_eq!(px(&frame, 0, 0), BLACK);
}

#[test]
fn zero_alpha_crossfade_shows_the_outgoing_screen() {
    let mut app = SolidApp::new(BLACK);
    app.under = WHITE;
    app.alpha = Some(0);
    let mut dirty = Dirty::new(SCREEN);
    dirty.mark_full();
    let (frame, _) = run(&app, &dirty, 0, true);
    assert_eq!(px(&frame, 0, 0), WHITE);
}

#[test]
fn text_uses_top_baseline_and_stays_inside_the_clip() {
    struct TextApp2;
    impl App for TextApp2 {
        fn event(&mut self, _e: &Event, _n: u64, _d: &mut DirtyList) {}
        fn tick(&mut self, _n: u64, _d: &mut DirtyList) -> bool {
            false
        }
        fn paint(&self, cx: &mut PaintCx<'_>, _n: u64) {
            cx.fill_rect(SCREEN, BLACK);
            cx.text("WWWW", Point::new(0, 1), TextStyle::Large, WHITE);
        }
    }
    let app = TextApp2;
    let mut dirty = Dirty::new(SCREEN);
    dirty.add(Rect::new(0, 0, 40, 20));
    let (frame, stats) = run(&app, &dirty, 0, false);
    assert_eq!(stats.bands, 2);
    // Row 0 is untouched because the text starts at y=1.
    assert_eq!(px(&frame, 5, 0), BLACK);
    // The clip stops the glyphs at x=40 even though the text is 40 px wide.
    let mut white = 0;
    for y in 1..20 {
        for x in 0..40 {
            if px(&frame, x, y) == WHITE {
                white += 1;
            }
        }
    }
    assert!(white > 0);
}

#[cfg(feature = "full-fb")]
#[test]
fn full_frame_path_matches_band_path() {
    use utsuroui::flush::{flush_full, render_full};

    let app = TextApp;
    let cfg = RenderCfg::new(SCREEN, BAND);
    let mut full = vec![0u8; W as usize * H as usize * 2];
    render_full(&app, &mut full, &cfg, 0);

    let mut dirty = Dirty::new(SCREEN);
    dirty.mark_full();
    let (band_frame, _) = run(&app, &dirty, 0, false);
    assert_eq!(full, band_frame);

    let mut sink_frame = vec![0u8; W as usize * H as usize * 2];
    let stats = {
        let mut sink = MemorySink::new(&mut sink_frame, W);
        pollster::block_on(flush_full(&mut sink, &full, &cfg)).unwrap()
    };
    assert_eq!(stats.bands, 20);
    assert_eq!(stats.bytes, W as u32 * H as u32 * 2);
    assert_eq!(sink_frame, full);
}

#[test]
fn size_constants_stay_consistent() {
    assert_eq!(SCREEN.size_hint(), (W, H));
    assert_eq!(Size::new(W, H).area(), W as u32 * H as u32);
}

trait SizeHint {
    fn size_hint(&self) -> (u16, u16);
}

impl SizeHint for Rect {
    fn size_hint(&self) -> (u16, u16) {
        (self.w, self.h)
    }
}
