//! Input events and their handling result.

/// A semantic input event produced by the input task or a simulator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// Move focus to the next item (typical first button).
    Next,
    /// Move focus to the previous item (third button / encoder).
    Previous,
    /// Activate the focused item (typical second button).
    Select,
    /// Go back / pop the screen (long press on the first button).
    Back,
    /// A button was held past the long-press threshold.
    LongPress,
    /// Auto-repeat while a button is held.
    Repeat,
    /// Relative encoder steps, positive = clockwise.
    Encoder(i16),
    /// Touch / pointer sample in screen coordinates.
    Touch {
        /// Horizontal coordinate.
        x: i16,
        /// Vertical coordinate.
        y: i16,
        /// Whether the pointer is down.
        down: bool,
    },
}

impl Event {
    /// Whether the event is a directional focus move.
    pub const fn is_navigation(self) -> bool {
        matches!(
            self,
            Self::Next | Self::Previous | Self::Encoder(_) | Self::Touch { .. }
        )
    }
}

/// What a widget or screen did with an event.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EventOut {
    /// The handler changed state that must be repainted.
    pub dirty: bool,
    /// The handler used the event; siblings should not see it.
    pub consumed: bool,
}

impl EventOut {
    /// Marks the event as handled and dirty.
    pub fn accept(&mut self) {
        self.consumed = true;
        self.dirty = true;
    }

    /// Marks the event as handled without requesting a repaint.
    pub fn consume(&mut self) {
        self.consumed = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accept_sets_both_flags() {
        let mut out = EventOut::default();
        assert!(!out.consumed);
        out.accept();
        assert!(out.consumed);
        assert!(out.dirty);
    }

    #[test]
    fn navigation_classification() {
        assert!(Event::Next.is_navigation());
        assert!(Event::Encoder(-2).is_navigation());
        assert!(!Event::Select.is_navigation());
        assert!(!Event::Back.is_navigation());
    }
}
