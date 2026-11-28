//! Four full-screen animation scenes, switchable with the front panel keys.
//!
//! | Scene | What it shows |
//! |---|---|
//! | `Waveform` | a scrolling audio trace, redrawn as one dirty band |
//! | `Ticker` | a horizontal marquee and a vertically scrolling log |
//! | `Float` | five rings drifting on sine paths — only their boxes are dirty |
//! | `Spectrum` | an equaliser whose bars repaint only when their height changes |
//!
//! Every scene draws **only inside the rectangle it marks dirty** (each paint is
//! clipped to its own area). A full redraw and an incremental redraw therefore
//! produce the same pixels; if a scene painted outside its dirty rect the two
//! would disagree and the panel would keep stale fragments.

use utsuroui::anim::{Orbit, wave};
use utsuroui::app::App;
use utsuroui::color::{BLACK, DARK_GRAY, GRAY, Rgb565, WHITE, YELLOW};
use utsuroui::dirty::DirtyList;
use utsuroui::event::Event;
use utsuroui::geom::{Point, Rect};
use utsuroui::paint::PaintCx;
use utsuroui::text::{FORMAT_BUF, TextStyle, format_u16, measure};

/// Screen width in pixels.
pub const W: u16 = 240;
/// Screen height in pixels.
pub const H: u16 = 240;

const HEADER_H: u16 = 34;
const FOOTER_Y: i16 = 220;

const GRID: Rgb565 = DARK_GRAY;
const CYAN: Rgb565 = Rgb565::new(0, 42, 30);
const MAGENTA: Rgb565 = Rgb565::new(28, 8, 26);
const LIME: Rgb565 = Rgb565::new(12, 60, 10);

/// Names shown in the header, in scene order.
pub const SCENE_NAMES: [&str; 4] = ["Waveform", "Ticker", "Float", "Spectrum"];

// --- shared math -----------------------------------------------------------

/// Integer square root (floor), for circle spans.
fn isqrt(value: i32) -> i16 {
    if value <= 0 {
        return 0;
    }
    let mut x = value;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + value / x) / 2;
    }
    x as i16
}

fn fill_circle(cx: &mut PaintCx<'_>, center: Point, radius: i16, color: Rgb565) {
    if radius <= 0 {
        return;
    }
    for dy in -radius..=radius {
        let rest = radius * radius - dy * dy;
        let dx = isqrt(rest as i32);
        cx.hline(center.y + dy, center.x - dx, center.x + dx, color);
    }
}

/// One screen of the showcase.
#[allow(clippy::large_enum_variant)]
enum Scene {
    Wave(Wave),
    Ticker(Ticker),
    Float(Float),
    Spectrum(Spectrum),
}

impl Scene {
    fn new(index: usize) -> Self {
        match index {
            0 => Self::Wave(Wave::new()),
            1 => Self::Ticker(Ticker),
            2 => Self::Float(Float::new()),
            _ => Self::Spectrum(Spectrum::new()),
        }
    }

    fn tick(&mut self, now_us: u64, dirty: &mut DirtyList) -> bool {
        match self {
            Self::Wave(scene) => scene.tick(dirty),
            Self::Ticker(scene) => scene.tick(dirty),
            Self::Float(scene) => scene.tick(now_us, dirty),
            Self::Spectrum(scene) => scene.tick(dirty),
        }
    }

    fn paint(&self, cx: &mut PaintCx<'_>, now_us: u64) {
        match self {
            Self::Wave(scene) => scene.paint(cx, now_us),
            Self::Ticker(scene) => scene.paint(cx, now_us),
            Self::Float(scene) => scene.paint(cx, now_us),
            Self::Spectrum(scene) => scene.paint(cx, now_us),
        }
    }
}

// --- waveform --------------------------------------------------------------

const WAVE_COLS: usize = 240;
const WAVE_TOP: i16 = 107;
const WAVE_H: u16 = 104;

struct Wave {
    history: [i16; WAVE_COLS],
    head: usize,
    step: u32,
}

