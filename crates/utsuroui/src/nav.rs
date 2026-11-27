//! Screen stack and transition state machine.
//!
//! The application owns its screen `enum`; the framework only tracks a
//! fixed-capacity stack of screen ids and the timing of the transition between
//! them. That keeps the router allocation-free and lets the app decide how each
//! id is turned into state and pixels.

use crate::anim::Q16;

/// A navigation request emitted by a screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavAction<S> {
    /// Push a new screen on top of the stack.
    Push(S),
    /// Pop the current screen (the root screen cannot be popped).
    Pop,
    /// Replace the current screen without growing the stack.
    Replace(S),
}

/// A fixed-capacity screen stack.
#[derive(Clone, Debug)]
pub struct Router<S: Copy, const N: usize> {
    stack: [Option<S>; N],
    len: usize,
}

impl<S: Copy, const N: usize> Router<S, N> {
    /// Creates a router with `root` as the only screen.
    pub fn new(root: S) -> Self {
        let mut stack = [None; N];
        stack[0] = Some(root);
        Self { stack, len: 1 }
    }

    /// The visible (topmost) screen.
    pub fn current(&self) -> S {
        match self.stack[self.len - 1] {
            Some(id) => id,
            // Invariant: `len >= 1` and slots below `len` are always populated.
            None => unreachable!(),
        }
    }

    /// Number of screens on the stack.
    pub const fn depth(&self) -> usize {
        self.len
    }

    /// The stack from the root to the current screen.
    pub fn stack(&self) -> impl Iterator<Item = S> + '_ {
        self.stack[..self.len].iter().filter_map(|slot| *slot)
    }

    /// Whether a `Pop` would succeed.
    pub const fn can_pop(&self) -> bool {
        self.len > 1
    }

    /// Applies a navigation action. Returns `false` when the stack is full or
    /// the root screen would be popped (the action is then ignored).
    pub fn apply(&mut self, action: NavAction<S>) -> bool {
        match action {
            NavAction::Push(id) => {
                if self.len >= N {
                    return false;
                }
                self.stack[self.len] = Some(id);
                self.len += 1;
                true
            }
            NavAction::Pop => {
                if self.len <= 1 {
                    return false;
                }
                self.len -= 1;
                self.stack[self.len] = None;
                true
            }
            NavAction::Replace(id) => {
                self.stack[self.len - 1] = Some(id);
                true
            }
        }
    }
}

/// Direction of a sliding transition.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SlideDir {
    /// New screen slides in from the right.
    #[default]
    Horizontal,
    /// New screen slides in from the bottom.
    Vertical,
}

/// How a screen change is animated.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TransitionKind {
    /// Instant screen change.
    #[default]
    Cut,
    /// Fade the current screen to black, swap, fade in.
    Fade {
        /// Fade-to-black duration in microseconds.
        out_us: u32,
        /// Fade-from-black duration in microseconds.
        in_us: u32,
    },
    /// Slide the old screen out and the new screen in.
    Slide {
        /// Slide axis.
        dir: SlideDir,
        /// Total duration in microseconds.
        dur_us: u32,
    },
    /// Blend the old frame into the new one.
    Crossfade {
        /// Total duration in microseconds.
        dur_us: u32,
    },
}

impl TransitionKind {
    /// Longest duration of the kind, in microseconds.
    pub const fn duration_us(self) -> u32 {
        match self {
            Self::Cut => 0,
            Self::Fade { out_us, in_us } => {
                if out_us > in_us {
                    out_us
                } else {
                    in_us
                }
            }
            Self::Slide { dur_us, .. } | Self::Crossfade { dur_us } => dur_us,
        }
    }
}

/// Current phase of a transition.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TransitionPhase {
    /// No transition running.
    #[default]
    Idle,
    /// Fading out to black.
    FadingOut,
    /// Fading in from black.
    FadingIn,
    /// Sliding between screens.
    Sliding,
    /// Crossfading between screens.
    Crossfading,
}

/// Result of [`Transition::trigger`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trigger<S> {
    /// Apply the action immediately (cut, slide, crossfade).
    Immediate(NavAction<S>),
    /// The action is applied at the swap point reported by [`Transition::tick`].
    Deferred,
}

/// Timing state for one screen transition.
#[derive(Clone, Copy, Debug)]
pub struct Transition<S: Copy> {
    kind: TransitionKind,
    phase: TransitionPhase,
    start_us: u64,
    duration_us: u32,
    pending: Option<NavAction<S>>,
}

impl<S: Copy> Default for Transition<S> {
    fn default() -> Self {
        Self {
            kind: TransitionKind::Cut,
            phase: TransitionPhase::Idle,
            start_us: 0,
            duration_us: 0,
            pending: None,
        }
    }
}

impl<S: Copy> Transition<S> {
    /// Creates an idle transition using `kind`.
    pub const fn new(kind: TransitionKind) -> Self {
        Self {
            kind,
            phase: TransitionPhase::Idle,
            start_us: 0,
            duration_us: 0,
            pending: None,
        }
    }

