# spi1-panel — 240x240 ST7789 on SPI1, with an 8-key front panel

Runs the `utsuroui` demo on a 240x240 ST7789 panel wired to **SPI1**, with four
front-panel keys. Nothing here is specific to a board: only the GPIO numbers
below matter.

## Wiring

### Display (ST7789, 240x240, inverted module, SPI1, mode 0)

| Signal | GPIO |
|---|---|
| MOSI | GP11 |
| MISO | GP12 (not used) |
| SCK | GP14 |
| CS | GP13 |
| DC | GP22 |
| RST | GP27 |
| BL | GP28 |

`SPI_HZ` requests 62.5 MHz (`clk_peri` / 2). Drop it to 50 MHz if the panel
shows noise, shifted pixels or colour errors, and lower still for long leads.

### Keys (active low, internal pull-ups)

| Key | GPIO | Demo action |
|---|---|---|
| MENU | GP8 | next item (auto-repeat) |
| PREV | GP9 | previous item (auto-repeat) |
| INPUT 1 | GP1 | select |
| BACK | GP26 | back |

The panel has eight keys in total; INPUT 2-4 (GP2-GP4), NEXT (GP10) and
SHIFT (GP15) are unused here. Add them to `src/input.rs` if the application
needs them. The on-board LED on GP25 goes high once the UI loop starts.

## Build and flash

```sh
cargo build -p spi1-panel --release --target thumbv6m-none-eabi
cd examples/spi1-panel
probe-rs run --chip rp2040 --protocol swd --speed 100
```

`defmt` logs are available over RTT.

## Layout

The panel is 240x240, so the shared demo (`utsuroui-demo`) fits without
changes: menu, measurement, settings and about screens plus fade, slide and
crossfade transitions.
