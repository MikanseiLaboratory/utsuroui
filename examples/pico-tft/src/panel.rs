//! Panel wiring: SPI, DC/CS/RST pins and the `lcd-async` display type.
//!
//! Default wiring (Raspberry Pi Pico / Pico 2, same pin numbers):
//!
//! | Signal | GPIO |
//! |---|---|
//! | SCK | GP18 |
//! | MOSI | GP19 |
//! | CS | GP16 |
//! | DC | GP17 |
//! | RST | GP20 |
//! | BL | GP21 (driven high) |
//! | TE | GP22 (feature `te`) |
//! | Button A | GP14 (to GND) |
//! | Button B | GP15 (to GND) |

use embassy_rp::Peri;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::interrupt::typelevel::Binding;
use embassy_rp::peripherals::{DMA_CH0, PIN_16, PIN_17, PIN_18, PIN_19, PIN_20, SPI0};
use embassy_rp::spi::{Async, Config, Phase, Polarity, Spi};
use embedded_hal_bus::spi::{ExclusiveDevice, NoDelay};
use lcd_async::Display;
use lcd_async::interface::SpiInterface;
use lcd_async::models::ST7789;
use lcd_async::options::{ColorOrder, Orientation};

/// Panel width in pixels.
pub const WIDTH: u16 = 240;
/// Panel height in pixels.
pub const HEIGHT: u16 = 240;

/// SPI clock. 31.25 MHz is `clk_peri / 4` on the default 125 MHz peripheral
/// clock. Raise it only after checking the module on real hardware
/// (DESIGN.md section 4.7).
pub const SPI_HZ: u32 = 31_250_000;

/// Sub-pixel order. Flip to `ColorOrder::Bgr` if red and blue are swapped.
pub const COLOR_ORDER: ColorOrder = ColorOrder::Rgb;

/// Panel orientation.
pub const ORIENTATION: Orientation = Orientation::new();

/// The fully assembled display type used by the application.
pub type Panel = Display<
    SpiInterface<
        ExclusiveDevice<Spi<'static, SPI0, Async>, Output<'static>, NoDelay>,
        Output<'static>,
    >,
    ST7789,
    Output<'static>,
>;

/// Resets and initialises the panel, returning the ready display.
#[allow(clippy::too_many_arguments)]
pub async fn init<IRQ>(
    spi: Peri<'static, SPI0>,
    sck: Peri<'static, PIN_18>,
    mosi: Peri<'static, PIN_19>,
    tx_dma: Peri<'static, DMA_CH0>,
    cs_pin: Peri<'static, PIN_16>,
    dc_pin: Peri<'static, PIN_17>,
    rst_pin: Peri<'static, PIN_20>,
    irq: IRQ,
) -> Panel
where
    IRQ: Binding<
            <DMA_CH0 as embassy_rp::dma::ChannelInstance>::Interrupt,
            embassy_rp::dma::InterruptHandler<DMA_CH0>,
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
        .orientation(ORIENTATION)
        .reset_pin(rst)
        .init(&mut AsyncDelay)
        .await
        .expect("panel init")
}

/// Backlight pin, driven high after init.
pub fn backlight(bl_pin: Peri<'static, embassy_rp::peripherals::PIN_21>) -> Output<'static> {
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
