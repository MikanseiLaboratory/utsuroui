//! A selection bar that slides between rows.

use embedded_graphics::pixelcolor::Rgb565;

use crate::anim::Tween;
use crate::dirty::DirtyList;
use crate::geom::{Rect, Size};
use crate::paint::PaintCx;
use crate::widget::{MeasureCx, Widget};

/// A highlight bar over a fixed number of equal rows.
///
/// The widget owns the motion; the application draws the row content, using
/// [`Highlight::row_rect`] and [`Highlight::bar`] to know where things are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Highlight {
    /// Number of rows.
    pub rows: u8,
    /// Height of one row in pixels.
    pub row_h: u16,
    /// Slide duration in microseconds.
    pub duration_us: u32,
    /// Bar fill.
    pub fill: Rgb565,
    /// Accent stripe colour on the left edge.
    pub edge: Rgb565,
}

impl Highlight {
    /// Creates a highlight bar.
    pub const fn new(rows: u8, row_h: u16, duration_us: u32, fill: Rgb565, edge: Rgb565) -> Self {
        Self {
            rows,
            row_h,
            duration_us,
            fill,
            edge,
        }
    }

    /// Rectangle of `row`, in screen coordinates.
    pub fn row_rect(&self, state: &HighlightState, row: u8) -> Rect {
        Rect::new(
            state.bounds.x,
            state.bounds.y + row as i16 * self.row_h as i16,
            state.bounds.w,
            self.row_h,
        )
    }

    /// The bar as it is drawn at `now_us`.
    pub fn bar(&self, state: &HighlightState, now_us: u64) -> Rect {
        let y = state.y.value_at(now_us) as i16;
        Rect::new(state.bounds.x, state.bounds.y + y, state.bounds.w, self.row_h)
    }

    /// Selects `row`; returns whether the selection changed.
    pub fn select(&self, state: &mut HighlightState, row: u8, now_us: u64) -> bool {
        let row = row.min(self.rows.saturating_sub(1));
        if row == state.index {
            return false;
        }
        // Remember where the bar was, so the first dirty rectangle covers the
        // whole path even before `tick` runs.
        state.shown = self.bar(state, now_us);
        state.index = row;
        let target = row as i32 * self.row_h as i32;
        state.y.retarget(now_us, target, self.duration_us);
        true
    }
}

/// Retained state of a [`Highlight`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HighlightState {
    /// Arranged bounds of the whole list.
    pub bounds: Rect,
    /// Selected row.
    pub index: u8,
    y: Tween,
    shown: Rect,
}

impl Default for HighlightState {
    fn default() -> Self {
        Self {
            bounds: Rect::EMPTY,
            index: 0,
            y: Tween::settled(0),
            shown: Rect::EMPTY,
        }
    }
}

impl Widget for Highlight {
    type State = HighlightState;

    fn measure(&self, cx: &MeasureCx, _state: &Self::State) -> Size {
        Size::new(cx.max.w, self.rows as u16 * self.row_h)
    }

    fn arrange(&self, bounds: Rect, state: &mut Self::State) {
        state.bounds = bounds;
    }

    fn event(
        &self,
        event: &crate::event::Event,
        state: &mut Self::State,
        out: &mut crate::event::EventOut,
        now_us: u64,
    ) {
        match event {
            crate::event::Event::Next => {
                if self.select(state, (state.index + 1) % self.rows.max(1), now_us) {
                    out.dirty = true;
                    out.consumed = true;
                }
            }
            crate::event::Event::Previous => {
                let row = if state.index == 0 {
                    self.rows.saturating_sub(1)
                } else {
                    state.index - 1
                };
                if self.select(state, row, now_us) {
                    out.dirty = true;
                    out.consumed = true;
                }
            }
            _ => {}
        }
    }

    fn tick(&self, state: &mut Self::State, now_us: u64, dirty: &mut DirtyList) -> bool {
        let running = state.y.poll(now_us);
        let bar = self.bar(state, now_us);
        if bar != state.shown {
            dirty.add(state.shown.union(bar));
            state.shown = bar;
        }
        running
    }

    fn paint(&self, cx: &mut PaintCx<'_>, state: &Self::State, now_us: u64) {
        let bar = self.bar(state, now_us);
        cx.fill_rect(bar, self.fill);
        cx.fill_rect(Rect::new(bar.x, bar.y, 4, bar.h), self.edge);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn widget() -> Highlight {
        Highlight::new(3, 28, 120_000, crate::color::YELLOW, crate::color::GRAY)
    }

    #[test]
    fn select_slides_and_marks_old_and_new() {
        let w = widget();
        let mut state = HighlightState::default();
        w.arrange(Rect::new(0, 40, 240, 84), &mut state);
        assert!(w.select(&mut state, 2, 0));

        let mut dirty = DirtyList::new(Rect::new(0, 0, 240, 320));
        assert!(w.tick(&mut state, 60_000, &mut dirty));
        assert_eq!(dirty.len(), 1);
        // The path covers the old row and the row being reached.
        let path = dirty.as_slice()[0];
        assert_eq!(path.y, 40);
        assert!(path.h >= 28);
    }

    #[test]
    fn settles_on_the_selected_row() {
        let w = widget();
        let mut state = HighlightState::default();
        w.arrange(Rect::new(0, 40, 240, 84), &mut state);
        w.select(&mut state, 2, 0);
        let mut now = 0;
        while w.tick(&mut state, now, &mut DirtyList::new(Rect::new(0, 0, 240, 320))) {
            now += 16_000;
        }
        assert_eq!(w.bar(&state, now).y, 40 + 2 * 28);
    }
}
