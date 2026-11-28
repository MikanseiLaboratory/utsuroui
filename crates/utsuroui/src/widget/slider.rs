//! A stepped slider with an animated fill.

use embedded_graphics::pixelcolor::Rgb565;

use crate::anim::Tween;
use crate::dirty::DirtyList;
use crate::event::{Event, EventOut};
use crate::geom::{Rect, Size};
use crate::paint::PaintCx;
use crate::widget::{MeasureCx, Widget};

/// A horizontal slider over `min..=max`, moved in `step` increments.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slider {
    /// Lowest value.
    pub min: i16,
    /// Highest value.
    pub max: i16,
    /// Increment per key press.
    pub step: i16,
    /// Follow duration in microseconds.
    pub duration_us: u32,
    /// Track colour.
    pub track: Rgb565,
    /// Filled part colour.
    pub fill: Rgb565,
    /// Knob colour.
    pub knob: Rgb565,
}

impl Slider {
    /// Creates a slider over `min..=max`.
    pub const fn new(min: i16, max: i16, step: i16, duration_us: u32) -> Self {
        Self {
            min,
            max,
            step,
            duration_us,
            track: crate::color::GRAY,
            fill: crate::color::YELLOW,
            knob: crate::color::WHITE,
        }
    }

    /// Clamped current value.
    pub fn value(&self, state: &SliderState) -> i16 {
        state.value.clamp(self.min, self.max)
    }

    /// Moves by `steps` increments; returns the new value.
    pub fn nudge(&self, state: &mut SliderState, steps: i16, now_us: u64) -> i16 {
        let next = (state.value as i32 + steps as i32 * self.step as i32)
            .clamp(self.min as i32, self.max as i32) as i16;
        self.set(state, next, now_us)
    }

    /// Sets the value; returns the clamped value.
    pub fn set(&self, state: &mut SliderState, value: i16, now_us: u64) -> i16 {
        let value = value.clamp(self.min, self.max);
        if value != state.value {
            state.value = value;
            state.shown.retarget(now_us, value as i32, self.duration_us);
        }
        value
    }

    /// Track rectangle inside `bounds`.
    fn track(&self, state: &SliderState) -> Rect {
        let h = 10u16;
        Rect::new(
            state.bounds.x + 8,
            state.bounds.y + ((state.bounds.h - h) / 2) as i16,
            state.bounds.w.saturating_sub(16),
            h,
        )
    }

    fn fraction(&self, value: i32) -> i32 {
        let span = (self.max - self.min).max(1) as i32;
        ((value - self.min as i32) * 1_000 / span).clamp(0, 1_000)
    }
}

/// Retained state of a [`Slider`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SliderState {
    /// Arranged bounds of the row.
    pub bounds: Rect,
    /// Current value.
    pub value: i16,
    shown: Tween,
}

impl Default for SliderState {
    fn default() -> Self {
        Self {
            bounds: Rect::EMPTY,
            value: 0,
            shown: Tween::settled(0),
        }
    }
}

impl SliderState {
    /// Creates a state at `value`.
    pub fn with_value(value: i16) -> Self {
        Self {
            value,
            shown: Tween::settled(value as i32),
            ..Self::default()
        }
    }
}

impl Widget for Slider {
    type State = SliderState;

    fn measure(&self, cx: &MeasureCx, _state: &Self::State) -> Size {
        Size::new(cx.max.w, 44)
    }

    fn arrange(&self, bounds: Rect, state: &mut Self::State) {
        state.bounds = bounds;
    }

    fn event(&self, event: &Event, state: &mut Self::State, out: &mut EventOut, now_us: u64) {
        let steps = match event {
            Event::Next | Event::Repeat => 1,
            Event::Previous => -1,
            Event::Encoder(steps) => *steps,
            _ => return,
        };
        self.nudge(state, steps, now_us);
        out.dirty = true;
        out.consumed = true;
    }

    fn tick(&self, state: &mut Self::State, now_us: u64, dirty: &mut DirtyList) -> bool {
        let running = state.shown.poll(now_us);
        if running {
            dirty.add(state.bounds);
        }
        running
    }

    fn paint(&self, cx: &mut PaintCx<'_>, state: &Self::State, now_us: u64) {
        let track = self.track(state);
        cx.fill_rect(track, self.track);

        let shown = state.shown.value_at(now_us);
        let fraction = self.fraction(shown);
        let filled = (track.w as i32 * fraction / 1_000) as u16;
        if filled > 0 {
            cx.fill_rect(Rect::new(track.x, track.y, filled, track.h), self.fill);
        }

        // Knob centred on the fill end.
        let knob_w = 18i16;
        let x = (track.x + filled as i16 - knob_w / 2)
            .clamp(track.x - knob_w / 2, track.x + track.w as i16 - knob_w / 2);
        let y = track.y + ((track.h.saturating_sub(knob_w as u16)) / 2) as i16;
        cx.fill_rect(Rect::new(x, y, knob_w as u16, knob_w as u16), self.knob);

        // One pixel per major tick so the scale is readable.
        let span = (self.max - self.min).max(1);
        let mut tick = 0i16;
        while tick <= span {
            let tx = track.x + (track.w as i32 * tick as i32 / span as i32) as i16;
            cx.vline(
                tx,
                track.y.saturating_sub(3),
                track.y.saturating_sub(1),
                self.track,
            );
            tick += 10;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nudges_clamp_and_animate() {
        let w = Slider::new(0, 100, 5, 120_000);
        let mut state = SliderState::with_value(50);
        w.arrange(Rect::new(8, 160, 224, 44), &mut state);

        assert_eq!(w.nudge(&mut state, 3, 0), 65);
        assert_eq!(w.nudge(&mut state, 100, 0), 100);

        let mut dirty = DirtyList::new(Rect::new(0, 0, 240, 320));
        assert!(w.tick(&mut state, 16_000, &mut dirty));
        assert_eq!(dirty.len(), 1);
    }
}
