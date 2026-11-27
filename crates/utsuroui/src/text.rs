//! Text measurement and heap-free number formatting.
//!
//! The default backend is the monospaced bitmap fonts shipped with
//! `embedded-graphics` (MIT OR Apache-2.0). An optional U8g2 backend lives
//! behind the `u8g2` feature; its fonts carry separate licenses.

use embedded_graphics::mono_font::MonoFont;
use embedded_graphics::mono_font::ascii::{FONT_6X10, FONT_8X13, FONT_10X20};

use crate::geom::Size;

/// One of the bundled monospaced fonts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextStyle {
    /// 6x10 - dense labels and hints.
    Small,
    /// 8x13 - menu rows and body text.
    #[default]
    Medium,
    /// 10x20 - big readouts and titles.
    Large,
}

impl TextStyle {
    /// The underlying `embedded-graphics` font.
    pub const fn font(self) -> &'static MonoFont<'static> {
        match self {
            Self::Small => &FONT_6X10,
            Self::Medium => &FONT_8X13,
            Self::Large => &FONT_10X20,
        }
    }

    /// Width of one glyph cell in pixels.
    pub const fn glyph_width(self) -> u16 {
        self.font().character_size.width as u16
    }

    /// Advance between glyph origins, including inter-character spacing.
    pub const fn advance(self) -> u16 {
        self.glyph_width() + self.font().character_spacing as u16
    }

    /// Height of one line in pixels.
    pub const fn line_height(self) -> u16 {
        self.font().character_size.height as u16
    }

    /// Baseline offset from the top of the glyph cell.
    pub const fn baseline(self) -> u16 {
        self.font().baseline as u16
    }
}

/// Measures `text` rendered with `style` (single line, no wrapping).
///
/// Non-ASCII characters are counted as one cell; the bundled fonts only contain
/// ASCII, so they render as the font's fallback glyph.
pub fn measure(text: &str, style: TextStyle) -> Size {
    let cells = text.chars().count() as u16;
    // The advance includes spacing after every character including the last,
    // but the visible width of the run is one spacing less.
    let w = if cells == 0 {
        0
    } else {
        cells
            .saturating_mul(style.advance())
            .saturating_sub(style.font().character_spacing as u16)
    };
    Size::new(w, style.line_height())
}

/// Length of the longest line in `text` when rendered with `style`.
pub fn measure_multiline(text: &str, style: TextStyle) -> Size {
    let mut w = 0u16;
    let mut lines = 0u16;
    for line in text.split('\n') {
        w = w.max(measure(line, style).w);
        lines = lines.saturating_add(1);
    }
    Size::new(w, lines.saturating_mul(style.line_height()))
}

/// Maximum number of bytes [`format_ms_x10`] and friends can produce.
pub const FORMAT_BUF: usize = 16;

/// Formats a tenths-of-a-millisecond value as `"31.2 ms"`.
///
/// The buffer is returned as a `&str`; nothing is allocated.
pub fn format_ms_x10(value_x10: u16, buf: &mut [u8; FORMAT_BUF]) -> &str {
    let mut n = 0;
    n += write_u16(&mut buf[n..], value_x10 / 10);
    n += push(&mut buf[n..], b'.');
    n += push(&mut buf[n..], b'0' + (value_x10 % 10) as u8);
    n += push(&mut buf[n..], b' ');
    n += push(&mut buf[n..], b'm');
    n += push(&mut buf[n..], b's');
    as_str(&buf[..n])
}

/// Formats a tenths value without a unit, e.g. `"31.2"`.
pub fn format_x10(value_x10: u16, buf: &mut [u8; FORMAT_BUF]) -> &str {
    let mut n = 0;
    n += write_u16(&mut buf[n..], value_x10 / 10);
    n += push(&mut buf[n..], b'.');
    n += push(&mut buf[n..], b'0' + (value_x10 % 10) as u8);
    as_str(&buf[..n])
}

/// Formats a `u16` as decimal digits.
pub fn format_u16(value: u16, buf: &mut [u8; FORMAT_BUF]) -> &str {
    let n = write_u16(buf, value);
    as_str(&buf[..n])
}

/// Formats a `0..=100` percentage as e.g. `"87%"`.
pub fn format_percent(value: u8, buf: &mut [u8; FORMAT_BUF]) -> &str {
    let mut n = write_u16(buf, (value.min(100)) as u16);
    n += push(&mut buf[n..], b'%');
    as_str(&buf[..n])
}

fn as_str(bytes: &[u8]) -> &str {
    core::str::from_utf8(bytes).unwrap_or("?")
}

fn push(out: &mut [u8], byte: u8) -> usize {
    if out.is_empty() {
        return 0;
    }
    out[0] = byte;
    1
}

/// Writes `value` as decimal digits, returning the number of bytes written.
fn write_u16(out: &mut [u8], mut value: u16) -> usize {
    let mut digits = [0u8; 5];
    let mut count = 0;
    if value == 0 {
        return push(out, b'0');
    }
    while value > 0 {
        digits[count] = b'0' + (value % 10) as u8;
        value /= 10;
        count += 1;
    }
    let mut n = 0;
    while count > 0 {
        count -= 1;
        n += push(&mut out[n..], digits[count]);
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_31_2_ms() {
        let mut buf = [0u8; FORMAT_BUF];
        assert_eq!(format_ms_x10(312, &mut buf), "31.2 ms");
    }

    #[test]
    fn formats_zero() {
        let mut buf = [0u8; FORMAT_BUF];
        assert_eq!(format_ms_x10(0, &mut buf), "0.0 ms");
    }

    #[test]
    fn formats_max() {
        let mut buf = [0u8; FORMAT_BUF];
        assert_eq!(format_ms_x10(u16::MAX, &mut buf), "6553.5 ms");
    }

    #[test]
    fn formats_plain_and_percent() {
        let mut buf = [0u8; FORMAT_BUF];
        assert_eq!(format_u16(0, &mut buf), "0");
        assert_eq!(format_u16(45, &mut buf), "45");
        assert_eq!(format_u16(u16::MAX, &mut buf), "65535");
        assert_eq!(format_x10(312, &mut buf), "31.2");
        assert_eq!(format_percent(87, &mut buf), "87%");
        assert_eq!(format_percent(200, &mut buf), "100%");
    }

    #[test]
    fn measures_monospace_text() {
        assert_eq!(measure("", TextStyle::Medium), Size::new(0, 13));
        // FONT_8X13 has no extra spacing: 4 chars * 8 px.
        assert_eq!(measure("ABCD", TextStyle::Medium).w, 32);
        assert_eq!(measure("ABCD", TextStyle::Small).w, 24);
        assert_eq!(measure("ABCD", TextStyle::Large).w, 40);
    }

    #[test]
    fn measures_multiline() {
        let size = measure_multiline("ABC\nAB", TextStyle::Small);
        assert_eq!(size.w, 18);
        assert_eq!(size.h, 20);
    }

    #[test]
    fn font_metrics_match_embedded_graphics() {
        assert_eq!(TextStyle::Small.line_height(), 10);
        assert_eq!(TextStyle::Medium.line_height(), 13);
        assert_eq!(TextStyle::Large.line_height(), 20);
        assert!(TextStyle::Large.baseline() <= TextStyle::Large.line_height());
    }
}
