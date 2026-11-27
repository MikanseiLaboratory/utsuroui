//! Drawing into one band buffer.
//!
//! [`PaintCx`] is created by the flush loop for every band. All coordinates are
//! **screen coordinates**: the context translates them into the band buffer and
//! clips to the band (and to whatever clip rect widgets push). Widgets therefore
//! never need to know which stripe is currently being painted.

use core::convert::Infallible;

use embedded_graphics::Drawable;
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::{OriginDimensions, Point as EgPoint, Size as EgSize};
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::primitives::Rectangle;
use embedded_graphics::text::{Baseline, Text, TextStyle as EgTextStyle};

use crate::color::blend;
use crate::geom::{Point, Rect, Size};
use crate::text::{self as text_util, TextStyle};

/// Per-band drawing context.
///
/// The buffer holds big-endian RGB565 pixels, which is the order panels expect.
pub struct PaintCx<'a> {
    target: BandTarget<'a>,
}

struct BandTarget<'a> {
    buf: &'a mut [u8],
    /// Screen coordinate of buffer pixel `(0, 0)`.
    origin: Point,
    /// Bytes per buffer row.
    row_bytes: usize,
    /// Buffer extent in pixels (screen coordinates).
    band: Rect,
    screen: Size,
    clip: Rect,
}

