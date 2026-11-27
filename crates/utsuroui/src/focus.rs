//! Focus tracking for button/encoder driven menus.

use crate::geom::{Point, Rect};

/// The focused index inside a list of `count` items.
///
/// The focus starts at index 0: there is no "focus_forward() must be called"
/// trap like Buoyant's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Focus {
    index: u8,
    count: u8,
}

impl Focus {
    /// Creates a focus over `count` items, pointing at the first one.
    pub const fn new(count: u8) -> Self {
        Self { index: 0, count }
    }

    /// Current index (always `< count` when `count > 0`).
    pub const fn index(self) -> u8 {
        self.index
    }

    /// Number of focusable items.
    pub const fn count(self) -> u8 {
        self.count
    }

    /// Whether there is anything to focus.
    pub const fn is_empty(self) -> bool {
        self.count == 0
    }

    /// Updates the number of items, clamping the index.
    pub fn set_count(&mut self, count: u8) {
        self.count = count;
        if count == 0 {
            self.index = 0;
        } else if self.index >= count {
            self.index = count - 1;
        }
    }

    /// Moves to the next item, wrapping around.
    pub fn next(&mut self) {
        if self.count == 0 {
            return;
        }
        self.index = (self.index + 1) % self.count;
    }

    /// Moves to the previous item, wrapping around.
    pub fn previous(&mut self) {
        if self.count == 0 {
            return;
        }
        self.index = if self.index == 0 {
            self.count - 1
        } else {
            self.index - 1
        };
    }

    /// Applies a relative encoder step, wrapping around.
    pub fn apply_encoder(&mut self, steps: i16) {
        if self.count == 0 || steps == 0 {
            return;
        }
        let count = self.count as i32;
        let next = (self.index as i32 + steps as i32).rem_euclid(count);
        self.index = next as u8;
    }

    /// Sets the focus directly, clamping to the item range.
    pub fn set_index(&mut self, index: u8) {
        if self.count == 0 {
            self.index = 0;
        } else {
            self.index = index.min(self.count - 1);
        }
    }

    /// Hit-tests a point against the arranged item rectangles.
    pub fn hit_test(self, rows: &[Rect], point: Point) -> Option<u8> {
        rows.iter()
            .position(|r| r.contains(point))
            .map(|i| i as u8)
            .filter(|i| *i < self.count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_at_zero() {
        let f = Focus::new(3);
        assert_eq!(f.index(), 0);
        assert_eq!(f.count(), 3);
    }

    #[test]
    fn next_wraps() {
        let mut f = Focus::new(3);
        f.next();
        assert_eq!(f.index(), 1);
        f.next();
        f.next();
        assert_eq!(f.index(), 0);
    }

    #[test]
    fn previous_wraps() {
        let mut f = Focus::new(3);
        f.previous();
        assert_eq!(f.index(), 2);
        f.previous();
        assert_eq!(f.index(), 1);
    }

    #[test]
    fn empty_focus_is_stable() {
        let mut f = Focus::new(0);
        f.next();
        f.previous();
        f.apply_encoder(5);
        f.set_index(3);
        assert_eq!(f.index(), 0);
        assert!(f.is_empty());
    }

    #[test]
    fn encoder_wraps_both_ways() {
        let mut f = Focus::new(4);
        f.apply_encoder(1);
        assert_eq!(f.index(), 1);
        f.apply_encoder(-2);
        assert_eq!(f.index(), 3);
        f.apply_encoder(5);
        assert_eq!(f.index(), 0);
    }

    #[test]
    fn set_count_clamps_index() {
        let mut f = Focus::new(4);
        f.set_index(3);
        f.set_count(2);
        assert_eq!(f.index(), 1);
        f.set_count(0);
        assert_eq!(f.index(), 0);
    }

    #[test]
    fn hit_test_returns_row() {
        let f = Focus::new(3);
        let rows = [
            Rect::new(0, 0, 100, 20),
            Rect::new(0, 20, 100, 20),
            Rect::new(0, 40, 100, 20),
        ];
        assert_eq!(f.hit_test(&rows, Point::new(50, 25)), Some(1));
        assert_eq!(f.hit_test(&rows, Point::new(50, 59)), Some(2));
        assert_eq!(f.hit_test(&rows, Point::new(150, 25)), None);
    }
}