impl Wave {
    fn new() -> Self {
        Self {
            history: [0; WAVE_COLS],
            head: 0,
            step: 0,
        }
    }

    fn at(&self, index: usize) -> i16 {
        self.history[(self.head + index) % WAVE_COLS]
    }

    fn push(&mut self, sample: i16) {
        self.history[self.head] = sample;
        self.head = (self.head + 1) % WAVE_COLS;
    }

    /// A synthetic signal: a slow swell, a faster partial and broadband noise.
    fn synth(&mut self) -> i16 {
        self.step = self.step.wrapping_add(1);
        let t = self.step;
        let slow = wave(t.wrapping_mul(140) as u16) as i32 / 3;
        let fast = wave(t.wrapping_mul(1_090) as u16) as i32 / 4;
        let noise =
            (((t.wrapping_mul(1_103_515_245).wrapping_add(12_345)) >> 16) & 0x1FF) as i32 - 256;
        (slow + fast + noise).clamp(-4_096, 4_096) as i16
    }

    fn tick(&mut self, dirty: &mut DirtyList) -> bool {
        // Three columns per frame: ~180 px per second at 60 fps.
        for _ in 0..3 {
            let sample = self.synth();
            self.push(sample);
        }
        dirty.add(Rect::new(0, WAVE_TOP, W, WAVE_H));
        true
    }

    fn paint(&self, cx: &mut PaintCx<'_>, now_us: u64) {
        let area = Rect::new(0, WAVE_TOP, W, WAVE_H);
        cx.with_clip(area, |cx| {
            cx.fill_rect(area, BLACK);

            let mut y = WAVE_TOP;
            while y < WAVE_TOP + WAVE_H as i16 {
                cx.hline(y, 0, W as i16 - 1, GRID);
                y += 29;
            }

            let mid = WAVE_TOP + (WAVE_H / 2) as i16;
            cx.hline(mid, 0, W as i16 - 1, GRAY);

            let amp = (WAVE_H as i32 / 2) - 6;

            // Mirrored ghost trace first, so the main trace sits on top.
            let mut previous = mid;
            for x in 0..WAVE_COLS as i16 {
                let value = -(self.at(x as usize) as i32);
                let y = mid - ((value * amp) / 4_096) as i16;
                let (top, bottom) = if previous <= y {
                    (previous, y)
                } else {
                    (y, previous)
                };
                cx.vline(x, top, bottom, GRID);
                previous = y;
            }

            let mut previous = mid;
            for x in 0..WAVE_COLS as i16 {
                let value = self.at(x as usize) as i32;
                let y = mid - ((value * amp) / 4_096) as i16;
                let (top, bottom) = if previous <= y {
                    (previous, y)
                } else {
                    (y, previous)
                };
                cx.vline(x, top, bottom, YELLOW);
                previous = y;
            }

            let marker = ((now_us / 9_000) % W as u64) as i16;
            cx.vline(marker, WAVE_TOP, WAVE_TOP + WAVE_H as i16 - 1, CYAN);
        });
    }
}

// --- ticker ----------------------------------------------------------------

const MARQUEE_A_Y: i16 = 84;
const MARQUEE_A_H: u16 = 18;
const MARQUEE_B_Y: i16 = 108;
const MARQUEE_B_H: u16 = 18;
const FEED_TOP: i16 = 138;
const FEED_H: u16 = 66;
const FEED_ROW: i16 = 22;

const TEXT_A: &str = "* DIRTY-RECT ANIMATION SHOWCASE * ";
const TEXT_B: &str = "EMBEDDED-GRAPHICS + EMBASSY + RP2040 + 240x240 * ";

const FEED: [&str; 10] = [
    "001  band flush, 16 rows",
    "002  tween on one scalar",
    "003  dirty rects merged",
    "004  no heap, no alloc",
    "005  crossfade without fb",
    "006  partial redraw only",
    "007  integer easing LUT",
    "008  embassy async task",
    "009  core1 keeps sampling",
    "010  idle means no SPI",
];

struct Ticker;