impl PaintCx<'_> {
    /// The band currently being painted, in screen coordinates.
    pub fn band(&self) -> Rect {
        self.target.band
    }

    /// The active clip rectangle, in screen coordinates.
    pub fn clip(&self) -> Rect {
        self.target.clip
    }

    /// Tightens the clip rectangle (never widens it).
    pub fn set_clip(&mut self, clip: Rect) {
        if let Some(i) = clip.intersect(self.target.clip) {
            self.target.clip = i;
        } else if let Some(i) = clip.intersect(self.target.band) {
            self.target.clip = i;
        } else {
            self.target.clip = Rect::EMPTY;
        }
    }

    /// Runs `f` with a tightened clip, restoring the previous clip afterwards.
    pub fn with_clip<R>(&mut self, clip: Rect, f: impl FnOnce(&mut PaintCx<'_>) -> R) -> R {
        let saved = self.target.clip;
        self.set_clip(clip);
        let result = f(self);
        self.target.clip = saved;
        result
    }

    /// Fills `rect` with `color`.
    pub fn fill_rect(&mut self, rect: Rect, color: Rgb565) {
        self.target.fill_screen(rect, color);
    }

    /// Fills `rect` with a black overlay of `alpha` (`0..=255`).
    pub fn fill_rect_alpha(&mut self, rect: Rect, color: Rgb565, alpha: u8) {
        self.target.blend_screen(rect, color, alpha);
    }

    /// Draws a one-pixel rectangle outline, inside `rect`.
    pub fn stroke_rect(&mut self, rect: Rect, color: Rgb565) {
        if rect.is_empty() {
            return;
        }
        let right = rect.x + rect.w as i16 - 1;
        let bottom = rect.y + rect.h as i16 - 1;
        self.hline(rect.y, rect.x, right, color);
        if bottom != rect.y {
            self.hline(bottom, rect.x, right, color);
        }
        self.vline(rect.x, rect.y, bottom, color);
        if right != rect.x {
            self.vline(right, rect.y, bottom, color);
        }
    }

    /// Draws a one-pixel horizontal line from `x0` to `x1` (inclusive).
    pub fn hline(&mut self, y: i16, x0: i16, x1: i16, color: Rgb565) {
        if x1 < x0 {
            return;
        }
        self.target
            .fill_screen(Rect::new(x0, y, (x1 - x0) as u16 + 1, 1), color);
    }

    /// Draws a one-pixel vertical line from `y0` to `y1` (inclusive).
    pub fn vline(&mut self, x: i16, y0: i16, y1: i16, color: Rgb565) {
        if y1 < y0 {
            return;
        }
        self.target
            .fill_screen(Rect::new(x, y0, 1, (y1 - y0) as u16 + 1), color);
    }

    /// Draws `text` with its top-left corner at `pos`.
    pub fn text(&mut self, text: &str, pos: Point, style: TextStyle, color: Rgb565) {
        let character_style = MonoTextStyle::new(style.font(), color);
        let text_style = EgTextStyle::with_baseline(Baseline::Top);
        let _ = Text::with_text_style(text, pos.to_eg(), character_style, text_style)
            .draw(&mut self.target);
    }

    /// Draws `text` horizontally centred inside `area`.
    pub fn text_center(&mut self, text: &str, area: Rect, style: TextStyle, color: Rgb565) {
        let size = text_util::measure(text, style);
        let x = area.x as i32 + (area.w as i32 - size.w as i32) / 2;
        let y = area.y as i32 + (area.h as i32 - size.h as i32) / 2;
        self.text(
            text,
            Point::new(
                x.clamp(i16::MIN as i32, i16::MAX as i32) as i16,
                y.clamp(i16::MIN as i32, i16::MAX as i32) as i16,
            ),
            style,
            color,
        );
    }

    /// Draws `text` so that its right edge is at `right_x`.
    pub fn text_right(
        &mut self,
        text: &str,
        right_x: i16,
        y: i16,
        style: TextStyle,
        color: Rgb565,
    ) {
        let w = text_util::measure(text, style).w;
        self.text(text, Point::new(right_x - w as i16, y), style, color);
    }

    /// Draws `text` vertically centred inside `area`.
    pub fn text_middle(&mut self, text: &str, x: i16, area: Rect, style: TextStyle, color: Rgb565) {
        let h = text_util::measure(text, style).h;
        let y = area.y as i32 + (area.h as i32 - h as i32) / 2;
        self.text(
            text,
            Point::new(x, y.clamp(i16::MIN as i32, i16::MAX as i32) as i16),
            style,
            color,
        );
    }

    /// Draws text with an arbitrary `embedded-graphics` text renderer.
    ///
    /// Used by the optional U8g2 backend; `baseline` is normally
    /// [`Baseline::Top`].
    pub fn text_with<R>(&mut self, text: &str, pos: Point, renderer: R, baseline: Baseline)
    where
        R: embedded_graphics::text::renderer::TextRenderer<Color = Rgb565>,
    {
        let style = EgTextStyle::with_baseline(baseline);
        let _ = Text::with_text_style(text, pos.to_eg(), renderer, style).draw(&mut self.target);
    }

    /// Draws any `embedded-graphics` drawable in screen coordinates.
    pub fn draw<D: Drawable<Color = Rgb565>>(&mut self, drawable: &D) {
        let _ = drawable.draw(&mut self.target);
    }

    /// Creates a context over one band buffer.
    ///
    /// `clip` is intersected with `band`.
    pub(crate) fn new(buf: &mut [u8], band: Rect, screen: Size, clip: Rect) -> PaintCx<'_> {
        let need = band.area() as usize * 2;
        assert!(
            buf.len() >= need,
            "band buffer too small: need {need} bytes for {:?}, got {}",
            band,
            buf.len()
        );
        PaintCx {
            target: BandTarget {
                buf,
                origin: Point::new(band.x, band.y),
                row_bytes: band.w as usize * 2,
                band,
                screen,
                clip: clip.intersect(band).unwrap_or(Rect::EMPTY),
            },
        }
    }

    /// Creates a context over a full-frame buffer (used by the `full-fb` path).
    #[cfg(feature = "full-fb")]
    pub(crate) fn new_full(
        buf: &mut [u8],
        stride_bytes: usize,
        screen: Size,
        clip: Rect,
    ) -> PaintCx<'_> {
        let need = screen.area() as usize * 2;
        assert!(
            buf.len() >= need,
            "full frame buffer too small: need {need} bytes, got {}",
            buf.len()
        );
        PaintCx {
            target: BandTarget {
                buf,
                origin: Point::ZERO,
                row_bytes: stride_bytes,
                band: Rect::from_size(screen),
                screen,
                clip,
            },
        }
    }
}

