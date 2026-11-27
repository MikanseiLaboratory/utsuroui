//! The live measurement screen: a big readout, min/avg/max and a bar graph.

use utsuroui::anim::Tween;
use utsuroui::geom::{Point, Rect};
use utsuroui::paint::PaintCx;
use utsuroui::text::{FORMAT_BUF, TextStyle, format_x10, measure};

use crate::style::{ACCENT, DIM, FG, GRAPH_RECT, READOUT_TWEEN_US, STATS_RECT};

/// Number of samples kept for the graph.
pub const SAMPLES: usize = 32;

const SEED: [u16; SAMPLES] = [
    280, 292, 305, 318, 300, 288, 312, 330, 345, 320, 300, 295, 310, 325, 340, 332, 318, 305, 290,
    300, 315, 330, 342, 335, 322, 308, 298, 305, 318, 328, 336, 312,
];

/// Retained state of the measurement screen.
#[derive(Clone, Copy, Debug)]
pub struct MeasureState {
    samples: [u16; SAMPLES],
    head: usize,
    len: usize,
    /// Latest raw value in tenths of a millisecond.
    pub value_x10: u16,
    /// Animated value shown by the big readout.
    pub shown_x10: Tween,
    /// Minimum/mean/maximum over the retained window.
    pub min_x10: u16,
    /// Mean over the retained window.
    pub avg_x10: u16,
    /// Maximum over the retained window.
    pub max_x10: u16,
    /// Whether new samples are ignored (frozen display).
    pub frozen: bool,
}

impl Default for MeasureState {
    fn default() -> Self {
        let mut state = Self {
            samples: [0; SAMPLES],
            head: 0,
            len: 0,
            value_x10: SEED[SEED.len() - 1],
            shown_x10: Tween::settled(SEED[SEED.len() - 1] as i32),
            min_x10: 0,
            avg_x10: 0,
            max_x10: 0,
            frozen: false,
        };
        state.samples = SEED;
        state.len = SAMPLES;
        state.head = 0;
        state.recompute_stats();
        state
    }
}

impl MeasureState {
    /// Adds a sample to the rolling window (ignored while frozen).
    pub fn push(&mut self, value_x10: u16, now_us: u64) -> bool {
        if self.frozen {
            return false;
        }
        self.value_x10 = value_x10;
        self.samples[self.head] = value_x10;
        self.head = (self.head + 1) % SAMPLES;
        if self.len < SAMPLES {
            self.len += 1;
        }
        self.recompute_stats();
        self.shown_x10
            .retarget(now_us, value_x10 as i32, READOUT_TWEEN_US);
        true
    }

    /// Advances the readout tween.
    pub fn tick(&mut self, now_us: u64) -> bool {
        self.shown_x10.is_running(now_us)
    }

    /// Rectangles that change when a sample arrives.
    pub fn dirty_rect(&self) -> Rect {
        // The graph area and the stats block are the only parts that change;
        // the footer readout is handled by the app.
        STATS_RECT.union(GRAPH_RECT)
    }

    /// Samples oldest to newest.
    pub fn samples(&self) -> impl Iterator<Item = u16> + '_ {
        let len = self.len;
        let head = self.head;
        (0..len).map(move |i| {
            let start = (head + SAMPLES - len) % SAMPLES;
            self.samples[(start + i) % SAMPLES]
        })
    }

    fn recompute_stats(&mut self) {
        if self.len == 0 {
            self.min_x10 = 0;
            self.avg_x10 = 0;
            self.max_x10 = 0;
            return;
        }
        let mut min = u16::MAX;
        let mut max = 0u16;
        let mut sum = 0u32;
        for sample in self.samples() {
            min = min.min(sample);
            max = max.max(sample);
            sum += sample as u32;
        }
        self.min_x10 = min;
        self.avg_x10 = (sum / self.len as u32) as u16;
        self.max_x10 = max;
    }

    /// Paints the measurement screen body.
    pub fn paint(&self, cx: &mut PaintCx<'_>, origin: Point, now_us: u64, scale_x10: u16) {
        let big = self.shown_x10.value_at(now_us).clamp(0, u16::MAX as i32) as u16;
        let mut buf = [0u8; FORMAT_BUF];
        let text = format_x10(big, &mut buf);
        let pos = Point::new(origin.x + 16, origin.y + 46);
        cx.text(text, pos, TextStyle::Large, ACCENT);
        let w = measure(text, TextStyle::Large).w;
        cx.text(
            "ms",
            Point::new(pos.x + w as i16 + 4, pos.y + 8),
            TextStyle::Small,
            FG,
        );

        let rows = [
            ("MIN", self.min_x10),
            ("AVG", self.avg_x10),
            ("MAX", self.max_x10),
        ];
        for (i, (label, value)) in rows.iter().enumerate() {
            let y = origin.y + STATS_RECT.y + i as i16 * 18;
            cx.text(
                label,
                Point::new(origin.x + STATS_RECT.x, y),
                TextStyle::Small,
                DIM,
            );
            let mut b = [0u8; FORMAT_BUF];
            let value_text = format_x10(*value, &mut b);
            cx.text_right(
                value_text,
                origin.x + STATS_RECT.right() as i16 - 2,
                y,
                TextStyle::Small,
                FG,
            );
        }
        // Graph frame and bars.
        let frame = GRAPH_RECT.translate(origin.x, origin.y);
        cx.stroke_rect(frame, DIM);
        let inner_h = frame.h.saturating_sub(4) as u32;
        let bar_w = 5u16;
        let gap = 1u16;
        let count = self.len as u16;
        let total_w = count.saturating_mul(bar_w) + count.saturating_sub(1) * gap;
        let start_x = frame.x + ((frame.w.saturating_sub(total_w)) / 2) as i16;
        let baseline = frame.y + frame.h as i16 - 2;
        let scale = scale_x10.max(1) as u32;
        for (i, sample) in self.samples().enumerate() {
            let h = ((sample as u32 * inner_h) / scale).min(inner_h) as u16;
            if h == 0 {
                continue;
            }
            let x = start_x + (i as u16 * (bar_w + gap)) as i16;
            let y = baseline - h as i16;
            cx.fill_rect(Rect::new(x, y, bar_w, h), ACCENT);
        }

        // Hint line under the graph, above the shared footer.
        cx.text(
            "B: freeze   A hold: back",
            Point::new(origin.x + 16, origin.y + 228),
            TextStyle::Small,
            DIM,
        );
    }
}
