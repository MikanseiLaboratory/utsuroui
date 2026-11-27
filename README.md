# utsuroui (移ろい)

[![CI](https://github.com/MikanseiLaboratory/utsuroui/actions/workflows/ci.yml/badge.svg)](https://github.com/MikanseiLaboratory/utsuroui/actions/workflows/ci.yml)

A tiny, allocation-free GUI for 240x320 RGB565 SPI TFTs (ST7789 / ILI9341) on
RP2040 / RP2350 with Embassy.

Only dirty rectangles are painted and sent, as 16-row bands. Animation is
fixed-point tweening with continuous retargeting.

| Menu | Measure |
|---|---|
| ![menu](docs/images/menu_initial.png) | ![measure](docs/images/measure.png) |

## Features

- **Dirty-rect bands** — a 16 px highlight move sends ~25 KB, not the 153,600 B
  full frame.
- **Fixed-point tweens** — `Tween::retarget` keeps motion continuous when a
  button is pressed mid-animation; easing is a 257-entry LUT, no soft-float.
- **Retained static UI** — widgets are descriptors, state lives as long as the
  screen. No heap, no `dyn Widget`, no double render tree; the core is
  `#![forbid(unsafe_code)]`.
- **Two-button focus** — starts at index 0; `Next` / `Select` / long-press
  `Back`. Encoder and touch decoding included.
- **Router + transitions** — cut, fade through black, horizontal slide and
  crossfade (crossfade repaints the outgoing screen, so no frame snapshot).
- **Embassy integration** — `ui_task` turns input, `Watch` samples and dirty
  rectangles into bands, then paces frames. Idle means no SPI traffic.
- **Measurement isolation** — core 1 only writes the latest sample; the UI
  throttles it to 30 Hz.
- **Host-tested** — 8 PNG goldens, byte-order and band-splitting checks,
  frame-budget tests, plus `thumbv6m` / `thumbv8m.main` cross-checks.
- **Size** — ~50 KiB flash / ~21 KiB RAM for the demo firmware (release, defmt).

## Workspace

```text
crates/utsuroui/      core: geom/color/dirty/anim/text, paint/band/flush,
                      widgets, router, transitions, Embassy task, lcd-async adapter
crates/utsuroui-demo/ 240x320 demo app, golden tests, PNG screenshot tool
examples/pico-tft/    RP2040 (SPI0) and RP2350 (feature rp235xa)
examples/iryx-sw00/   Iryx SW-00: SPI1 panel + SW-00 front-panel keys
```

## Quick start

```sh
# Host tests and screenshots
cargo test -p utsuroui -p utsuroui-demo
cargo run -p utsuroui-demo --example render_png     # target/golden/*.png

# Generic Pico (RP2040)
rustup target add thumbv6m-none-eabi
cargo build -p pico-tft --release --target thumbv6m-none-eabi

# Iryx SW-00
cargo build -p iryx-sw00 --release --target thumbv6m-none-eabi

# Flash (SWD)
cd examples/pico-tft && probe-rs run --chip rp2040
```

RP2350 / Pico 2: `cargo build -p pico-tft --release --no-default-features
--features rp235xa --target thumbv8m.main-none-eabihf`.

Pin maps and clock notes: [docs/HARDWARE.md](docs/HARDWARE.md).

## Writing an app

```rust
use utsuroui::{App, Dirty, DirtyList, Event, PaintCx};

struct MyApp;

impl App for MyApp {
    fn event(&mut self, _: &Event, _: u64, _: &mut DirtyList) {}
    fn tick(&mut self, _: u64, _: &mut DirtyList) -> bool { false }
    fn paint(&self, cx: &mut PaintCx<'_>, _: u64) {
        cx.fill_rect(utsuroui::SCREEN, utsuroui::BLACK);
        cx.text("hello", utsuroui::Point::new(8, 8),
                utsuroui::TextStyle::Large, utsuroui::YELLOW);
    }
}

let mut app = MyApp;
let mut dirty = Dirty::new(utsuroui::SCREEN);
dirty.mark_full();
// Firmware: utsuroui::embassy_task::ui_task(&mut app, &mut display, ...)
```

`paint` draws one band of the screen. Everything marked dirty must be fully
covered — the band buffer is not cleared. Coordinates are always screen
coordinates; clipping and band translation are handled by `PaintCx`.

## Status

Host-tested and cross-compiled for both MCUs; the demo firmware builds and links
for RP2040 and RP2350. Real-hardware behaviour (SPI clock limits, stack
watermark, frame times) has **not** been measured yet — the procedure is in
[docs/HARDWARE.md](docs/HARDWARE.md).

## Docs

[docs/DESIGN.md](docs/DESIGN.md) (starting design) ·
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) ·
[docs/IMPLEMENTATION_PLAN.md](docs/IMPLEMENTATION_PLAN.md) ·
[docs/HARDWARE.md](docs/HARDWARE.md) ·
[docs/FONT-LICENSES.md](docs/FONT-LICENSES.md)

## License

MIT OR Apache-2.0. Third-party notices: [NOTICE](NOTICE).