    /// Changes the transition style; a running transition is cancelled.
    pub fn set_kind(&mut self, kind: TransitionKind) {
        self.kind = kind;
        self.cancel();
    }

    /// The configured style.
    pub const fn kind(&self) -> TransitionKind {
        self.kind
    }

    /// Starts `action`.
    pub fn trigger(&mut self, now_us: u64, action: NavAction<S>) -> Trigger<S> {
        match self.kind {
            TransitionKind::Cut => Trigger::Immediate(action),
            TransitionKind::Fade { out_us, in_us } => {
                self.pending = Some(action);
                self.duration_us = out_us;
                self.start_us = now_us;
                self.phase = TransitionPhase::FadingOut;
                let _ = in_us;
                Trigger::Deferred
            }
            TransitionKind::Slide { dir, dur_us } => {
                let _ = dir;
                self.duration_us = dur_us;
                self.start_us = now_us;
                self.phase = TransitionPhase::Sliding;
                Trigger::Immediate(action)
            }
            TransitionKind::Crossfade { dur_us } => {
                self.duration_us = dur_us;
                self.start_us = now_us;
                self.phase = TransitionPhase::Crossfading;
                Trigger::Immediate(action)
            }
        }
    }

    /// Advances the state machine.
    ///
    /// Returns the action to apply exactly once, at the fade swap point.
    pub fn tick(&mut self, now_us: u64) -> Option<NavAction<S>> {
        let elapsed = now_us.saturating_sub(self.start_us);
        match self.phase {
            TransitionPhase::Idle => None,
            TransitionPhase::FadingOut => {
                if elapsed >= self.duration_us as u64 {
                    let action = self.pending.take();
                    let in_us = match self.kind {
                        TransitionKind::Fade { in_us, .. } => in_us,
                        _ => 0,
                    };
                    self.phase = if in_us == 0 {
                        TransitionPhase::Idle
                    } else {
                        TransitionPhase::FadingIn
                    };
                    self.start_us = now_us;
                    self.duration_us = in_us;
                    action
                } else {
                    None
                }
            }
            TransitionPhase::FadingIn => {
                if elapsed >= self.duration_us as u64 {
                    self.phase = TransitionPhase::Idle;
                }
                None
            }
            TransitionPhase::Sliding | TransitionPhase::Crossfading => {
                if elapsed >= self.duration_us as u64 {
                    self.phase = TransitionPhase::Idle;
                }
                None
            }
        }
    }

    /// Whether a transition is still running.
    pub const fn is_active(&self) -> bool {
        !matches!(self.phase, TransitionPhase::Idle)
    }

    /// Current phase.
    pub const fn phase(&self) -> TransitionPhase {
        self.phase
    }

    /// Black overlay alpha (`0..=255`) for a fade.
    pub fn overlay_alpha(&self, now_us: u64) -> u8 {
        let elapsed = now_us.saturating_sub(self.start_us);
        match self.phase {
            TransitionPhase::FadingOut => ramp(elapsed, self.duration_us),
            TransitionPhase::FadingIn => 255 - ramp(elapsed, self.duration_us),
            _ => 0,
        }
    }

    /// Crossfade alpha (`0..=255`) when a crossfade is running.
    pub fn crossfade_alpha(&self, now_us: u64) -> Option<u8> {
        match self.phase {
            TransitionPhase::Crossfading => {
                let elapsed = now_us.saturating_sub(self.start_us);
                Some(ramp(elapsed, self.duration_us))
            }
            _ => None,
        }
    }

    /// Slide progress (`0..=65535`) when a slide is running.
    pub fn slide_progress(&self, now_us: u64) -> Option<Q16> {
        match self.phase {
            TransitionPhase::Sliding => {
                let elapsed = now_us.saturating_sub(self.start_us);
                Some(
                    ((elapsed.min(self.duration_us as u64) * 65_535)
                        / self.duration_us.max(1) as u64) as Q16,
                )
            }
            _ => None,
        }
    }

    /// Aborts the transition; no pending action is applied.
    pub fn cancel(&mut self) {
        self.phase = TransitionPhase::Idle;
        self.pending = None;
        self.duration_us = 0;
    }
}

