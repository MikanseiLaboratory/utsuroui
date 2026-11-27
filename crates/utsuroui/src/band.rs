//! Band buffers and the sink that receives finished bands.
//!
//! The renderer paints into a band buffer and hands the bytes to a
//! [`BandSink`]. On hardware that sink is an `lcd-async` display; on the host it
//! is an in-memory frame buffer. Keeping the sink behind a trait is what makes
//! the whole renderer testable without a panel.

use core::convert::Infallible;
use core::future::Future;

use crate::color::{Rgb565, raw_u16};
use crate::geom::Rect;

/// A destination for finished RGB565 bands.
///
/// This is the only boundary between the renderer and the hardware. The
/// renderer never names a bus, a DMA channel or a HAL: it paints one band into
/// RAM and awaits [`BandSink::write_band`]. Implement it for an SPI panel
/// (the bundled `lcd` adapter for `lcd-async`, which drives the transfer with
/// DMA inside the HAL), for a parallel 8080 bus, for a USB display, or for an
/// in-memory buffer like [`MemorySink`].
///
/// # Contract
///
/// * `data` is `band.w * band.h * 2` bytes of big-endian RGB565, row-major,
///   exactly covering `band`.
/// * The future must not resolve until `data` may be overwritten again. A sink
///   that hands the buffer to a DMA engine therefore waits for the transfer to
///   finish (that is what `lcd-async` does), or copies the band first.
/// * `write_band` is awaited serially, once per band; painting the next band
///   starts only after it resolves.
pub trait BandSink {
    /// Transport error type.
    type Error: core::fmt::Debug;

    /// Sends `data` (big-endian RGB565, `band.w * band.h * 2` bytes) to `band`.
    ///
    /// Note: no `Send` bound. Embassy 0.10's thread-mode `Spawner` accepts
    /// non-`Send` tasks, and `lcd-async` 0.1.3's interface futures are not
    /// declared `Send`, so requiring it would make the adapter unusable.
    fn write_band(
        &mut self,
        band: Rect,
        data: &[u8],
    ) -> impl Future<Output = Result<(), Self::Error>>;
}

impl<T: BandSink + ?Sized> BandSink for &mut T {
    type Error = T::Error;

    async fn write_band(&mut self, band: Rect, data: &[u8]) -> Result<(), Self::Error> {
        T::write_band(self, band, data).await
    }
}

/// A [`BandSink`] that copies bands into a full-frame RGB565 buffer.
///
/// Used by the host renderer, golden-image tests and `no_std` snapshot code.
pub struct MemorySink<'a> {
    buf: &'a mut [u8],
    width: u16,
}

impl<'a> MemorySink<'a> {
    /// Creates a sink over a full-frame buffer of `width` pixels per row.
    pub fn new(buf: &'a mut [u8], width: u16) -> Self {
        assert!(
            buf.len() >= width as usize * 2,
            "memory sink buffer must hold at least one row"
        );
        Self { buf, width }
    }

    /// Borrows the underlying buffer.
    pub fn as_slice(&self) -> &[u8] {
        self.buf
    }
}

impl BandSink for MemorySink<'_> {
    type Error = Infallible;

    async fn write_band(&mut self, band: Rect, data: &[u8]) -> Result<(), Self::Error> {
        let row_bytes = band.w as usize * 2;
        let stride = self.width as usize * 2;
        for row in 0..band.h as usize {
            let src = &data[row * row_bytes..(row + 1) * row_bytes];
            let dst_y = band.y as usize + row;
            let dst_start = dst_y * stride + band.x as usize * 2;
            let dst = &mut self.buf[dst_start..dst_start + row_bytes];
            dst.copy_from_slice(src);
        }
        Ok(())
    }
}

/// Writes one big-endian RGB565 pixel into `buf` at `index`.
#[inline]
pub fn write_px(buf: &mut [u8], index: usize, color: Rgb565) {
    let bytes = crate::color::be_bytes(color);
    buf[index] = bytes[0];
    buf[index + 1] = bytes[1];
}

