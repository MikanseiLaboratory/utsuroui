//! Host-side rendering helpers (used by tests, examples and screenshots).
//!
//! Rendering goes through the same `flush` loop as the firmware, with a
//! `MemorySink` instead of an SPI panel. That keeps golden images honest: they
//! show exactly what the device would send.

use utsuroui::band::MemorySink;
use utsuroui::dirty::Dirty;
use utsuroui::flush::{FlushStats, RenderCfg, flush};

use crate::app::DemoApp;
use crate::style::{H, SCREEN, W};

/// Rows per band.
pub const BAND_H: u16 = 16;
/// Bytes needed for one band buffer.
pub const BAND_LEN: usize = W as usize * BAND_H as usize * 2;
/// Bytes needed for one full frame (240x320 RGB565).
pub const FRAME_LEN: usize = W as usize * H as usize * 2;

/// The band buffers used by the host renderer.
pub type BandBuf = utsuroui::BandBuffers<BAND_LEN>;

/// Renders one full frame into `frame`.
pub async fn render_frame(
    app: &DemoApp,
    now_us: u64,
    frame: &mut [u8],
    buffers: &mut BandBuf,
) -> FlushStats {
    assert!(
        frame.len() >= FRAME_LEN,
        "frame buffer must hold 240x320 RGB565 pixels"
    );
    let mut dirty = Dirty::new(SCREEN);
    dirty.mark_full();
    let cfg = RenderCfg::new(SCREEN, BAND_H);
    let mut sink = MemorySink::new(frame, W);
    match flush(app, &mut sink, &dirty, now_us, buffers, &cfg).await {
        Ok(stats) => stats,
        Err(never) => match never {},
    }
}
