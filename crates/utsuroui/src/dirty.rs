//! Dirty-rectangle accumulation.
//!
//! A frame only sends the rectangles that changed. Rectangles that intersect
//! are always merged; rectangles separated by at most one pixel are merged only
//! when the merged area is at most 1.5x the sum of the two areas (sending empty
//! pixels costs SPI time). If the list is full and no cheap merge exists, the
//! list falls back to a full-screen redraw.

use crate::geom::Rect;

/// Default number of rectangles tracked per frame.
pub const MAX_DIRTY: usize = 8;

/// Merged area must satisfy `U * UNION_WASTE_DEN <= S * UNION_WASTE_NUM`.
const UNION_WASTE_NUM: u32 = 3;
const UNION_WASTE_DEN: u32 = 2;

/// The default dirty list (eight rectangles).
///
/// The alias exists because a plain `DirtyList::new(..)` cannot infer the
/// const generic parameter.
pub type Dirty = DirtyList<MAX_DIRTY>;

/// A fixed-capacity list of dirty rectangles.
#[derive(Clone, Debug)]
pub struct DirtyList<const N: usize = MAX_DIRTY> {
    rects: [Rect; N],
    len: usize,
    full: bool,
    screen: Rect,
}

impl<const N: usize> DirtyList<N> {
    /// Creates an empty list clipped to `screen`.
    pub const fn new(screen: Rect) -> Self {
        Self {
            rects: [Rect::EMPTY; N],
            len: 0,
            full: false,
            screen,
        }
    }

    /// The screen rect that all additions are clipped to.
    pub const fn screen(&self) -> Rect {
        self.screen
    }

    /// Empties the list.
    pub fn clear(&mut self) {
        self.len = 0;
        self.full = false;
    }

    /// Marks the whole screen dirty.
    pub fn mark_full(&mut self) {
        self.len = 0;
        self.full = true;
    }

    /// Whether the next flush must redraw the whole screen.
    pub const fn is_full_redraw(&self) -> bool {
        self.full
    }

    /// Whether nothing needs redrawing.
    pub const fn is_empty(&self) -> bool {
        !self.full && self.len == 0
    }

    /// Number of tracked rectangles (zero when a full redraw is pending).
    pub const fn len(&self) -> usize {
        self.len
    }

    /// The tracked rectangles; empty when a full redraw is pending.
    pub fn as_slice(&self) -> &[Rect] {
        if self.full {
            &[]
        } else {
            &self.rects[..self.len]
        }
    }

    /// Adds a rectangle, merging and clipping as needed.
    pub fn add(&mut self, rect: Rect) {
        if self.full || rect.is_empty() {
            return;
        }
        let Some(mut r) = rect.intersect(self.screen) else {
            return;
        };

        // Merge repeatedly: each merge removes one slot or replaces one entry,
        // so this terminates in at most N rounds.
        loop {
            let mut merged = false;
            let mut i = 0;
            while i < self.len {
                if should_merge(self.rects[i], r) {
                    r = self.rects[i].union(r);
                    self.remove(i);
                    merged = true;
                    break;
                }
                i += 1;
            }
            if merged {
                continue;
            }

            if self.len < N {
                self.rects[self.len] = r;
                self.len += 1;
                return;
            }

            // Full: force-merge with the cheapest partner, or redraw everything.
            match best_forced_merge(&self.rects[..self.len], r) {
                Some(i) => {
                    r = self.rects[i].union(r);
                    self.remove(i);
                }
                None => {
                    self.mark_full();
                    return;
                }
            }
        }
    }

    /// Adds the union of two rectangles.
    pub fn add_union(&mut self, a: Rect, b: Rect) {
        self.add(a.union(b));
    }

    fn remove(&mut self, i: usize) {
        self.len -= 1;
        // Swap-remove keeps the list compact; draw order of dirty rects does
        // not matter because every rect is painted independently.
        if i < self.len {
            self.rects[i] = self.rects[self.len];
        }
    }
}

fn should_merge(a: Rect, b: Rect) -> bool {
    if a.intersect(b).is_some() {
        return true;
    }
    const GAP: i32 = 1;
    let horizontal = a.right() + GAP >= b.x as i32
        && b.right() + GAP >= a.x as i32
        && ranges_close(a.y as i32, a.bottom(), b.y as i32, b.bottom(), GAP);
    let vertical = a.bottom() + GAP >= b.y as i32
        && b.bottom() + GAP >= a.y as i32
        && ranges_close(a.x as i32, a.right(), b.x as i32, b.right(), GAP);
    (horizontal || vertical) && union_is_cheap(a, b)
}

fn ranges_close(a0: i32, a1: i32, b0: i32, b1: i32, gap: i32) -> bool {
    a1 + gap >= b0 && b1 + gap >= a0
}

fn union_is_cheap(a: Rect, b: Rect) -> bool {
    let union = a.union(b).area();
    let sum = a.area() + b.area();
    union.saturating_mul(UNION_WASTE_DEN) <= sum.saturating_mul(UNION_WASTE_NUM)
}