impl Ticker {
    fn tick(&self, dirty: &mut DirtyList) -> bool {
        dirty.add(Rect::new(0, MARQUEE_A_Y, W, MARQUEE_A_H));
        dirty.add(Rect::new(0, MARQUEE_B_Y, W, MARQUEE_B_H));
        dirty.add(Rect::new(0, FEED_TOP, W, FEED_H));
        true
    }

    fn paint(&self, cx: &mut PaintCx<'_>, now_us: u64) {
        cx.with_clip(Rect::new(0, MARQUEE_A_Y, W, MARQUEE_A_H), |cx| {
            self.marquee(
                cx,
                now_us,
                TEXT_A,
                MARQUEE_A_Y,
                70,
                TextStyle::Medium,
                YELLOW,
            );
        });
        cx.with_clip(Rect::new(0, MARQUEE_B_Y, W, MARQUEE_B_H), |cx| {
            self.marquee(cx, now_us, TEXT_B, MARQUEE_B_Y, 110, TextStyle::Small, CYAN);
        });
        cx.with_clip(Rect::new(0, FEED_TOP, W, FEED_H), |cx| {
            self.feed(cx, now_us);
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn marquee(
        &self,
        cx: &mut PaintCx<'_>,
        now_us: u64,
        text: &str,
        y: i16,
        speed: i32,
        style: TextStyle,
        color: Rgb565,
    ) {
        cx.fill_rect(Rect::new(0, y, W, style.line_height() + 3), BLACK);

        let width = measure(text, style).w as i32;
        let span = width + W as i32;
        let travelled = ((now_us / 1_000) as i32 * speed) / 1_000;
        let x = W as i32 - (travelled % span);

        cx.text(text, Point::new(x as i16, y), style, color);
        if x + width < W as i32 {
            cx.text(text, Point::new((x + span) as i16, y), style, color);
        }
    }

    fn feed(&self, cx: &mut PaintCx<'_>, now_us: u64) {
        cx.fill_rect(Rect::new(0, FEED_TOP, W, FEED_H), BLACK);

        let scrolled = ((now_us / 1_000) as i32 * 34) / 1_000;
        let first_row = scrolled / FEED_ROW as i32;
        let offset = scrolled % FEED_ROW as i32;
        let rows = FEED_H as i32 / FEED_ROW as i32 + 2;

        let mut y = FEED_TOP - offset as i16;
        for row in 0..rows {
            let index = (first_row + row).rem_euclid(FEED.len() as i32) as usize;
            if (first_row + row) % 2 == 0 {
                cx.fill_rect(Rect::new(0, y, W, FEED_ROW as u16 - 1), GRID);
            }
            cx.text(FEED[index], Point::new(8, y + 5), TextStyle::Small, WHITE);
            y += FEED_ROW;
        }
    }
}

// --- floating rings --------------------------------------------------------

const BLOBS: usize = 5;
const BLOB_TOP: i16 = 44;
const BLOB_BOTTOM: i16 = 212;

struct Blob {
    base: Point,
    amplitude: (i16, i16),
    period_us: u32,
    phase: u16,
    radius: i16,
    color: Rgb565,
    previous: Rect,
}

impl Blob {
    fn position(&self, now_us: u64) -> Point {
        Orbit::new(self.base, self.amplitude, self.period_us, self.phase).position(now_us)
    }

    fn bounds(&self, center: Point) -> Rect {
        let d = self.radius * 2 + 2;
        Rect::new(
            center.x - self.radius - 1,
            center.y - self.radius - 1,
            d as u16,
            d as u16,
        )
    }
}

struct Float {
    blobs: [Blob; BLOBS],
}

impl Float {
    fn new() -> Self {
        let blobs = [
            Blob {
                base: Point::new(56, 74),
                amplitude: (12, 10),
                period_us: 3_100_000,
                phase: 0,
                radius: 16,
                color: CYAN,
                previous: Rect::EMPTY,
            },
            Blob {
                base: Point::new(184, 74),
                amplitude: (12, 10),
                period_us: 4_300_000,
                phase: 12_000,
                radius: 16,
                color: MAGENTA,
                previous: Rect::EMPTY,
            },
            Blob {
                base: Point::new(56, 130),
                amplitude: (12, 10),
                period_us: 5_100_000,
                phase: 30_000,
                radius: 16,
                color: LIME,
                previous: Rect::EMPTY,
            },
            Blob {
                base: Point::new(184, 130),
                amplitude: (12, 10),
                period_us: 3_700_000,
                phase: 50_000,
                radius: 14,
                color: YELLOW,
                previous: Rect::EMPTY,
            },
            Blob {
                base: Point::new(120, 186),
                amplitude: (14, 10),
                period_us: 4_900_000,
                phase: 20_000,
                radius: 16,
                color: WHITE,
                previous: Rect::EMPTY,
            },
        ];
        Self { blobs }
    }

    fn tick(&mut self, now_us: u64, dirty: &mut DirtyList) -> bool {
        for blob in &mut self.blobs {
            let bounds = blob.bounds(blob.position(now_us));
            dirty.add(blob.previous.union(bounds));
            blob.previous = bounds;
        }
        true
    }

    fn paint(&self, cx: &mut PaintCx<'_>, now_us: u64) {
        let area = Rect::new(0, BLOB_TOP, W, (BLOB_BOTTOM - BLOB_TOP) as u16);
        cx.with_clip(area, |cx| {
            cx.fill_rect(area, BLACK);
            for blob in &self.blobs {
                let center = blob.position(now_us);
                fill_circle(cx, center, blob.radius, blob.color);
                fill_circle(cx, center, blob.radius - 4, BLACK);
                fill_circle(cx, center, blob.radius - 8, blob.color);
            }
        });
    }
}

// --- spectrum --------------------------------------------------------------

const BARS: usize = 20;
const BAR_W: i16 = 10;
const BAR_GAP: i16 = 2;
const BAR_BASE: i16 = 210;
const BAR_TOP: i16 = 84;
const BAR_MAX: i16 = BAR_BASE - BAR_TOP;

struct Spectrum {
    value: [i16; BARS],
    target: [i16; BARS],
    shown: [i16; BARS],
    seed: u32,
    frames: u32,
}

impl Spectrum {
    fn new() -> Self {
        Self {
            value: [10; BARS],
            target: [10; BARS],
            shown: [0; BARS],
            seed: 0x1234_5678,
            frames: 0,
        }
    }

    fn random(&mut self) -> u32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        self.seed
    }

    fn height(percent: i16) -> i16 {
        // Two-pixel steps keep the dirty rectangles small.
        (percent.clamp(2, 100) * BAR_MAX / 100) & !1
    }

    fn tick(&mut self, dirty: &mut DirtyList) -> bool {
        self.frames += 1;
        if self.frames % 7 == 0 {
            for bar in 0..BARS {
                let r = self.random();
                let noise = (r % 81) as i16 - 40;
                // Mean-reverting walk so the bars hover around half scale
                // instead of saturating at the top.
                let pull = (self.target[bar] - 50) / 6;
                let next = self.target[bar] + noise - pull;
                self.target[bar] = next.clamp(6, 100);
            }
        }

        for bar in 0..BARS {
            let target = self.target[bar];
            let value = self.value[bar];
            if value < target {
                self.value[bar] = value + 4;
            } else if value > target {
                self.value[bar] = value - 2;
            }

            let height = Self::height(self.value[bar]);
            let previous = self.shown[bar];
            if height != previous {
                let tallest = height.max(previous);
                let x = 1 + bar as i16 * (BAR_W + BAR_GAP);
                dirty.add(Rect::new(
                    x,
                    BAR_BASE - tallest,
                    BAR_W as u16,
                    tallest as u16,
                ));
                self.shown[bar] = height;
            }
        }
        true
    }

    fn paint(&self, cx: &mut PaintCx<'_>, _now_us: u64) {
        let area = Rect::new(0, BAR_TOP, W, (BAR_BASE - BAR_TOP + 2) as u16);
        cx.with_clip(area, |cx| {
            cx.fill_rect(area, BLACK);

            for bar in 0..BARS {
                let height = self.shown[bar];
                if height <= 0 {
                    continue;
                }
                let x = 1 + bar as i16 * (BAR_W + BAR_GAP);
                let top = BAR_BASE - height;
                let low = height.min(12);
                let mid = (height - low).min(24);

                cx.fill_rect(Rect::new(x, BAR_BASE - low, BAR_W as u16, low as u16), CYAN);
                if mid > 0 {
                    cx.fill_rect(
                        Rect::new(x, BAR_BASE - low - mid, BAR_W as u16, mid as u16),
                        LIME,
                    );
                }
                let high = height - low - mid;
                if high > 0 {
                    cx.fill_rect(Rect::new(x, top, BAR_W as u16, high as u16), YELLOW);
                }
            }
        });
    }
}

// --- application -----------------------------------------------------------

/// The showcase application: one scene at a time, switched with the keys.
pub struct Showcase {
    index: usize,
    scene: Scene,
    paused: bool,
}

impl Showcase {
    /// Starts on the first scene.
    pub fn new() -> Self {
        Self {
            index: 0,
            scene: Scene::new(0),
            paused: false,
        }
    }

    fn switch(&mut self, delta: i32, dirty: &mut DirtyList) {
        let count = SCENE_NAMES.len() as i32;
        self.index = ((self.index as i32 + delta).rem_euclid(count)) as usize;
        self.scene = Scene::new(self.index);
        self.paused = false;
        dirty.mark_full();
    }

    fn chrome(&self, cx: &mut PaintCx<'_>) {
        cx.with_clip(Rect::new(0, 0, W, HEADER_H), |cx| {
            cx.fill_rect(Rect::new(0, 0, W, HEADER_H), BLACK);
            cx.text(
                SCENE_NAMES[self.index],
                Point::new(8, 6),
                TextStyle::Large,
                YELLOW,
            );

            let mut buf = [0u8; FORMAT_BUF];
            let index = format_u16(self.index as u16 + 1, &mut buf);
            let suffix_w = measure("/4", TextStyle::Medium).w as i16;
            cx.text_right("/4", W as i16 - 8, 12, TextStyle::Medium, GRAY);
            cx.text_right(
                index,
                W as i16 - 10 - suffix_w,
                12,
                TextStyle::Medium,
                WHITE,
            );
            cx.hline(HEADER_H as i16 - 1, 0, W as i16 - 1, GRID);
        });

        cx.with_clip(Rect::new(0, FOOTER_Y - 4, W, 28), |cx| {
            cx.fill_rect(Rect::new(0, FOOTER_Y - 6, W, 26), BLACK);
            if self.paused {
                cx.text(
                    "PAUSED",
                    Point::new(8, FOOTER_Y - 2),
                    TextStyle::Small,
                    YELLOW,
                );
            } else {
                cx.text(
                    "MENU/PREV: scene  INPUT1: pause",
                    Point::new(8, FOOTER_Y - 2),
                    TextStyle::Small,
                    WHITE,
                );
            }
            cx.text(
                "BACK: restart scene",
                Point::new(8, FOOTER_Y + 8),
                TextStyle::Small,
                GRAY,
            );
        });
    }
}

impl App for Showcase {
    fn event(&mut self, event: &Event, _now_us: u64, dirty: &mut DirtyList) {
        match event {
            Event::Next => self.switch(1, dirty),
            Event::Previous => self.switch(-1, dirty),
            Event::Back => self.switch(0, dirty),
            Event::Select | Event::LongPress => {
                self.paused = !self.paused;
                dirty.mark_full();
            }
            Event::Repeat | Event::Encoder(_) | Event::Touch { .. } => {}
        }
    }

    fn tick(&mut self, now_us: u64, dirty: &mut DirtyList) -> bool {
        if self.paused {
            return false;
        }
        self.scene.tick(now_us, dirty)
    }

    fn paint(&self, cx: &mut PaintCx<'_>, now_us: u64) {
        cx.fill_rect(Rect::new(0, 0, W, H), BLACK);
        self.chrome(cx);
        self.scene.paint(cx, now_us);
    }
}
