//! Optional U8g2 text backend (`u8g2` feature).
//!
//! The crate code of `u8g2-fonts` is MIT OR Apache-2.0, but **the bundled fonts
//! are licensed individually** by the U8g2 project. The demo and the firmware in
//! this repository therefore use the `embedded-graphics` fonts only; applications
//! that enable this feature must record the license of every font they embed
//! (see `NOTICE` and `docs/FONT-LICENSES.md`).
//!
//! ```ignore
//! use utsuroui::u8g2::{U8g2TextStyle, fonts};
//!
//! let style = U8g2TextStyle::new(fonts::u8g2_font_6x10_tf, utsuroui::WHITE);
//! cx.text_with("Hello", pos, style, Baseline::Top);
//! ```

pub use u8g2_fonts;
pub use u8g2_fonts::U8g2TextStyle;
pub use u8g2_fonts::fonts;

pub use embedded_graphics::text::Baseline;
