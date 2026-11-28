//! GUI showcase: a card list, a controls screen, a sliding transition and a
//! toast, all driven by dirty rectangles.
//!
//! Panel: SCK GP14, MOSI GP11, CS GP13, DC GP22, RST GP27, BL GP28.
//! Keys: MENU GP8, PREV GP9, INPUT 1 GP1, BACK GP26 (active low).
//!
//! Build:
//!
//! ```sh
//! cargo build -p gui-showcase --release --target thumbv6m-none-eabi
//! ```
#![no_std]
#![no_main]
#![allow(unsafe_code)]

use defmt::info;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Input, Level, Output, Pull};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::watch::Watch;
use panic_probe as _;
use static_cell::StaticCell;

use utsuroui::band::BandSink;
use utsuroui::embassy_task::{EVENT_QUEUE, LATENCY_SLOTS, UiConfig};
use utsuroui::event::Event;
use utsuroui::flush::RenderCfg;

mod app;
mod input;
mod panel;
mod style;

bind_interrupts!(struct Irqs {
    DMA_IRQ_0 => embassy_rp::dma::InterruptHandler<embassy_rp::peripherals::DMA_CH2>;
});

/// Rows per band.
const BAND_H: u16 = 16;
/// Bytes of one band buffer.
const STRIP_BYTES: usize = style::W as usize * BAND_H as usize * 2;
/// Frame budget while animating (60 fps).
const FRAME_BUDGET_US: u32 = 16_667;

static EVENTS: Channel<CriticalSectionRawMutex, Event, EVENT_QUEUE> = Channel::new();
static LATENCY: Watch<CriticalSectionRawMutex, u16, LATENCY_SLOTS> = Watch::new();
static BUFFERS: StaticCell<utsuroui::BandBuffers<STRIP_BYTES>> = StaticCell::new();

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    info!("gui-showcase starting");

    let menu = Input::new(p.PIN_8, Pull::Up);
    let prev = Input::new(p.PIN_9, Pull::Up);
    let input1 = Input::new(p.PIN_1, Pull::Up);
    let back = Input::new(p.PIN_26, Pull::Up);
    spawner
        .spawn(input::task(menu, prev, input1, back, EVENTS.sender()).expect("spawn input task"));

    let display = panel::init(
        p.SPI1, p.PIN_14, p.PIN_11, p.DMA_CH2, p.PIN_13, p.PIN_22, p.PIN_27, Irqs,
    )
    .await;
    let _backlight = panel::backlight(p.PIN_28);

    let mut led = Output::new(p.PIN_25, Level::Low);
    led.set_high();

    let mut app = app::GuiApp::new();
    let mut display = display;
    run_ui(&mut app, &mut display).await;
}

async fn run_ui<S: BandSink>(app: &mut app::GuiApp, sink: &mut S) {
    let mut events = EVENTS.receiver();
    let mut latency = LATENCY.receiver().expect("latency watch has a receiver");
    let buffers = BUFFERS.init(utsuroui::BandBuffers::new());

    let cfg = UiConfig::new(RenderCfg::new(utsuroui::SCREEN, BAND_H), FRAME_BUDGET_US);

    if let Err(error) =
        utsuroui::embassy_task::ui_task(app, sink, buffers, &mut events, &mut latency, cfg).await
    {
        info!("display write failed: {:?}", defmt::Debug2Format(&error));
    }
    loop {
        cortex_m::asm::wfi();
    }
}
