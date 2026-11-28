//! # utsuroui
//!
//! A small, allocation-free GUI for 240x320 RGB565 SPI TFTs (ST7789 / ILI9341)
//! on any Embassy target that can drive a display and provide timers.
//!
//! The core has no HAL dependency: it needs `embedded-graphics` for drawing and,
//! for the task scheduler, `embassy-sync` / `embassy-time`. Only the example
//! firmwares pick an MCU (RP2040/RP2350); the framework itself is portable. The
//! `lcd` feature adds an `lcd-async` sink, and any other panel wrapper only has
//! to implement [`band::BandSink`].
//!
//! The framework is built around three ideas:
//!
//! * **Dirty rectangles**: only what changed is painted and transferred. A
//!   highlight sliding by 16 px sends roughly 240x44 pixels, not the whole
//!   frame.
//! * **Fixed-point tweens**: every animation is an integer `Tween` whose
//!   `retarget` keeps motion continuous when a button is pressed again mid-way.
//! * **Static composition**: widgets are cheap descriptors, state is a plain
//!   struct that lives as long as the screen. There is no heap, no
//!   `dyn Widget`, and no double render tree.
//!
//! The application implements [`App`]; the renderer turns its [`DirtyList`]
//! into band transfers through a [`BandSink`]. On hardware the sink is an
//! `lcd-async` display (`lcd` feature); on the host it is an in-memory frame
//! buffer, which is what the golden-image tests use.
//!
//! ```
//! use utsuroui::geom::{Rect, Size};
//! use utsuroui::dirty::Dirty;
//!
//! let mut dirty = Dirty::new(Rect::from_size(Size::new(240, 320)));
//! dirty.add_union(Rect::new(8, 48, 224, 28), Rect::new(8, 76, 224, 28));
//! assert_eq!(dirty.len(), 1);
//! ```
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod anim;
pub mod app;
pub mod band;
pub mod color;
pub mod dirty;
pub mod event;
pub mod flush;
pub mod focus;
pub mod geom;
pub mod input;
pub mod nav;
pub mod paint;
pub mod text;
pub mod widget;

#[cfg(feature = "lcd")]
pub mod lcd;

#[cfg(feature = "u8g2")]
pub mod u8g2;

#[cfg(feature = "embassy")]
pub mod embassy_task;

pub use anim::{Easing, Orbit, Q16, Tween, ease, lerp_i32, wave};
pub use app::App;
pub use band::{BandBuffers, BandSink, MemorySink, blend_band};
pub use color::{BLACK, DARK_GRAY, GRAY, WHITE, YELLOW, be_bytes, blend, darken, lerp};
pub use dirty::{Dirty, DirtyList, MAX_DIRTY};
pub use event::{Event, EventOut};
pub use flush::{FlushStats, RenderCfg, flush};
#[cfg(feature = "pipeline")]
pub use flush::flush_pipelined;
pub use focus::Focus;
pub use geom::{Point, Rect, Size};
pub use input::{DebounceConfig, Debouncer, Edge, EncoderDecoder};
pub use nav::{NavAction, Router, Transition, TransitionKind, TransitionPhase, Trigger};
pub use paint::PaintCx;
pub use text::{
    FORMAT_BUF, TextStyle, format_ms_x10, format_percent, format_u16, format_x10, measure,
};
pub use widget::{
    Background, CrossAlign, Edges, Expanded, Fixed, HStack, Highlight, HighlightState, MainAlign,
    Marquee, MarqueeState, MeasureCx, Padding, Slider, SliderState, Toast, ToastState, Toggle,
    ToggleState, VStack,
    Widget,
};

/// The nominal screen size of the reference device.
pub const SCREEN: Rect = Rect::new(0, 0, 240, 320);
