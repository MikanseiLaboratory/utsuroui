//! ST7789 panel on SPI1 (240x240, inverted module).
//!
//! This is the same driver as the generic Pico example; only the bus and the
//! GPIO numbers differ. Rewire the pins below to match your own board.
//!
//! | Signal | GPIO |
//! |---|---|
//! | MOSI | GP11 |
//! | MISO | GP12 (not used) |
//! | SCK | GP14 |
//! | CS | GP13 |
//! | DC | GP22 |
//! | RST | GP27 |
//! | BL | GP28 |
//!

use embassy_rp::Peri;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::interrupt::typelevel::Binding;
use embassy_rp::peripherals::{DMA_CH2, PIN_11, PIN_13, PIN_14, PIN_22, PIN_27, PIN_28, SPI1};
use embassy_rp::spi::{Async, Config, Phase, Polarity, Spi};
use embedded_hal_bus::spi::{ExclusiveDevice, NoDelay};
use lcd_async::Display;
use lcd_async::interface::SpiInterface;
use lcd_async::models::ST7789;
use lcd_async::options::{ColorInversion, ColorOrder, Orientation};

/// Panel width in pixels.
pub const WIDTH: u16 = 240;
/// Panel height in pixels.
pub const HEIGHT: u16 = 240;

/// Requested SPI clock: `clk_peri` / 2 = 62.5 MHz on a 125 MHz RP2040.
///
/// Drop to 50 MHz (31.25 MHz actual) if the panel shows noise or shifted
/// pixels, and lower still for long leads.
pub const SPI_HZ: u32 = 62_500_000;

/// The module is inverted.
pub const INVERT: ColorInversion = ColorInversion::Inverted;

/// Sub-pixel order of the module.
pub const COLOR_ORDER: ColorOrder = ColorOrder::Rgb;

/// Panel orientation.
pub const ORIENTATION: Orientation = Orientation::new();

/// The assembled display type.
pub type Panel = Display<
    SpiInterface<
        ExclusiveDevice<Spi<'static, SPI1, Async>, Output<'static>, NoDelay>,
        Output<'static>,
    >,
    ST7789,
    Output<'static>,
>;

/// Resets and initialises the panel.
#[allow(clippy::too_many_arguments)]
pub async fn init<IRQ>(
    spi: Peri<'static, SPI1>,
    sck: Peri<'static, PIN_14>,
    mosi: Peri<'static, PIN_11>,
    tx_dma: Peri<'static, DMA_CH2>,
    cs_pin: Peri<'static, PIN_13>,
    dc_pin: Peri<'static, PIN_22>,
    rst_pin: Peri<'static, PIN_27>,
    irq: IRQ,
) -> Panel
where
    IRQ: Binding<
            <DMA_CH2 as embassy_rp::dma::ChannelInstance>::Interrupt,
            embassy_rp::dma::InterruptHandler<DMA_CH2>,
        > + 'static,
{
    let mut config = Config::default();
    config.frequency = SPI_HZ;
    config.phase = Phase::CaptureOnFirstTransition;
    config.polarity = Polarity::IdleLow;

    let spi = Spi::new_txonly(spi, sck, mosi, tx_dma, irq, config);
    let cs = Output::new(cs_pin, Level::High);
    let dc = Output::new(dc_pin, Level::Low);
    let rst = Output::new(rst_pin, Level::High);
    let device = ExclusiveDevice::new_no_delay(spi, cs).expect("chip-select is infallible");

    lcd_async::Builder::new(ST7789, SpiInterface::new(device, dc))
        .display_size(WIDTH, HEIGHT)
        .color_order(COLOR_ORDER)
        .invert_colors(INVERT)
        .orientation(ORIENTATION)
        .reset_pin(rst)
        .init(&mut AsyncDelay)
        .await
        .expect("panel init")
}

/// Backlight pin, driven high after init.
pub fn backlight(bl_pin: Peri<'static, PIN_28>) -> Output<'static> {
    Output::new(bl_pin, Level::High)
}

/// A small async delay implementation for the panel init sequence.
pub struct AsyncDelay;

impl embedded_hal_async::delay::DelayNs for AsyncDelay {
    async fn delay_ns(&mut self, ns: u32) {
        let us = (ns / 1_000).max(1) as u64;
        embassy_time::Timer::after(embassy_time::Duration::from_micros(us)).await;
    }
}
