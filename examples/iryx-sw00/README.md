# iryx-sw00 — Iryx SW-00 firmware example

Runs the `utsuroui` demo on the Iryx SW-00 board. The pin map is taken from the
original SW-00 firmware (`iryx` commit `fd992f7^`, `pico/src/bin/switcher.rs`),
so it is **not** the same as the generic Pico example.

## Pin map

### Display (ST7789, 240x320, SPI1, inverted module)

| Signal | GPIO |
|---|---|
| MOSI | GP11 |
| MISO | GP12 (not used) |
| SCK | GP14 |
| CS | GP13 |
| DC | GP22 |
| RST | GP27 |
| BL | GP28 |

SPI1 at **50 MHz** (the value the SW-00 firmware uses), mode 0. The W5500 on
SPI0 (GP16-GP21) is left untouched by this example.

### Front panel keys (active low, pull-up)

| Key | GPIO | Demo action |
|---|---|---|
| MENU | GP8 | next item (auto-repeat) |
| MARK IN | GP9 | previous item (auto-repeat) |
| INPUT 1 | GP1 | select |
| BACK | GP26 | back |

The other SW-00 keys (INPUT 2-4 on GP2-GP4, MARK OUT GP10, SHIFT GP15) are not
used by the demo; add them to `src/input.rs` if the application needs them.

## Build and flash

```sh
cargo build -p iryx-sw00 --release --target thumbv6m-none-eabi
cd examples/iryx-sw00
probe-rs run --chip rp2040
```

`defmt` logs are available over RTT. The on-board LED (GP25) goes high once the
UI loop starts.

## Layout

The SW-00 panel is 240x320, so the shared demo (`utsuroui-demo`) fits without
changes: menu, measurement, settings and about screens plus fade/slide/
crossfade transitions.
