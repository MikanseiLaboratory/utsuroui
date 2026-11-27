//! Iryx SW-00 firmware example: `utsuroui` on the SW-00 240x320 ST7789 panel.
//!
//! The pin map follows the original SW-00 firmware (`iryx` commit `fd992f7^`):
//! the panel is on **SPI1** (GP14 SCK / GP11 MOSI / GP13 CS / GP22 DC / GP27 RST
//! / GP28 BL) and the front panel keys are MENU GP8, MARK IN GP9, INPUT 1 GP1
//! and BACK GP26. The W5500 on SPI0 is not used by this example.
//!
//! Tasks:
//!
//! * `input::task` on core 0 — debounced keys -> `Event` channel.
//! * `measure::run` on core 1 — synthetic latency samples in a `Watch` slot.
//! * the UI loop in `main` — `utsuroui::embassy_task::ui_task` with the demo app.
//!
//! Build (RP2040 only):
//!
//! ```sh
//! cargo build -p iryx-sw00 --release --target thumbv6m-none-eabi
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
static STRIPS: StaticCell<[u8; 2 * STRIP_BYTES]> = StaticCell::new();
static CORE1_STACK: StaticCell<Stack<4096>> = StaticCell::new();

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    info!("utsuroui iryx-sw00 example starting");

    // SW-00 front panel keys (all active-low with pull-ups).
    let menu = Input::new(p.PIN_8, Pull::Up);
    let mark_in = Input::new(p.PIN_9, Pull::Up);
    let input1 = Input::new(p.PIN_1, Pull::Up);
    let back = Input::new(p.PIN_26, Pull::Up);
    spawner.spawn(
        input::task(menu, mark_in, input1, back, EVENTS.sender()).expect("spawn input task"),
    );

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

    // On-board LED (GP25 on the W5500-EVB-Pico): high once the UI is running.
    let mut led = Output::new(p.PIN_25, Level::Low);
    led.set_high();

    let mut display = display;
    run_ui(&mut display).await;
}

async fn run_ui<S: BandSink>(mut sink: S) {
    let mut app = DemoApp::new();
    let mut events = EVENTS.receiver();
    let mut latency = LATENCY.receiver().expect("latency watch has a receiver");
    let strips = STRIPS.init([0u8; 2 * STRIP_BYTES]);
    let (band_a, band_b) = strips.split_at_mut(STRIP_BYTES);

    let cfg = UiConfig::new(RenderCfg::new(utsuroui::SCREEN, BAND_H), 22_000);

    if let Err(error) = utsuroui::embassy_task::ui_task(
        &mut app,
        &mut sink,
        band_a,
        band_b,
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
