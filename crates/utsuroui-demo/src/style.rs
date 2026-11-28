//! Visual style constants for the demo application.
//!
//! The layout is a flat UI: a header with a one-pixel rule, 28 px menu rows, a
//! yellow highlight bar, and a footer with a live latency readout and bar.

use utsuroui::color::{GRAY, Rgb565, YELLOW};
use utsuroui::geom::Rect;

use utsuroui::color::{BLACK, WHITE};

/// Panel width in pixels.
pub const W: u16 = 240;
/// Panel height in pixels.
pub const H: u16 = 240;
/// The full screen.
pub const SCREEN: Rect = Rect::new(0, 0, W, H);

/// Header height.
pub const HEADER_H: u16 = 34;
/// Height of one menu row.
pub const ROW_H: u16 = 28;
/// Left margin of content.
pub const MARGIN: i16 = 8;
/// Content width (screen minus both margins).
pub const CONTENT_W: u16 = 224;
/// Y of the first menu row.
pub const MENU_TOP: i16 = 44;
/// Y of the footer separator.
pub const FOOTER_Y: i16 = 186;
/// Y of the big latency readout.
pub const READOUT_Y: i16 = 190;
/// Y of the latency bar frame.
pub const BAR_Y: i16 = 222;
/// Height of the latency bar.
pub const BAR_H: u16 = 12;

/// Title shown on the menu screen.
pub const TITLE: &str = "UTSUROUI";
/// Version shown on the about screen.
pub const VERSION: &str = "0.1.0";

/// Highlight slide duration in microseconds.
pub const ROW_HIGHLIGHT_US: u32 = 120_000;
/// Numeric readout tween duration in microseconds.
pub const READOUT_TWEEN_US: u32 = 120_000;
/// Fade duration (out and in) in microseconds.
pub const FADE_US: u32 = 90_000;
/// Slide duration in microseconds.
pub const SLIDE_US: u32 = 180_000;
/// Crossfade duration in microseconds.
pub const CROSSFADE_US: u32 = 180_000;
/// Minimum interval between displayed latency updates.
pub const LATENCY_INTERVAL_US: u64 = 33_000;

/// Default full-scale latency for the bar, in tenths of a millisecond.
pub const DEFAULT_SCALE_X10: u16 = 600;

/// Background colour.
pub const BG: Rgb565 = BLACK;
/// Primary text colour.
pub const FG: Rgb565 = WHITE;
/// Secondary text colour.
pub const DIM: Rgb565 = GRAY;
/// Accent colour (highlight bar, live values).
pub const ACCENT: Rgb565 = YELLOW;

/// Height of the settings rows.
pub const SETTING_ROW_H: u16 = 28;
/// Y of the first settings row.
pub const SETTINGS_TOP: i16 = 52;

/// The rect of one menu row.
pub const fn row_rect(index: u8) -> Rect {
    Rect::new(
        MARGIN,
        MENU_TOP + index as i16 * ROW_H as i16,
        CONTENT_W,
        ROW_H,
    )
}

/// The rect of one settings row.
pub const fn setting_rect(index: u8) -> Rect {
    Rect::new(
        MARGIN,
        SETTINGS_TOP + index as i16 * SETTING_ROW_H as i16,
        CONTENT_W,
        SETTING_ROW_H,
    )
}

/// The rect of the latency readout in the footer.
pub const READOUT_RECT: Rect = Rect::new(12, READOUT_Y, 216, 20);

/// The rect of the footer bar frame.
pub const BAR_RECT: Rect = Rect::new(12, BAR_Y, 216, BAR_H);

/// The rect of the graph frame on the measure screen.
pub const GRAPH_RECT: Rect = Rect::new(16, 124, 208, 56);

/// The rect holding the min/avg/max rows on the measure screen.
pub const STATS_RECT: Rect = Rect::new(16, 72, 208, 48);

/// Clamps a tenths value to the bar scale and returns the bar fill width.
pub fn bar_width(value_x10: u16, scale_x10: u16) -> u16 {
    if scale_x10 == 0 {
        return 0;
    }
    let inner = BAR_RECT.w.saturating_sub(2) as u32;
    let scaled = value_x10 as u32 * inner / scale_x10 as u32;
    scaled.min(inner) as u16
}
