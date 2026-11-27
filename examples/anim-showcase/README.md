# anim-showcase — animation demo firmware

Four animation scenes on the same 240x320 ST7789 panel, in one firmware. Every
scene animates through dirty rectangles, so the SPI traffic matches what
actually moved.

| Scene | What moves | Dirty region per frame |
|---|---|---|
| **Waveform** | a synthetic audio trace scrolling left, with a mirrored ghost and a sweep marker | one 240x116 band |
| **Ticker** | a horizontal marquee, a second slower marquee, a vertically scrolling log | two 240x18 bands and one 240x88 band |
| **Float** | five blobs drifting on sine paths at different periods | the union of each blob's old and new box |
| **Spectrum** | an equaliser whose bars ease towards random targets | only the bars whose 2-px height step changed |

## Keys

| Key | GPIO | Action |
|---|---|---|
| MENU | GP8 | next scene |
| PREV | GP9 | previous scene |
| INPUT 1 | GP1 | pause / resume |
| BACK | GP26 | restart the current scene |

## Build and flash

```sh
cargo build -p anim-showcase --release --target thumbv6m-none-eabi
cd examples/anim-showcase
probe-rs run --chip rp2040 --protocol swd --speed 100
```

The panel pin map is the same as `examples/spi1-panel`: SPI1 with MOSI GP11,
SCK GP14, CS GP13, DC GP22, RST GP27, BL GP28. `defmt` logs are available over
RTT; the on-board LED (GP25) goes high once the UI loop starts.

## How the animation works

Nothing keeps a framebuffer. Each frame the app computes the new state, marks
the rectangles it invalidated, and the renderer paints those rectangles into
16-row bands and sends only those. A blob that moves four pixels marks its old
and new box, not the screen; a bar that grows four pixels marks one column.

Sine motion uses a parabolic integer approximation (`wobble`) instead of a
table, so there is no floating point anywhere.
