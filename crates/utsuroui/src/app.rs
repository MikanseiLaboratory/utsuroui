//! The application interface driven by the renderer and the Embassy task.

use crate::dirty::DirtyList;
use crate::event::Event;
use crate::paint::PaintCx;

/// An application screen set.
///
/// A single value owns the router, the per-screen state, the transition state
/// machine and the tweens. The framework only paints it, feeds it events and
/// asks it what is dirty.
pub trait App {
    /// Handles one input event.
    fn event(&mut self, event: &Event, now_us: u64, dirty: &mut DirtyList);

    /// Advances animations. Returns `true` while a frame is still animating.
    fn tick(&mut self, now_us: u64, dirty: &mut DirtyList) -> bool;

    /// Paints the current screen into `cx`.
    ///
    /// Whatever the caller marked dirty must be fully covered; the band buffer
    /// is not cleared beforehand.
    fn paint(&self, cx: &mut PaintCx<'_>, now_us: u64);

    /// Paints the *outgoing* screen underneath a crossfade.
    ///
    /// Return `false` (the default) to fall back to a plain repaint. Only called
    /// while [`App::crossfade_alpha`] returns `Some`.
    fn paint_under(&self, cx: &mut PaintCx<'_>, now_us: u64) -> bool {
        let _ = (cx, now_us);
        false
    }

    /// Crossfade alpha (`0..=255`) while a crossfade is running.
    fn crossfade_alpha(&self, now_us: u64) -> Option<u8> {
        let _ = now_us;
        None
    }

    /// Hint for the scheduler: when does the next animation end?
    fn next_wakeup_us(&self) -> Option<u64> {
        None
    }

    /// Receives a new latency sample (tenths of a millisecond) from the
    /// measurement side of the application.
    fn latency(&mut self, value_x10: u16, now_us: u64, dirty: &mut DirtyList) {
        let _ = (value_x10, now_us, dirty);
    }
}
