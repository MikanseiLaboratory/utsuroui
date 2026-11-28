//! A switch with a sliding knob.

use embedded_graphics::pixelcolor::Rgb565;

use crate::anim::Tween;
use crate::color::blend;
use crate::dirty::DirtyList;
use crate::event::{Event, EventOut};
use crate::geom::{Rect, Size};
use crate::paint::PaintCx;
use crate::widget::{MeasureCx, Widget};

/// An on/off switch: a pill track with a knob that slides over `duration_us`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Toggle {
    /// Slide duration in microseconds.
    pub duration_us: u32,
    /// Track colour when on.
    pub on: Rgb565,
    /// Track colour when off.
    pub off: Rgb565,
    /// Knob colour.
    pub knob: Rgb565,
    /// Row height.
    pub height: u16,
}

impl Toggle {
    /// Creates a switch.
    pub const fn new(duration_us: u32, on: Rgb565, off: Rgb565, knob: Rgb565) -> Self {
        Self {
            duration_us,
            on,
            off,
            knob,
            height: 40,
        }
    }

    /// Track rectangle inside `bounds`: 48x24, right aligned.
    fn track(&self, state: &ToggleState) -> Rect {
        let w = 48u16;
        let h = 24u16;
        Rect::new(
            state.bounds.x + state.bounds.w as i16 - w as i16 - 8,
            state.bounds.y + ((state.bounds.h - h) / 2) as i16,
            w,
            h,
        )
    }

    /// Flips the switch; returns the new value.
    pub fn toggle(&self, state: &mut ToggleState, now_us: u64) -> bool {
        state.on = !state.on;
        state.pos.retarget(now_us, if state.on { 1 } else { 0 }, self.duration_us);
        state.on
    }
}

/// Retained state of a [`Toggle`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToggleState {
    /// Arranged bounds of the row.
    pub bounds: Rect,
    /// Current value.
    pub on: bool,
    pos: Tween,
}

impl Default for ToggleState {
    fn default() -> Self {
        Self {
            bounds: Rect::EMPTY,
            on: false,
            pos: Tween::settled(0),
        }
    }
}

impl ToggleState {
    /// Creates a state with a value.
    pub fn with_value(on: bool) -> Self {
        Self {
            on,
            pos: Tween::settled(if on { 1 } else { 0 }),
            ..Self::default()
        }
    }
}

impl Widget for Toggle {
    type State = ToggleState;

    fn measure(&self, cx: &MeasureCx, _state: &Self::State) -> Size {
        Size::new(cx.max.w, self.height)
    }

    fn arrange(&self, bounds: Rect, state: &mut Self::State) {
        state.bounds = bounds;
    }

    fn event(&self, event: &Event, state: &mut Self::State, out: &mut EventOut, now_us: u64) {
        let activate = matches!(event, Event::Select | Event::Touch { down: true, .. });
        if activate {
            self.toggle(state, now_us);
            out.dirty = true;
            out.consumed = true;
        }
    }

    fn tick(&self, state: &mut Self::State, now_us: u64, dirty: &mut DirtyList) -> bool {
        let running = state.pos.poll(now_us);
        if running {
            dirty.add(state.bounds);
        }
        running
    }

    fn paint(&self, cx: &mut PaintCx<'_>, state: &Self::State, now_us: u64) {
        let track = self.track(state);
        // The tween holds 0 or 1, not a 0..1 progress ratio, so map the value.
        let position = state.pos.value_at(now_us).clamp(0, 1);
        let progress = (position * 255) as u8;
        cx.fill_rect(track, blend(self.on, self.off, progress));

        let knob = Rect::new(0, 0, 18, 18);
        let left = track.x + 3;
        let right = track.x + track.w as i16 - 3 - knob.w as i16;
        let x = left + ((right - left) as i32 * progress as i32 / 255) as i16;
        let y = track.y + ((track.h - knob.h) / 2) as i16;
        cx.fill_rect(Rect::new(x, y, knob.w, knob.h), self.knob);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_flips_and_marks_its_row() {
        let w = Toggle::new(120_000, crate::color::YELLOW, crate::color::GRAY, crate::color::WHITE);
        let mut state = ToggleState::default();
        w.arrange(Rect::new(8, 64, 224, 40), &mut state);
        let mut out = EventOut::default();
        w.event(&Event::Select, &mut state, &mut out, 0);
        assert!(state.on);
        assert!(out.dirty);

        let mut dirty = DirtyList::new(Rect::new(0, 0, 240, 320));
        assert!(w.tick(&mut state, 16_000, &mut dirty));
        assert_eq!(dirty.as_slice()[0], Rect::new(8, 64, 224, 40));
    }
}
