//! Horizontally scrolling text.

use embedded_graphics::pixelcolor::Rgb565;

use crate::dirty::DirtyList;
use crate::geom::{Point, Rect, Size};
use crate::paint::PaintCx;
use crate::text::{TextStyle, measure};
use crate::widget::{MeasureCx, Widget};

/// Text that scrolls right-to-left and wraps around.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Marquee {
    /// Text to scroll.
    pub text: &'static str,
    /// Speed in pixels per second.
    pub px_per_s: i32,
    /// Font.
    pub style: TextStyle,
    /// Colour.
    pub color: Rgb565,
    /// Blank space between the end and the restarted beginning, in pixels.
    pub gap: u16,
}

impl Marquee {
    /// Creates a marquee.
    pub const fn new(text: &'static str, px_per_s: i32, style: TextStyle, color: Rgb565) -> Self {
        Self {
            text,
            px_per_s,
            style,
            color,
            gap: 16,
        }
    }

    /// Distance the text has travelled at `now_us`.
    pub fn offset(&self, state: &MarqueeState, now_us: u64) -> i32 {
        let period = self.period(state);
        if period <= 0 {
            return 0;
        }
        ((now_us / 1_000) as i32 * self.px_per_s / 1_000).rem_euclid(period)
    }

    /// One full cycle: the text plus one screen width plus the gap.
    fn period(&self, state: &MarqueeState) -> i32 {
        measure(self.text, self.style).w as i32 + state.bounds.w as i32 + self.gap as i32
    }
}

/// Retained state of a [`Marquee`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MarqueeState {
    /// Arranged bounds (the visible window).
    pub bounds: Rect,
    last: i32,
}

impl Default for MarqueeState {
    fn default() -> Self {
        Self {
            bounds: Rect::EMPTY,
            last: i32::MIN,
        }
    }
}

impl Widget for Marquee {
    type State = MarqueeState;

    fn measure(&self, cx: &MeasureCx, _state: &Self::State) -> Size {
        Size::new(cx.max.w, self.style.line_height())
    }

    fn arrange(&self, bounds: Rect, state: &mut Self::State) {
        state.bounds = bounds;
    }

    fn tick(&self, state: &mut Self::State, now_us: u64, dirty: &mut DirtyList) -> bool {
        let offset = self.offset(state, now_us);
        if offset == state.last {
            return false;
        }
        state.last = offset;
        dirty.add(state.bounds);
        true
    }

    fn paint(&self, cx: &mut PaintCx<'_>, state: &Self::State, now_us: u64) {
        let offset = self.offset(state, now_us);
        let period = self.period(state);
        let y = state.bounds.y;

        cx.with_clip(state.bounds, |cx| {
            let mut x = state.bounds.x as i32 - offset;
            // Two copies cover the wrap-around without a gap.
            while x < state.bounds.x as i32 + state.bounds.w as i32 {
                if x + period >= state.bounds.x as i32 {
                    cx.text(
                        self.text,
                        Point::new(x.clamp(i16::MIN as i32, i16::MAX as i32) as i16, y),
                        self.style,
                        self.color,
                    );
                }
                x += period;
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrolls_and_marks_the_window() {
        let w = Marquee::new("hello world", 60, TextStyle::Small, crate::color::WHITE);
        let mut state = MarqueeState::default();
        w.arrange(Rect::new(0, 50, 240, 12), &mut state);

        let mut dirty = DirtyList::new(Rect::new(0, 0, 240, 320));
        assert!(w.tick(&mut state, 1_000_000, &mut dirty));
        assert_eq!(dirty.as_slice(), &[Rect::new(0, 50, 240, 12)]);

        // Same instant: nothing changed, no work.
        assert!(!w.tick(
            &mut state,
            1_000_000,
            &mut DirtyList::new(Rect::new(0, 0, 240, 320))
        ));
    }
}