fn ramp(elapsed: u64, duration_us: u32) -> u8 {
    if duration_us == 0 {
        return 255;
    }
    let t = elapsed.min(duration_us as u64);
    ((t * 255) / duration_us as u64) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_cannot_pop() {
        let mut r: Router<u8, 4> = Router::new(0);
        assert!(!r.apply(NavAction::Pop));
        assert_eq!(r.current(), 0);
        assert_eq!(r.depth(), 1);
    }

    #[test]
    fn push_pop_and_replace() {
        let mut r: Router<u8, 4> = Router::new(0);
        assert!(r.apply(NavAction::Push(1)));
        assert!(r.apply(NavAction::Push(2)));
        assert_eq!(r.current(), 2);
        assert_eq!(r.depth(), 3);
        assert_eq!(r.stack().collect::<Vec<_>>(), vec![0, 1, 2]);
        assert!(r.apply(NavAction::Replace(7)));
        assert_eq!(r.current(), 7);
        assert_eq!(r.depth(), 3);
        assert!(r.apply(NavAction::Pop));
        assert_eq!(r.current(), 1);
        assert!(r.apply(NavAction::Pop));
        assert_eq!(r.current(), 0);
        assert!(!r.can_pop());
    }

    #[test]
    fn stack_full_is_rejected() {
        let mut r: Router<u8, 2> = Router::new(0);
        assert!(r.apply(NavAction::Push(1)));
        assert!(!r.apply(NavAction::Push(2)));
        assert_eq!(r.depth(), 2);
        assert_eq!(r.current(), 1);
    }

    #[test]
    fn cut_is_immediate() {
        let mut t: Transition<u8> = Transition::new(TransitionKind::Cut);
        assert_eq!(
            t.trigger(0, NavAction::Push(1)),
            Trigger::Immediate(NavAction::Push(1))
        );
        assert!(!t.is_active());
        assert_eq!(t.tick(0), None);
    }

    #[test]
    fn fade_defers_until_swap_point() {
        let mut t: Transition<u8> = Transition::new(TransitionKind::Fade {
            out_us: 100,
            in_us: 200,
        });
        assert_eq!(t.trigger(1000, NavAction::Push(1)), Trigger::Deferred);
        assert_eq!(t.phase(), TransitionPhase::FadingOut);
        assert_eq!(t.overlay_alpha(1000), 0);
        assert_eq!(t.overlay_alpha(1050), 127);
        assert_eq!(t.tick(1099), None);
        assert_eq!(t.tick(1100), Some(NavAction::Push(1)));
        assert_eq!(t.phase(), TransitionPhase::FadingIn);
        assert_eq!(t.overlay_alpha(1200), 128);
        assert!(t.is_active());
        assert_eq!(t.tick(1300), None);
        assert!(!t.is_active());
        assert_eq!(t.overlay_alpha(1300), 0);
    }

    #[test]
    fn fade_swap_is_returned_only_once() {
        let mut t: Transition<u8> = Transition::new(TransitionKind::Fade {
            out_us: 10,
            in_us: 10,
        });
        t.trigger(0, NavAction::Pop);
        assert_eq!(t.tick(10), Some(NavAction::Pop));
        assert_eq!(t.tick(20), None);
    }

    #[test]
    fn fade_without_in_phase_settles_at_swap() {
        let mut t: Transition<u8> = Transition::new(TransitionKind::Fade {
            out_us: 10,
            in_us: 0,
        });
        t.trigger(0, NavAction::Pop);
        assert_eq!(t.tick(10), Some(NavAction::Pop));
        assert!(!t.is_active());
    }

    #[test]
    fn cancel_drops_pending_action() {
        let mut t: Transition<u8> = Transition::new(TransitionKind::Fade {
            out_us: 100,
            in_us: 100,
        });
        t.trigger(0, NavAction::Push(3));
        t.cancel();
        assert!(!t.is_active());
        assert_eq!(t.tick(1000), None);
    }

    #[test]
    fn crossfade_alpha_ramps() {
        let mut t: Transition<u8> = Transition::new(TransitionKind::Crossfade { dur_us: 1000 });
        assert_eq!(
            t.trigger(0, NavAction::Push(2)),
            Trigger::Immediate(NavAction::Push(2))
        );
        assert_eq!(t.crossfade_alpha(0), Some(0));
        assert_eq!(t.crossfade_alpha(500), Some(127));
        assert_eq!(t.crossfade_alpha(1000), Some(255));
        assert_eq!(t.crossfade_alpha(500), Some(127));
        t.tick(1000);
        assert!(!t.is_active());
        assert_eq!(t.crossfade_alpha(1000), None);
    }

    #[test]
    fn slide_progress_ramps_and_finishes() {
        let mut t: Transition<u8> = Transition::new(TransitionKind::Slide {
            dir: SlideDir::Horizontal,
            dur_us: 1000,
        });
        t.trigger(0, NavAction::Push(1));
        assert_eq!(t.slide_progress(0), Some(0));
        assert_eq!(t.slide_progress(500), Some(32_767));
        assert_eq!(t.slide_progress(1000), Some(65_535));
        t.tick(1000);
        assert!(!t.is_active());
        assert_eq!(t.slide_progress(1000), None);
    }

    #[test]
    fn set_kind_cancels_running_transition() {
        let mut t: Transition<u8> = Transition::new(TransitionKind::Fade {
            out_us: 100,
            in_us: 100,
        });
        t.trigger(0, NavAction::Push(1));
        t.set_kind(TransitionKind::Cut);
        assert!(!t.is_active());
        assert_eq!(t.kind(), TransitionKind::Cut);
    }
}
