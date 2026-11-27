//! Integer geometry for the UI.
//!
//! The framework deliberately uses `i16`/`u16` coordinates instead of the
//! `i32`/`u32` of `embedded-graphics`: on a small core every multiplication and
//! clamp matters, and a 240x320 panel never needs more than a few hundred
//! pixels. Conversion to `embedded-graphics` types happens at the draw calls.

use embedded_graphics::geometry::Point as EgPoint;
use embedded_graphics::geometry::Size as EgSize;
use embedded_graphics::primitives::Rectangle as EgRectangle;

/// A point in screen coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Point {
    /// Horizontal coordinate.
    pub x: i16,
    /// Vertical coordinate.
    pub y: i16,
}

impl Point {
    /// The origin.
    pub const ZERO: Self = Self::new(0, 0);

    /// Creates a point.
    pub const fn new(x: i16, y: i16) -> Self {
        Self { x, y }
    }

    /// Translates the point by `(dx, dy)`, saturating at the `i16` bounds.
    pub const fn translate(self, dx: i16, dy: i16) -> Self {
        Self {
            x: self.x.saturating_add(dx),
            y: self.y.saturating_add(dy),
        }
    }

    /// Converts to an `embedded-graphics` point.
    pub const fn to_eg(self) -> EgPoint {
        EgPoint::new(self.x as i32, self.y as i32)
    }

    /// Converts from an `embedded-graphics` point, saturating.
    pub fn from_eg(p: EgPoint) -> Self {
        Self {
            x: p.x.clamp(i16::MIN as i32, i16::MAX as i32) as i16,
            y: p.y.clamp(i16::MIN as i32, i16::MAX as i32) as i16,
        }
    }
}

/// A width/height pair in pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Size {
    /// Width in pixels.
    pub w: u16,
    /// Height in pixels.
    pub h: u16,
}

impl Size {
    /// A zero size.
    pub const ZERO: Self = Self::new(0, 0);

    /// Creates a size.
    pub const fn new(w: u16, h: u16) -> Self {
        Self { w, h }
    }

    /// Number of pixels covered by the size.
    pub const fn area(self) -> u32 {
        self.w as u32 * self.h as u32
    }

    /// Shrinks the size to at most `other` on both axes.
    pub const fn min(self, other: Self) -> Self {
        Self {
            w: if self.w < other.w { self.w } else { other.w },
            h: if self.h < other.h { self.h } else { other.h },
        }
    }

    /// Converts to an `embedded-graphics` size.
    pub const fn to_eg(self) -> EgSize {
        EgSize::new(self.w as u32, self.h as u32)
    }
}

/// An axis-aligned rectangle in screen coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Rect {
    /// Left edge.
    pub x: i16,
    /// Top edge.
    pub y: i16,
    /// Width in pixels.
    pub w: u16,
    /// Height in pixels.
    pub h: u16,
}

impl Rect {
    /// An empty rectangle at the origin.
    pub const EMPTY: Self = Self::new(0, 0, 0, 0);

    /// Creates a rectangle.
    pub const fn new(x: i16, y: i16, w: u16, h: u16) -> Self {
        Self { x, y, w, h }
    }

    /// Creates a rectangle at the origin with the given size.
    pub const fn from_size(size: Size) -> Self {
        Self::new(0, 0, size.w, size.h)
    }

    /// Creates a rectangle from two corners (top-left inclusive, bottom-right exclusive).
    pub const fn from_corners(left: i16, top: i16, right: i16, bottom: i16) -> Self {
        let w = right - left;
        let h = bottom - top;
        if w <= 0 || h <= 0 {
            return Self::EMPTY;
        }
        Self::new(left, top, w as u16, h as u16)
    }

    /// Whether the rectangle covers no pixels.
    pub const fn is_empty(self) -> bool {
        self.w == 0 || self.h == 0
    }

    /// Number of pixels covered.
    pub const fn area(self) -> u32 {
        self.w as u32 * self.h as u32
    }

    /// Right edge (exclusive).
    pub const fn right(self) -> i32 {
        self.x as i32 + self.w as i32
    }

    /// Bottom edge (exclusive).
    pub const fn bottom(self) -> i32 {
        self.y as i32 + self.h as i32
    }

    /// Center point, truncated.
    pub const fn center(self) -> Point {
        Point::new(self.x + (self.w / 2) as i16, self.y + (self.h / 2) as i16)
    }

    /// The intersection of two rectangles, if it covers at least one pixel.
    pub fn intersect(self, other: Self) -> Option<Self> {
        if self.is_empty() || other.is_empty() {
            return None;
        }
        let x0 = max_i32(self.x as i32, other.x as i32);
        let y0 = max_i32(self.y as i32, other.y as i32);
        let x1 = min_i32(self.right(), other.right());
        let y1 = min_i32(self.bottom(), other.bottom());
        if x1 <= x0 || y1 <= y0 {
            return None;
        }
        Some(Self::new(
            x0 as i16,
            y0 as i16,
            (x1 - x0) as u16,
            (y1 - y0) as u16,
        ))
    }

    /// The smallest rectangle covering both inputs (empty inputs are ignored).
    pub fn union(self, other: Self) -> Self {
        if self.is_empty() {
            return other;
        }
        if other.is_empty() {
            return self;
        }
        let x0 = min_i32(self.x as i32, other.x as i32);
        let y0 = min_i32(self.y as i32, other.y as i32);
        let x1 = max_i32(self.right(), other.right());
        let y1 = max_i32(self.bottom(), other.bottom());
        Self::new(x0 as i16, y0 as i16, (x1 - x0) as u16, (y1 - y0) as u16)
    }

