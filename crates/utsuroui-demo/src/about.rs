//! Static about screen.

use utsuroui::geom::Point;
use utsuroui::paint::PaintCx;
use utsuroui::text::TextStyle;

use crate::style::{ACCENT, DIM, FG, VERSION};

/// Lines shown under the title.
pub const LINES: &[&str] = &[
    "dirty-rect embedded GUI",
    "240x320 RGB565 / ST7789",
    "RP2040 + RP2350 / Embassy",
    "MIT OR Apache-2.0",
    "design: DESIGN.md",
];

/// Paints the about screen body.
pub fn paint(cx: &mut PaintCx<'_>, origin: Point) {
    let mut y = origin.y + 64;
    cx.text(
        "utsuroui",
        Point::new(origin.x + 16, y),
        TextStyle::Large,
        ACCENT,
    );
    y += 26;
    cx.text(VERSION, Point::new(origin.x + 16, y), TextStyle::Small, DIM);
    y += 26;
    for line in LINES {
        cx.text(line, Point::new(origin.x + 16, y), TextStyle::Small, FG);
        y += 16;
    }
    cx.text(
        "MikanseiLaboratory",
        Point::new(origin.x + 16, origin.y + 226),
        TextStyle::Small,
        DIM,
    );
}
