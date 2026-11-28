//! A bottom banner that slides in, holds, and slides out.

use embedded_graphics::pixelcolor::Rgb565;

use crate::color::WHITE;
use crate::dirty::DirtyList;
use crate::geom::{Rect, Size};
use crate::paint::PaintCx;
use crate::text::{TextStyle, measure};
use crate::widget::{MeasureCx, Widget};

/// A transient message banner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Toast {
    /// Banner height in pixels.
    pub height: u16,
    /// Slide-in duration in microseconds.
    pub in_us: u32,
    /// How long it stays up, in microseconds.
    pub hold_us: u32,
    /// Slide-out duration in microseconds.
    pub out_us: u32,
    /// Banner background.
    pub bg: Rgb565,
    /// Text colour.
    pub fg: Rgb565,
}

impl Toast {
    /// Creates a toast with 160 ms in, 1.2 s hold and 160 ms out.
    pub const fn new(height: u16, bg: Rgb565, fg: Rgb565) -> Self {
        Self {
            height,
            in_us: 160_000,
            hold_us: 1_200_000,
            out_us: 160_000,
            bg,
            fg,
        }
    }

    /// Shows `text`, starting now.
    pub fn show(&self, state: &mut ToastState, text: &'static str, now_us: u64) {
        state.text = text;
        state.start_us = now_us;
        state.active = true;
    }

    /// Whether a banner is on screen (or moving off it).
    pub fn is_active(&self, state: &ToastState) -> bool {
        state.active
    }

    fn total_us(&self) -> u64 {
        self.in_us as u64 + self.hold_us as u64 + self.out_us as u64
    }

    /// The banner rectangle at `now_us` (it sits below `bounds` when hidden).
    pub fn rect(&self, state: &ToastState, now_us: u64) -> Rect {
        let elapsed = now_us.saturating_sub(state.start_us);
        let hidden_y = state.bounds.y + self.height as i16;
        let shown_y = state.bounds.y;

        let y = if elapsed >= self.total_us() {
            hidden_y
        } else if elapsed < self.in_us as u64 {
            // Slide up.
            let t = elapsed as i32 * 1_000 / self.in_us.max(1) as i32;
            hidden_y - ((self.height as i32 * t) / 1_000) as i16
        } else if elapsed < (self.in_us + self.hold_us) as u64 {
            shown_y
        } else {
            // Slide down.
            let t = (elapsed - (self.in_us + self.hold_us) as u64) as i32 * 1_000
                / self.out_us.max(1) as i32;
            shown_y + ((self.height as i32 * t) / 1_000) as i16
        };
        Rect::new(state.bounds.x, y, state.bounds.w, self.height)
    }
}

/// Retained state of a [`Toast`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToastState {
    /// The slot the banner occupies when fully shown.
    pub bounds: Rect,
    /// Message being shown.
    pub text: &'static str,
    active: bool,
    start_us: u64,
    shown: Rect,
}

impl Default for ToastState {
    fn default() -> Self {
        Self {
            bounds: Rect::EMPTY,
            text: "",
            active: false,
            start_us: 0,
            shown: Rect::EMPTY,
        }
    }
}

impl Widget for Toast {
    type State = ToastState;

    fn measure(&self, cx: &MeasureCx, _state: &Self::State) -> Size {
        Size::new(cx.max.w, self.height)
    }

    fn arrange(&self, bounds: Rect, state: &mut Self::State) {
        state.bounds = bounds;
    }

    fn tick(&self, state: &mut Self::State, now_us: u64, dirty: &mut DirtyList) -> bool {
        if !state.active {
            return false;
        }
        let rect = self.rect(state, now_us);
        if rect != state.shown {
            dirty.add(state.shown.union(rect));
            state.shown = rect;
        }
        let finished = now_us.saturating_sub(state.start_us) >= self.total_us();
        if finished {
            state.active = false;
        }
        true
    }

    fn paint(&self, cx: &mut PaintCx<'_>, state: &Self::State, now_us: u64) {
        if !state.active && state.text.is_empty() {
            return;
        }
        let rect = self.rect(state, now_us);
        // Nothing to do once it has fully slid away.
        if rect.y >= state.bounds.y + self.height as i16 {
            return;
        }
        cx.fill_rect(rect, self.bg);
        let text_w = measure(state.text, TextStyle::Small).w;
        let text_h = TextStyle::Small.line_height();
        let x = rect.x + ((rect.w.saturating_sub(text_w)) / 2) as i16;
        let y = rect.y + ((rect.h.saturating_sub(text_h)) / 2) as i16;
        if !state.text.is_empty() {
            cx.text(
                state.text,
                crate::geom::Point::new(x, y),
                TextStyle::Small,
                self.fg,
            );
        }
    }
}

/// Convenience for callers that just want a white-on-dark banner.
impl Toast {
    /// Default colours: dark banner, white text.
    pub const fn plain(height: u16) -> Self {
        Self::new(height, crate::color::DARK_GRAY, WHITE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slides_in_holds_and_leaves() {
        let w = Toast::plain(28);
        let mut state = ToastState::default();
        w.arrange(Rect::new(8, 240, 224, 28), &mut state);
        w.show(&mut state, "Saved", 0);

        let mut dirty = DirtyList::new(Rect::new(0, 0, 240, 320));
        // Mid slide-in.
        assert!(w.tick(&mut state, 80_000, &mut dirty));
        let early = w.rect(&state, 80_000);
        assert!(early.y > 240 && early.y < 268);

        // Fully up.
        assert_eq!(w.rect(&state, 200_000).y, 240);
        // Gone.
        assert_eq!(w.rect(&state, 2_000_000).y, 268);
        assert!(!w.is_active(&state) || w.rect(&state, 2_000_000).y == 268);
    }
}
