//! RGB565 colours, blending and tweening.
//!
//! The framework uses [`embedded_graphics`]'s [`Rgb565`] as its colour type so
//! that drawing primitives and images interoperate without conversions. The
//! helpers here are integer-only and safe on a Cortex-M0+.

use embedded_graphics::pixelcolor::RgbColor;
use embedded_graphics::pixelcolor::raw::{RawData, RawU16};

pub use embedded_graphics::pixelcolor::Rgb565;

/// Pure black.
pub const BLACK: Rgb565 = Rgb565::new(0, 0, 0);
/// Pure white.
pub const WHITE: Rgb565 = Rgb565::new(31, 63, 31);
/// lagbuster-style highlight yellow.
pub const YELLOW: Rgb565 = Rgb565::new(31, 63, 0);
/// Background grey used for panels.
pub const DARK_GRAY: Rgb565 = Rgb565::new(4, 8, 4);
/// A dim grey for secondary labels.
pub const GRAY: Rgb565 = Rgb565::new(16, 32, 16);

/// Blends `src` over `dst` with `alpha` in `0..=255`.
///
/// Channels are mixed in their native 5/6/5 widths; no gamma correction.
pub fn blend(src: Rgb565, dst: Rgb565, alpha: u8) -> Rgb565 {
    match alpha {
        0 => dst,
        255 => src,
        _ => {
            let a = alpha as u32;
            let ia = 255 - a;
            let r = (src.r() as u32 * a + dst.r() as u32 * ia) / 255;
            let g = (src.g() as u32 * a + dst.g() as u32 * ia) / 255;
            let b = (src.b() as u32 * a + dst.b() as u32 * ia) / 255;
            Rgb565::new(r as u8, g as u8, b as u8)
        }
    }
}

/// Linear interpolation between two colours, `t` in `0..=65535`.
pub fn lerp(from: Rgb565, to: Rgb565, t: u16) -> Rgb565 {
    let alpha = ((t as u32 * 255) / 65535) as u8;
    blend(to, from, alpha)
}

/// Fades `color` towards black by `alpha` in `0..=255` (255 = unchanged).
pub fn darken(color: Rgb565, alpha: u8) -> Rgb565 {
    blend(color, BLACK, alpha)
}

/// The big-endian byte pair a panel expects for `color`.
///
/// `lcd-async` sends RGB565 pixels as big-endian byte pairs; keeping the
/// conversion in one place makes the panel byte order testable on the host.
pub fn be_bytes(color: Rgb565) -> [u8; 2] {
    raw_u16(color).to_be_bytes()
}

/// The raw 16-bit storage value of an RGB565 colour.
pub fn raw_u16(color: Rgb565) -> u16 {
    RawU16::from(color).into_inner()
}

/// Builds an RGB565 colour from its raw 16-bit value.
pub fn from_raw_u16(raw: u16) -> Rgb565 {
    Rgb565::new(
        ((raw >> 11) & 0x1F) as u8,
        ((raw >> 5) & 0x3F) as u8,
        (raw & 0x1F) as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alpha_extremes_are_identities() {
        assert_eq!(blend(WHITE, BLACK, 0), BLACK);
        assert_eq!(blend(WHITE, BLACK, 255), WHITE);
        assert_eq!(blend(YELLOW, DARK_GRAY, 255), YELLOW);
    }

    #[test]
    fn blend_midpoint_is_between_channels() {
        let c = blend(WHITE, BLACK, 128);
        assert!(c.r() > 14 && c.r() < 17);
        assert!(c.g() > 30 && c.g() < 33);
        assert_eq!(c, Rgb565::new(15, 31, 15));
    }

    #[test]
    fn lerp_endpoints() {
        assert_eq!(lerp(BLACK, YELLOW, 0), BLACK);
        assert_eq!(lerp(BLACK, YELLOW, 65535), YELLOW);
    }

    #[test]
    fn lerp_is_monotonic() {
        let mut prev = lerp(BLACK, WHITE, 0);
        for step in 1..=64u16 {
            let t = step * (65535 / 64);
            let c = lerp(BLACK, WHITE, t);
            assert!(c.r() >= prev.r(), "r must not go down");
            assert!(c.g() >= prev.g(), "g must not go down");
            prev = c;
        }
    }

    #[test]
    fn darken_zero_is_black() {
        assert_eq!(darken(WHITE, 0), BLACK);
        assert_eq!(darken(WHITE, 255), WHITE);
    }

    #[test]
    fn be_bytes_match_panel_order() {
        assert_eq!(be_bytes(BLACK), [0x00, 0x00]);
        assert_eq!(be_bytes(WHITE), [0xFF, 0xFF]);
        assert_eq!(be_bytes(YELLOW), [0xFF, 0xE0]);
        assert_eq!(be_bytes(Rgb565::new(0, 0, 31)), [0x00, 0x1F]);
    }

    #[test]
    fn named_colours_are_rgb565() {
        assert_eq!(YELLOW.r(), 31);
        assert_eq!(YELLOW.g(), 63);
        assert_eq!(YELLOW.b(), 0);
        assert_eq!(WHITE.r(), 31);
        assert_eq!(BLACK, Rgb565::BLACK);
    }
}
