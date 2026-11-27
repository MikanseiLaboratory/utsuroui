//! # utsuroui-demo
//!
//! A lagbuster-style two-button demo application for [`utsuroui`], shared by the
//! host golden-image tests, the PNG screenshot tool and the RP2040/RP2350
//! firmware example.
//!
//! Screens:
//!
//! * **Menu** — three rows with a yellow highlight that slides in 120 ms.
//! * **Measure** — big live `31.2 ms` readout, min/avg/max and a 32-bar graph.
//! * **Settings** — update rate, transition style and bar scale.
//! * **About** — version and license.
//!
//! The footer is shared: the latest latency, an animated number, and a bar.
//!
//! [`render`] renders a full frame through the same band pipeline the firmware
//! uses, so a golden image is byte-identical to what the panel would receive.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod about;
pub mod app;
pub mod latency;
pub mod measure;
pub mod menu;
pub mod render;
pub mod settings;
pub mod style;

pub use app::{DemoApp, NavMode, ScreenId};
pub use latency::synthetic;
pub use measure::MeasureState;
pub use menu::{MenuAction, MenuState};
pub use settings::SettingsState;
pub use style::{H, SCREEN, W};
