//! Turning dirty rectangles into band transfers.
//!
//! Painting and sending are serial: each band is painted into a RAM buffer and
//! then awaited through the [`BandSink`]. True DMA/paint overlap is a later
//! optimisation (DESIGN.md section 2), but because every band boundary awaits,
//! the input task keeps running during a full-screen redraw.

use crate::app::App;
use crate::band::{BandBuffers, BandSink, blend_band};
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
/// Paint and transfer share the band buffers: the band being sent and the band
/// being painted live in different buffers, so a sink that hands the buffer to a
/// DMA engine lets the CPU fill the next band while the panel receives the
/// current one. On firmware use [`flush_pipelined`] (the `embassy` feature); this
/// serial form is what the host renderer and non-Embassy users call.
pub async fn flush<A, S, const LEN: usize>(
    app: &A,
    sink: &mut S,
    dirty: &DirtyList,
    now_us: u64,
    buffers: &mut BandBuffers<LEN>,
    cfg: &RenderCfg,
) -> Result<FlushStats, S::Error>
where
    A: App + ?Sized,
    S: BandSink + ?Sized,
{
    let (band_a, _band_b, under) = buffers.split();
    let mut stats = FlushStats {
        rects: dirty_rects(dirty),
        ..FlushStats::default()
    };
    for band in bands(dirty, cfg) {
        let len = paint_band(app, band, cfg, now_us, band_a, Some(under));
        sink.write_band(band, &band_a[..len]).await?;
        stats.bands = stats.bands.saturating_add(1);
        stats.bytes = stats.bytes.saturating_add(len as u32);
    }
    Ok(stats)
}

/// Paints and sends with paint/transfer overlap.
///
/// Two futures run in one task: the renderer fills a free band buffer, and the
/// transport awaits [`BandSink::write_band`] on the filled one. While the sink
/// waits (a DMA transfer), the renderer keeps painting into the other buffer.
///
/// A two-slot hand-off is enough: with two buffers the renderer can be at most
/// one band ahead, and the transport is never starved.
#[cfg(feature = "pipeline")]
pub async fn flush_pipelined<A, S, const LEN: usize>(
    app: &A,
    sink: &mut S,
    dirty: &DirtyList,
    now_us: u64,
    buffers: &mut BandBuffers<LEN>,
    cfg: &RenderCfg,
) -> Result<FlushStats, S::Error>
where
    A: App + ?Sized,
    S: BandSink + ?Sized,
{
    use core::cell::Cell;

    use embassy_futures::join::join;
    use embassy_sync::blocking_mutex::raw::NoopRawMutex;
    use embassy_sync::channel::Channel;

    let (band_a, band_b, under) = buffers.split();
    let free: Channel<NoopRawMutex, &mut [u8], 2> = Channel::new();
    let work: Channel<NoopRawMutex, Option<(Rect, &mut [u8])>, 2> = Channel::new();
    let error: Cell<Option<S::Error>> = Cell::new(None);

    free.send(band_a).await;
    free.send(band_b).await;

    let render = async {
        let mut stats = FlushStats {
            rects: dirty_rects(dirty),
            ..FlushStats::default()
        };
        for band in bands(dirty, cfg) {
            let mut buf = free.receive().await;
            let len = paint_band(app, band, cfg, now_us, &mut buf, Some(&mut *under));
            stats.bands = stats.bands.saturating_add(1);
            stats.bytes = stats.bytes.saturating_add(len as u32);
            work.send(Some((band, buf))).await;
        }
        // Sentinel: the transport stops once the queue is drained.
        work.send(None).await;
        stats
    };

    let transport = async {
        let mut failed = false;
        while let Some((band, buf)) = work.receive().await {
            if !failed {
                let len = band.area() as usize * 2;
                if let Err(err) = sink.write_band(band, &buf[..len]).await {
                    error.set(Some(err));
                    failed = true;
                }
            }
            // Always hand the buffer back, even after a failure, so the
            // renderer cannot block forever.
            free.send(buf).await;
        }
    };

    let (stats, ()) = join(render, transport).await;
    match error.take() {
        Some(err) => Err(err),
        None => Ok(stats),
    }
}

/// Paints one band into `out`, returns the byte length written.
///
/// Shared by the serial and the pipelined path so both produce identical
/// pixels. When the app reports a crossfade the outgoing screen is painted into
/// `under` and blended in.
fn paint_band<A>(
    app: &A,
    band: Rect,
    cfg: &RenderCfg,
    now_us: u64,
    out: &mut [u8],
    under: Option<&mut [u8]>,
) -> usize
where
    A: App + ?Sized,
{
    let len = band.area() as usize * 2;
    assert!(
        out.len() >= len,
        "band buffer too small: need {len} bytes for {band:?}, got {}",
        out.len()
    );
    let screen = cfg.screen_size();

    match (app.crossfade_alpha(now_us), under) {
        (Some(alpha), Some(under)) => {
            // Outgoing screen first, then the incoming one, then blend.
            {
                let mut cx = PaintCx::new(&mut under[..], band, screen, band);
                if !app.paint_under(&mut cx, now_us) {
                    cx.fill_rect(band, BLACK);
                }
            }
            {
                let mut cx = PaintCx::new(&mut out[..], band, screen, band);
                app.paint(&mut cx, now_us);
            }
            blend_band(&mut out[..len], &under[..len], alpha);
        }
        _ => {
            let mut cx = PaintCx::new(&mut out[..], band, screen, band);
            app.paint(&mut cx, now_us);
        }
    }
    len
}

/// Number of dirty rectangles in this flush (1 for a full redraw).
fn dirty_rects(dirty: &DirtyList) -> u16 {
    if dirty.is_full_redraw() {
        1
    } else {
        dirty.as_slice().len() as u16
    }
}

/// Every band to paint, in screen order.
fn bands<'a>(dirty: &'a DirtyList, cfg: &'a RenderCfg) -> impl Iterator<Item = Rect> + 'a {
    let screen = cfg.screen;
    let band_h = cfg.band_h;
    let full = dirty.is_full_redraw();
    dirty
        .as_slice()
        .iter()
        .copied()
        // A full redraw stores no rectangles; substitute the screen.
        .chain(core::iter::once(screen).filter(move |_| full))
        .filter_map(move |region| region.intersect(screen))
        .flat_map(move |region| region.rows(band_h))
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