/// Reads one big-endian RGB565 pixel from `buf` at `index`.
#[inline]
pub fn read_px(buf: &[u8], index: usize) -> Rgb565 {
    crate::color::from_raw_u16(u16::from_be_bytes([buf[index], buf[index + 1]]))
}

/// Blends the `under` band into the `over` band with `alpha` (`0..=255`).
///
/// * `alpha == 255` keeps `over` untouched (the new screen is fully visible).
/// * `alpha == 0` copies `under` into `over` (the outgoing screen is fully
///   visible).
///
/// Both slices must contain the same number of big-endian RGB565 pixels.
pub fn blend_band(over: &mut [u8], under: &[u8], alpha: u8) {
    if alpha == 255 {
        return;
    }
    if alpha == 0 {
        over.copy_from_slice(&under[..over.len()]);
        return;
    }
    let pixels = over.len().min(under.len()) / 2;
    for i in 0..pixels {
        let idx = i * 2;
        let top = read_px(over, idx);
        let bottom = read_px(under, idx);
        write_px(over, idx, crate::color::blend(top, bottom, alpha));
    }
}

/// The raw u16 value of a colour, re-exported for buffer tooling.
pub fn raw(color: Rgb565) -> u16 {
    raw_u16(color)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::{BLACK, WHITE, YELLOW};

    fn frame(_w: u16, _h: u16) -> [u8; 8 * 4 * 2] {
        [0u8; 8 * 4 * 2]
    }

    #[test]
    fn memory_sink_places_band_at_offsets() {
        let mut buf = frame(8, 4);
        let mut sink = MemorySink::new(&mut buf, 8);
        let band = Rect::new(2, 1, 3, 2);
        let mut data = [0u8; 3 * 2 * 2];
        for (i, px) in data.chunks_mut(2).enumerate() {
            write_px(px, 0, if i % 2 == 0 { WHITE } else { YELLOW });
        }
        pollster::block_on(sink.write_band(band, &data)).unwrap();
        assert_eq!(read_px(&buf, (8 + 2) * 2), WHITE);
        assert_eq!(read_px(&buf, (8 + 3) * 2), YELLOW);
        assert_eq!(read_px(&buf, (2 * 8 + 4) * 2), YELLOW);
        // Outside the band is untouched.
        assert_eq!(read_px(&buf, 0), BLACK);
        assert_eq!(read_px(&buf, (2 * 8 + 5) * 2), BLACK);
    }

    #[test]
    fn write_read_px_round_trips() {
        let mut buf = [0u8; 4];
        write_px(&mut buf, 0, YELLOW);
        write_px(&mut buf, 2, WHITE);
        assert_eq!(buf[0], 0xFF);
        assert_eq!(buf[1], 0xE0);
        assert_eq!(read_px(&buf, 0), YELLOW);
        assert_eq!(read_px(&buf, 2), WHITE);
        assert_eq!(raw(YELLOW), 0xFFE0);
    }

    #[test]
    fn blend_band_extremes() {
        let under = [0xFF, 0xE0, 0x00, 0x00];
        let mut over = [0x12, 0x34, 0x56, 0x78];
        blend_band(&mut over, &under, 255);
        assert_eq!(over, [0x12, 0x34, 0x56, 0x78]);

        let mut over2 = [0x12, 0x34, 0x56, 0x78];
        blend_band(&mut over2, &under, 0);
        assert_eq!(over2, under);
    }

    #[test]
    fn blend_band_midpoint() {
        let mut over = [0x00, 0x00, 0x00, 0x00]; // black incoming screen
        let under = [0xFF, 0xFF, 0xFF, 0xFF]; // white outgoing screen
        blend_band(&mut over, &under, 128);
        assert_eq!(read_px(&over, 0), crate::color::blend(BLACK, WHITE, 128));
    }
}
