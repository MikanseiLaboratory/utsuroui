//! `lcd-async` adapter (`lcd` feature).
//!
//! The framework only needs one operation from a panel: "send this rectangle of
//! big-endian RGB565 bytes". `lcd-async` 0.1.3 exposes exactly that as
//! `Display::show_raw_data`, so the adapter is deliberately tiny. Everything
//! else (model init, MADCTL, colour order, offsets) stays in the application.
//!
//! The future returned by `show_raw_data` borrows the display mutably. It is
//! deliberately not required to be `Send`: Embassy's thread-mode spawner accepts
//! non-`Send` tasks and `lcd-async` does not declare its interface futures as
//! `Send`.

use crate::band::BandSink;
use crate::geom::Rect;

pub use lcd_async;

use embedded_hal::digital::OutputPin;
use lcd_async::Display;
use lcd_async::interface::Interface;
use lcd_async::models::Model;

impl<DI, M, RST> BandSink for Display<DI, M, RST>
where
    DI: Interface<Word = u8>,
    M: Model,
    RST: OutputPin,
{
    type Error = DI::Error;

    async fn write_band(&mut self, band: Rect, data: &[u8]) -> Result<(), Self::Error> {
        debug_assert!(
            data.len() >= band.area() as usize * 2,
            "band data is shorter than the rectangle it describes"
        );
        self.show_raw_data(
            band.x.max(0) as u16,
            band.y.max(0) as u16,
            band.w,
            band.h,
            data,
        )
        .await
    }
}

/// Sends a full-screen RGB565 buffer through `show_raw_data` in one call.
///
/// Only useful for the `full-fb` path; the band path is the default.
pub async fn show_full_frame<DI, M, RST>(
    display: &mut Display<DI, M, RST>,
    data: &[u8],
    width: u16,
    height: u16,
) -> Result<(), DI::Error>
where
    DI: Interface<Word = u8>,
    M: Model,
    RST: OutputPin,
{
    display.show_raw_data(0, 0, width, height, data).await
}
