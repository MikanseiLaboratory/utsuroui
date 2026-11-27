//! Turning dirty rectangles into band transfers.
//!
//! Painting and sending are serial: each band is painted into a RAM buffer and
//! then awaited through the [`BandSink`]. True DMA/paint overlap is a later
//! optimisation (DESIGN.md section 2), but because every band boundary awaits,
//! the input task keeps running during a full-screen redraw.

use crate::app::App;
use crate::band::{BandSink, blend_band};
use crate::color::BLACK;
use crate::dirty::DirtyList;
use crate::geom::{Rect, Size};
use crate::paint::PaintCx;

/// Screen geometry and band height used by the flush loop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderCfg {
    /// The panel area, in screen coordinates.
    pub screen: Rect,
    /// Rows painted and sent per band (16 by default).
    pub band_h: u16,
}

impl RenderCfg {
    /// Creates a configuration.
    pub const fn new(screen: Rect, band_h: u16) -> Self {
        Self { screen, band_h }
    }

    /// The screen size.
    pub const fn screen_size(&self) -> Size {
        Size::new(self.screen.w, self.screen.h)
    }

    /// Bytes one band buffer must provide.
    pub const fn band_buffer_len(&self) -> usize {
        self.screen.w as usize * self.band_h as usize * 2
    }
}

/// Counters for one flush, useful for frame-budget tests and `defmt` logs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FlushStats {
    /// Number of dirty rectangles (1 for a full redraw).
    pub rects: u16,
    /// Number of band transfers.
    pub bands: u16,
    /// Bytes handed to the sink.
    pub bytes: u32,
}

/// Paints and sends everything the [`DirtyList`] asks for.
///
/// `band_a` is the paint buffer; `band_b` is only needed for crossfades (the
/// outgoing screen is painted there and blended). Both must be at least
/// [`RenderCfg::band_buffer_len`] bytes.
pub async fn flush<A, S>(
    app: &A,
    sink: &mut S,
    dirty: &DirtyList,
    now_us: u64,
    band_a: &mut [u8],
    mut band_b: Option<&mut [u8]>,
    cfg: &RenderCfg,
) -> Result<FlushStats, S::Error>
where
    A: App + ?Sized,
    S: BandSink + ?Sized,
{
    let mut stats = FlushStats::default();
    if dirty.is_full_redraw() {
        stats.rects = 1;
        flush_region(
            app,
            sink,
            cfg.screen,
            now_us,
            band_a,
            band_b.as_deref_mut(),
            cfg,
            &mut stats,
        )
        .await?;
    } else {
        let regions = dirty.as_slice();
        stats.rects = regions.len() as u16;
        for &region in regions {
            flush_region(
                app,
                sink,
                region,
                now_us,
                band_a,
                band_b.as_deref_mut(),
                cfg,
                &mut stats,
            )
            .await?;
        }
    }
    Ok(stats)
}

#[allow(clippy::too_many_arguments)]
async fn flush_region<A, S>(
    app: &A,
    sink: &mut S,
    region: Rect,
    now_us: u64,
    band_a: &mut [u8],
    mut band_b: Option<&mut [u8]>,
    cfg: &RenderCfg,
    stats: &mut FlushStats,
) -> Result<(), S::Error>
where
    A: App + ?Sized,
    S: BandSink + ?Sized,
{
    let screen = cfg.screen_size();
    let screen_rect = cfg.screen;
    let Some(region) = region.intersect(screen_rect) else {
        return Ok(());
    };

    for band in region.rows(cfg.band_h) {
        let len = band.area() as usize * 2;
        let crossfade = app.crossfade_alpha(now_us);

        match (crossfade, band_b.as_deref_mut()) {
            (Some(alpha), Some(under)) => {
                // Outgoing screen first, then the incoming one, then blend.
                {
                    let mut cx = PaintCx::new(&mut under[..], band, screen, band);
                    if !app.paint_under(&mut cx, now_us) {
                        cx.fill_rect(band, BLACK);
                    }
                }
                {
                    let mut cx = PaintCx::new(&mut band_a[..], band, screen, band);
                    app.paint(&mut cx, now_us);
                }
                blend_band(&mut band_a[..len], &under[..len], alpha);
            }
            _ => {
                let mut cx = PaintCx::new(&mut band_a[..], band, screen, band);
                app.paint(&mut cx, now_us);
            }
        }

        sink.write_band(band, &band_a[..len]).await?;
        stats.bands = stats.bands.saturating_add(1);
        stats.bytes = stats.bytes.saturating_add(len as u32);
    }
    Ok(())
}

/// Renders the whole screen into `buf` once, without a sink.
///
/// Used by the `full-fb` path and by host screenshot tools. Crossfades are not
/// supported here; use [`flush`] with two band buffers.
#[cfg(feature = "full-fb")]
pub fn render_full<A: App + ?Sized>(app: &A, buf: &mut [u8], cfg: &RenderCfg, now_us: u64) {
    let screen = cfg.screen_size();
    for band in cfg.screen.rows(cfg.band_h) {
        let mut cx = PaintCx::new_full(buf, cfg.screen.w as usize * 2, screen, band);
        app.paint(&mut cx, now_us);
    }
}

/// Sends a full-frame buffer band by band.
#[cfg(feature = "full-fb")]
pub async fn flush_full<S: BandSink + ?Sized>(
    sink: &mut S,
    buf: &[u8],
    cfg: &RenderCfg,
) -> Result<FlushStats, S::Error> {
    let mut stats = FlushStats::default();
    let row_bytes = cfg.screen.w as usize * 2;
    for band in cfg.screen.rows(cfg.band_h) {
        let start = band.y as usize * row_bytes;
        let end = start + band.h as usize * row_bytes;
        sink.write_band(band, &buf[start..end]).await?;
        stats.bands = stats.bands.saturating_add(1);
        stats.bytes = stats.bytes.saturating_add((end - start) as u32);
    }
    stats.rects = 1;
    Ok(stats)
}
