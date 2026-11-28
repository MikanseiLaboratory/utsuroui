//! `utsuroui` on a 240x320 ST7789 panel wired to **SPI1**, with an 8-key
//! front panel.
//!
//! Panel: SCK GP14, MOSI GP11, CS GP13, DC GP22, RST GP27, BL GP28.
//! Keys: MENU GP8, PREV GP9, INPUT 1 GP1, BACK GP26 (all active low).
//! Only these GPIOs matter; nothing here is specific to a particular board.
//!
//! Tasks:
//!
//! * `input::task` on core 0 — debounced keys -> `Event` channel.
//! * `measure::run` on core 1 — synthetic latency samples in a `Watch` slot.
//! * the UI loop in `main` — `utsuroui::embassy_task::ui_task` with the demo app.
//!
//! Build:
//!
//! ```sh
//! cargo build -p spi1-panel --release --target thumbv6m-none-eabi
//! ```
#![no_std]
#![no_main]
#![allow(unsafe_code)]

use defmt::info;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Input, Level, Output, Pull};
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
    DMA_IRQ_0 => embassy_rp::dma::InterruptHandler<embassy_rp::peripherals::DMA_CH2>;
});

/// Rows per band.
const BAND_H: u16 = 16;
/// Bytes of one band buffer.
const STRIP_BYTES: usize = W as usize * BAND_H as usize * 2;

static EVENTS: Channel<CriticalSectionRawMutex, Event, EVENT_QUEUE> = Channel::new();
static LATENCY: Watch<CriticalSectionRawMutex, u16, LATENCY_SLOTS> = Watch::new();
static BUFFERS: StaticCell<utsuroui::BandBuffers<STRIP_BYTES>> = StaticCell::new();
static CORE1_STACK: StaticCell<Stack<4096>> = StaticCell::new();

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    info!("utsuroui spi1-panel example starting");

    // Front panel keys (all active-low with pull-ups).
    let menu = Input::new(p.PIN_8, Pull::Up);
    let prev = Input::new(p.PIN_9, Pull::Up);
    let input1 = Input::new(p.PIN_1, Pull::Up);
    let back = Input::new(p.PIN_26, Pull::Up);
    spawner
        .spawn(input::task(menu, prev, input1, back, EVENTS.sender()).expect("spawn input task"));

    // Core 1: measurement only.
    let latency_sender = LATENCY.sender();
    spawn_core1(p.CORE1, CORE1_STACK.init(Stack::new()), move || {
        measure::run(latency_sender)
    });

    let display = panel::init(
        p.SPI1, p.PIN_14, p.PIN_11, p.DMA_CH2, p.PIN_13, p.PIN_22, p.PIN_27, Irqs,
    )
    .await;
    let _backlight = panel::backlight(p.PIN_28);

    // On-board LED on GP25: high once the UI is running.
    let mut led = Output::new(p.PIN_25, Level::Low);
    led.set_high();

    let mut display = display;
    run_ui(&mut display).await;
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