fn best_forced_merge(rects: &[Rect], r: Rect) -> Option<usize> {
    let mut best: Option<(usize, u32)> = None;
    for (i, a) in rects.iter().enumerate() {
        let union = a.union(r).area();
        let sum = a.area() + r.area();
        let acceptable = union < sum || union_is_cheap(*a, r);
        if !acceptable {
            continue;
        }
        let waste = union.saturating_sub(sum);
        if best.is_none_or(|(_, w)| waste < w) {
            best = Some((i, waste));
        }
    }
    best.map(|(i, _)| i)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Rect = Rect::new(0, 0, 240, 320);

    #[test]
    fn empty_initially() {
        let d: DirtyList = DirtyList::new(SCREEN);
        assert!(d.is_empty());
        assert!(!d.is_full_redraw());
        assert_eq!(d.as_slice().len(), 0);
    }

    #[test]
    fn empty_rect_is_ignored() {
        let mut d: DirtyList = DirtyList::new(SCREEN);
        d.add(Rect::EMPTY);
        assert!(d.is_empty());
    }

    #[test]
    fn off_screen_is_clipped_away() {
        let mut d: DirtyList = DirtyList::new(SCREEN);
        d.add(Rect::new(300, 0, 10, 10));
        assert!(d.is_empty());
    }

    #[test]
    fn partially_off_screen_is_clipped() {
        let mut d: DirtyList = DirtyList::new(SCREEN);
        d.add(Rect::new(220, 300, 40, 40));
        assert_eq!(d.as_slice(), &[Rect::new(220, 300, 20, 20)]);
    }

    #[test]
    fn intersecting_rects_merge() {
        let mut d: DirtyList = DirtyList::new(SCREEN);
        d.add(Rect::new(0, 0, 20, 20));
        d.add(Rect::new(10, 10, 20, 20));
        assert_eq!(d.len(), 1);
        assert_eq!(d.as_slice()[0], Rect::new(0, 0, 30, 30));
    }

    #[test]
    fn cheap_adjacent_merge() {
        let mut d: DirtyList = DirtyList::new(SCREEN);
        // Same row, 30 px wide each, merged to 60 -> union == sum, cheap.
        d.add(Rect::new(0, 0, 30, 20));
        d.add(Rect::new(31, 0, 30, 20));
        assert_eq!(d.len(), 1);
    }

    #[test]
    fn expensive_adjacent_merge_is_rejected() {
        let mut d: DirtyList = DirtyList::new(SCREEN);
        // Vertical slivers far apart: union has a huge empty middle.
        d.add(Rect::new(0, 0, 20, 10));
        d.add(Rect::new(0, 300, 20, 10));
        assert_eq!(d.len(), 2);
    }

    #[test]
    fn far_apart_rects_stay_separate() {
        let mut d: DirtyList = DirtyList::new(SCREEN);
        d.add(Rect::new(0, 0, 10, 10));
        d.add(Rect::new(100, 100, 10, 10));
        assert_eq!(d.len(), 2);
    }

    #[test]
    fn full_screen_overflow_falls_back_to_full_redraw() {
        let mut d: DirtyList = DirtyList::new(SCREEN);
        // Eight tiny rects that are far apart: no merge possible.
        for i in 0..8u16 {
            d.add(Rect::new(i as i16 * 28, i as i16 * 36, 10, 10));
        }
        assert_eq!(d.len(), 8);
        assert!(!d.is_full_redraw());
        d.add(Rect::new(220, 310, 10, 8));
        assert!(d.is_full_redraw());
        assert!(d.as_slice().is_empty());
    }

    #[test]
    fn overflow_forces_a_cheap_merge_when_available() {
        let mut d: DirtyList = DirtyList::new(SCREEN);
        for i in 0..8u16 {
            d.add(Rect::new(i as i16 * 30, 0, 10, 10));
        }
        assert_eq!(d.len(), 8);
        // The list is full; this rect is adjacent to the last one, so the
        // renderer merges it instead of falling back to a full redraw.
        d.add(Rect::new(220, 0, 10, 10));
        assert_eq!(d.len(), 8);
        assert!(!d.is_full_redraw());
    }

    #[test]
    fn mark_full_and_clear() {
        let mut d: DirtyList = DirtyList::new(SCREEN);
        d.mark_full();
        assert!(d.is_full_redraw());
        assert!(!d.is_empty());
        d.add(Rect::new(0, 0, 10, 10));
        assert!(d.is_full_redraw());
        d.clear();
        assert!(d.is_empty());
        assert!(!d.is_full_redraw());
    }

    #[test]
    fn union_helper() {
        let mut d: DirtyList = DirtyList::new(SCREEN);
        d.add_union(Rect::new(0, 0, 10, 10), Rect::new(20, 20, 10, 10));
        assert_eq!(d.as_slice(), &[Rect::new(0, 0, 30, 30)]);
    }

    #[test]
    fn many_small_rects_never_exceed_capacity() {
        let mut d: DirtyList = DirtyList::new(SCREEN);
        for i in 0..50i16 {
            d.add(Rect::new((i * 7) % 220, (i * 11) % 300, 8, 8));
            assert!(d.len() <= MAX_DIRTY);
        }
    }
}