impl BandTarget<'_> {
    #[inline]
    fn index(&self, x: i16, y: i16) -> Option<usize> {
        if x < self.band.x
            || y < self.band.y
            || x >= self.band.x + self.band.w as i16
            || y >= self.band.y + self.band.h as i16
        {
            return None;
        }
        let local_x = (x - self.origin.x) as usize;
        let local_y = (y - self.origin.y) as usize;
        let idx = local_y * self.row_bytes + local_x * 2;
        if idx + 2 <= self.buf.len() {
            Some(idx)
        } else {
            None
        }
    }

    #[inline]
    fn put(&mut self, x: i16, y: i16, color: Rgb565) {
        let Some(idx) = self.index(x, y) else {
            return;
        };
        let bytes = crate::color::be_bytes(color);
        self.buf[idx] = bytes[0];
        self.buf[idx + 1] = bytes[1];
    }

    fn fill_screen(&mut self, rect: Rect, color: Rgb565) {
        let Some(clipped) = rect
            .intersect(self.band)
            .and_then(|r| r.intersect(self.clip))
        else {
            return;
        };
        let bytes = crate::color::be_bytes(color);
        let x_start = (clipped.x - self.origin.x) as usize;
        let y_start = (clipped.y - self.origin.y) as usize;
        let width = clipped.w as usize;
        for row in 0..clipped.h as usize {
            let base = (y_start + row) * self.row_bytes + x_start * 2;
            let dst = &mut self.buf[base..base + width * 2];
            for px in dst.chunks_exact_mut(2) {
                px[0] = bytes[0];
                px[1] = bytes[1];
            }
        }
    }

    fn blend_screen(&mut self, rect: Rect, color: Rgb565, alpha: u8) {
        if alpha == 0 {
            return;
        }
        if alpha == 255 {
            self.fill_screen(rect, color);
            return;
        }
        let Some(clipped) = rect
            .intersect(self.band)
            .and_then(|r| r.intersect(self.clip))
        else {
            return;
        };
        let x_start = (clipped.x - self.origin.x) as usize;
        let y_start = (clipped.y - self.origin.y) as usize;
        for row in 0..clipped.h as usize {
            let base = (y_start + row) * self.row_bytes + x_start * 2;
            let dst = &mut self.buf[base..base + clipped.w as usize * 2];
            for px in dst.chunks_exact_mut(2) {
                let under = crate::color::from_raw_u16(u16::from_be_bytes([px[0], px[1]]));
                let out = crate::color::be_bytes(blend(color, under, alpha));
                px[0] = out[0];
                px[1] = out[1];
            }
        }
    }
}

impl OriginDimensions for BandTarget<'_> {
    fn size(&self) -> EgSize {
        self.screen.to_eg()
    }
}

impl DrawTarget for BandTarget<'_> {
    type Color = Rgb565;
    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = embedded_graphics::Pixel<Rgb565>>,
    {
        for embedded_graphics::Pixel(coord, color) in pixels {
            let x = coord.x.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
            let y = coord.y.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
            if !self.clip.contains(Point::new(x, y)) {
                continue;
            }
            self.put(x, y, color);
        }
        Ok(())
    }

    fn fill_solid(&mut self, area: &Rectangle, color: Self::Color) -> Result<(), Self::Error> {
        self.fill_screen(Rect::from_eg(*area), color);
        Ok(())
    }

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        self.fill_screen(self.clip, color);
        Ok(())
    }
}

impl core::fmt::Debug for PaintCx<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PaintCx")
            .field("band", &self.target.band)
            .field("clip", &self.target.clip)
            .finish()
    }
}

/// Helper so callers do not need to import `embedded-graphics` geometry types.
pub fn eg_point(x: i16, y: i16) -> EgPoint {
    EgPoint::new(x as i32, y as i32)
}
