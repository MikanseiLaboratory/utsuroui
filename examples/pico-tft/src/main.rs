//! RP2040 / RP2350 firmware example: `utsuroui` on a 240x320 ST7789 panel.
//!
//! Wiring and clock notes live in `docs/HARDWARE.md`; the layout is the same on
//! both chips (GP18/GP19 SPI, GP16 CS, GP17 DC, GP20 RST, GP21 BL, GP14/GP15
//! buttons).
//!
//! Tasks:
//!
//! * `input::task` on core 0 — polls the two buttons and sends `Event`s.
//! * `measure::run` on core 1 — a bare loop that writes synthetic latency
//!   samples into a `Watch` slot (the real product timestamps GPIO edges here).
//! * the UI loop in `main` — `utsuroui::embassy_task::ui_task` with `DemoApp`.
//!
//! Build:
//!
//! ```sh
//! # RP2040 (default)
//! cargo build -p pico-tft --release --target thumbv6m-none-eabi
//! # RP2350 (Pico 2)
//! cargo build -p pico-tft --release --no-default-features --features rp235xa \
//!     --target thumbv8m.main-none-eabihf
//! ```
#![no_std]
#![no_main]
#![allow(unsafe_code)]

use defmt::info;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Input, Pull};
use embassy_rp::multicore::{Stack, spawn_core1};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::watch::Watch;
use panic_probe as _;
use static_cell::StaticCell;

use utsuroui::band::BandSink;
use utsuroui::embassy_task::{EVENT_QUEUE, LATENCY_SLOTS, UiConfig};
use utsuroui::event::Event;
use utsuroui::flush::RenderCfg;
use utsuroui_demo::app::DemoApp;
use utsuroui_demo::style::W;

mod input;
mod measure;
mod panel;

bind_interrupts!(struct Irqs {
    DMA_IRQ_0 => embassy_rp::dma::InterruptHandler<embassy_rp::peripherals::DMA_CH0>;
});

/// Rows per band.
const BAND_H: u16 = 16;
/// Bytes of one band buffer.
const STRIP_BYTES: usize = W as usize * BAND_H as usize * 2;

static EVENTS: Channel<CriticalSectionRawMutex, Event, EVENT_QUEUE> = Channel::new();
static LATENCY: Watch<CriticalSectionRawMutex, u16, LATENCY_SLOTS> = Watch::new();
static BUFFERS: StaticCell<utsuroui::BandBuffers<STRIP_BYTES>> = StaticCell::new();
static CORE1_STACK: StaticCell<Stack<4096>> = StaticCell::new();

/// Optional tear-effect gate: waits for the panel's TE rising edge before each
/// band transfer. Conservative (once per band instead of once per frame).
#[cfg(feature = "te")]
struct TeSink<'a> {
    inner: &'a mut panel::Panel,
    te: Input<'a>,
}

#[cfg(feature = "te")]
impl BandSink for TeSink<'_> {
    type Error = <panel::Panel as BandSink>::Error;

    async fn write_band(
        &mut self,
        band: utsuroui::geom::Rect,
        data: &[u8],
    ) -> Result<(), Self::Error> {
        self.te.wait_for_rising_edge().await;
        self.inner.write_band(band, data).await
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    info!("utsuroui pico-tft example starting");

    // Core 0: debounced buttons -> Event channel.
    let button_a = Input::new(p.PIN_14, Pull::Up);
    let button_b = Input::new(p.PIN_15, Pull::Up);
    spawner.spawn(input::task(button_a, button_b, EVENTS.sender()).expect("spawn input task"));

    // Core 1: measurement only. It writes the latest value and never waits for
    // the UI (DESIGN.md section 12.1).
    let latency_sender = LATENCY.sender();
    spawn_core1(p.CORE1, CORE1_STACK.init(Stack::new()), move || {
        measure::run(latency_sender)
    });

    let display = panel::init(
        p.SPI0, p.PIN_18, p.PIN_19, p.DMA_CH0, p.PIN_16, p.PIN_17, p.PIN_20, Irqs,
    )
    .await;
    let _backlight = panel::backlight(p.PIN_21);

    #[cfg(feature = "te")]
    {
        use lcd_async::options::TearingEffect;
        let mut display = display;
        display
            .set_tearing_effect(TearingEffect::Vertical)
            .await
            .expect("TE output enable");
        let te = Input::new(p.PIN_22, Pull::Down);
        run_ui(TeSink {
            inner: &mut display,
            te,
        })
        .await;
    }

    #[cfg(not(feature = "te"))]
    {
        let mut display = display;
        run_ui(&mut display).await;
    }
}

async fn run_ui<S: BandSink>(mut sink: S) {
    let mut app = DemoApp::new();
    let mut events = EVENTS.receiver();
    let mut latency = LATENCY.receiver().expect("latency watch has a receiver");
    let buffers = BUFFERS.init(utsuroui::BandBuffers::new());

    let cfg = UiConfig::new(RenderCfg::new(utsuroui::SCREEN, BAND_H), 22_000);

    if let Err(error) = utsuroui::embassy_task::ui_task(
        &mut app,
        &mut sink,
        buffers,
        &mut events,
        &mut latency,
        cfg,
    )
    .await
    {
        info!("display write failed: {:?}", defmt::Debug2Format(&error));
    }
    loop {
        cortex_m::asm::wfi();
    }
}
