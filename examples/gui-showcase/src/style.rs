//! Colours and metrics for the showcase.
//!
//! Everything is on an 8 px grid with one accent colour, so the screens read as
//! one design rather than a pile of demos.

use utsuroui::color::{BLACK, GRAY, Rgb565, WHITE, YELLOW};
use utsuroui::geom::Rect;

/// Panel width in pixels.
pub const W: u16 = 240;
/// Panel height in pixels.
pub const H: u16 = 320;
/// The full screen.
pub const SCREEN: Rect = Rect::new(0, 0, W, H);

/// Background.
pub const BG: Rgb565 = BLACK;
/// Card fill.
pub const CARD: Rgb565 = Rgb565::new(1, 5, 2);
/// Selected card fill.
pub const CARD_SELECTED: Rgb565 = Rgb565::new(3, 13, 4);
/// Hairline rules and secondary text.
pub const MUTED: Rgb565 = GRAY;
/// Body text.
pub const TEXT: Rgb565 = WHITE;
/// Accent (selection edge, slider fill).
pub const ACCENT: Rgb565 = YELLOW;
/// Switch track when off.
pub const TRACK_OFF: Rgb565 = Rgb565::new(6, 12, 6);

/// Header height.
pub const HEADER_H: u16 = 36;
/// Left and right margin.
pub const MARGIN: i16 = 8;
/// Content width.
pub const CONTENT_W: u16 = 224;
/// Y of the footer hint.
pub const FOOTER_Y: i16 = 292;

/// First card row.
pub const LIST_TOP: i16 = 56;
/// One card row.
pub const ROW_H: u16 = 56;