    /// Whether the point lies inside the rectangle.
    pub fn contains(self, p: Point) -> bool {
        let x = p.x as i32;
        let y = p.y as i32;
        x >= self.x as i32 && x < self.right() && y >= self.y as i32 && y < self.bottom()
    }

    /// Translates the rectangle, saturating at the `i16` bounds.
    pub const fn translate(self, dx: i16, dy: i16) -> Self {
        Self {
            x: self.x.saturating_add(dx),
            y: self.y.saturating_add(dy),
            w: self.w,
            h: self.h,
        }
    }

    /// Shrinks the rectangle by `n` pixels on every side.
    pub fn inset(self, n: u16) -> Self {
        let twice = n as u32 * 2;
        if twice >= self.w as u32 || twice >= self.h as u32 {
            return Self::EMPTY;
        }
        Self::new(
            self.x + n as i16,
            self.y + n as i16,
            self.w - n * 2,
            self.h - n * 2,
        )
    }

    /// Splits the rectangle into horizontal bands of at most `band_h` rows.
    pub fn rows(self, band_h: u16) -> RowBands {
        RowBands {
            rect: self,
            band_h: band_h.max(1),
            y: self.y,
        }
    }

    /// Converts to an `embedded-graphics` rectangle.
    pub const fn to_eg(self) -> EgRectangle {
        EgRectangle::new(
            EgPoint::new(self.x as i32, self.y as i32),
            EgSize::new(self.w as u32, self.h as u32),
        )
    }

    /// Converts from an `embedded-graphics` rectangle, clipping to the `i16` range.
    pub fn from_eg(r: EgRectangle) -> Self {
        let x = r.top_left.x.clamp(-32000, 32000) as i16;
        let y = r.top_left.y.clamp(-32000, 32000) as i16;
        let w = r.size.width.min(u16::MAX as u32) as u16;
        let h = r.size.height.min(u16::MAX as u32) as u16;
        Self::new(x, y, w, h)
    }
}

/// Iterator over horizontal bands produced by [`Rect::rows`].
#[derive(Clone, Copy, Debug)]
pub struct RowBands {
    rect: Rect,
    band_h: u16,
    y: i16,
}

impl Iterator for RowBands {
    type Item = Rect;

    fn next(&mut self) -> Option<Rect> {
        if self.rect.is_empty() {
            return None;
        }
        let remaining = self.rect.bottom() - self.y as i32;
        if remaining <= 0 {
            return None;
        }
        let h = (remaining as u32).min(self.band_h as u32) as u16;
        let band = Rect::new(self.rect.x, self.y, self.rect.w, h);
        self.y = self.y.saturating_add(h as i16);
        Some(band)
    }
}

const fn min_i32(a: i32, b: i32) -> i32 {
    if a < b { a } else { b }
}

const fn max_i32(a: i32, b: i32) -> i32 {
    if a > b { a } else { b }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intersect_and_union() {
        let a = Rect::new(0, 0, 10, 10);
        let b = Rect::new(5, 5, 10, 10);
        assert_eq!(a.intersect(b), Some(Rect::new(5, 5, 5, 5)));
        assert_eq!(a.union(b), Rect::new(0, 0, 15, 15));
    }

    #[test]
    fn touching_rects_do_not_intersect() {
        let a = Rect::new(0, 0, 10, 10);
        let b = Rect::new(10, 0, 5, 10);
        assert_eq!(a.intersect(b), None);
        assert_eq!(a.union(b), Rect::new(0, 0, 15, 10));
    }

    #[test]
    fn empty_inputs() {
        let a = Rect::new(0, 0, 10, 10);
        assert!(Rect::EMPTY.is_empty());
        assert_eq!(a.union(Rect::EMPTY), a);
        assert_eq!(a.intersect(Rect::EMPTY), None);
    }

    #[test]
    fn negative_coordinates() {
        let r = Rect::new(-20, -10, 30, 30);
        assert_eq!(r.right(), 10);
        assert_eq!(r.bottom(), 20);
        assert!(r.contains(Point::new(-20, -10)));
        assert!(r.contains(Point::new(9, 19)));
        assert!(!r.contains(Point::new(10, 20)));
        assert!(!r.contains(Point::new(-21, 0)));
    }

    #[test]
    fn rows_split_evenly_and_remainder() {
        let r = Rect::new(3, 5, 40, 20);
        let bands: Vec<_> = r.rows(16).collect();
        assert_eq!(bands.len(), 2);
        assert_eq!(bands[0], Rect::new(3, 5, 40, 16));
        assert_eq!(bands[1], Rect::new(3, 21, 40, 4));
    }

    #[test]
    fn rows_of_empty_rect_is_empty() {
        assert_eq!(Rect::new(0, 0, 0, 10).rows(16).count(), 0);
        assert_eq!(Rect::new(0, 0, 10, 0).rows(16).count(), 0);
    }

    #[test]
    fn inset_and_center() {
        let r = Rect::new(0, 0, 100, 40);
        assert_eq!(r.inset(2), Rect::new(2, 2, 96, 36));
        assert_eq!(r.inset(20), Rect::EMPTY);
        assert_eq!(Rect::new(0, 0, 11, 11).center(), Point::new(5, 5));
    }

    #[test]
    fn eg_round_trip() {
        let r = Rect::new(10, 20, 30, 40);
        assert_eq!(Rect::from_eg(r.to_eg()), r);
        assert_eq!(Point::from_eg(Point::new(-3, 4).to_eg()), Point::new(-3, 4));
    }

    #[test]
    fn from_corners_clamps_reversed_input() {
        assert_eq!(Rect::from_corners(10, 10, 5, 5), Rect::EMPTY);
        assert_eq!(Rect::from_corners(0, 0, 10, 4), Rect::new(0, 0, 10, 4));
    }
}
