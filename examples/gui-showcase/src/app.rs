//! A small, polished UI built from the framework's animated widgets.
//!
//! Two screens share one design: a card list whose selection bar slides, and a
//! controls screen with two switches and a slider. A banner slides up when a
//! value changes, and screen changes slide horizontally.
//!
//! Everything the user can see is animated by dirty rectangles: the selection
//! bar repaints the union of its old and new row, the switch repaints its row
//! while the knob moves, and the slider repaints its track while the fill
//! catches up. When nothing moves, no SPI traffic happens at all.

use utsuroui::anim::{Easing, ease};
use utsuroui::app::App;
use utsuroui::dirty::DirtyList;
use utsuroui::event::Event;
use utsuroui::geom::{Point, Rect};
use utsuroui::paint::PaintCx;
use utsuroui::text::{FORMAT_BUF, TextStyle, format_u16, measure};
use utsuroui::widget::{
    Highlight, HighlightState, Slider, SliderState, Toast, ToastState, Toggle, ToggleState, Widget,
};

use crate::style::*;

/// Number of cards on the home screen.
const ROWS: u8 = 4;
/// Card titles and subtitles.
const ITEMS: [(&str, &str); 4] = [
    ("Controls", "switches and a slider"),
    ("Network", "status and address"),
    ("Display", "brightness, inversion"),
    ("About", "version and license"),
];

const LIST: Rect = Rect::new(MARGIN, LIST_TOP, CONTENT_W, ROW_H * ROWS as u16);
const TOGGLE_0: Rect = Rect::new(MARGIN, 64, CONTENT_W, 40);
const TOGGLE_1: Rect = Rect::new(MARGIN, 112, CONTENT_W, 40);
const SLIDER_ROW: Rect = Rect::new(MARGIN, 168, CONTENT_W, 44);
const READOUT: Rect = Rect::new(MARGIN, 224, CONTENT_W, 28);
const TOAST_ROW: Rect = Rect::new(MARGIN, 258, CONTENT_W, 28);
const SLIDE_US: u64 = 180_000;

/// The two screens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    /// Card list.
    Home,
    /// Switches and a slider.
    Controls,
}

impl Screen {
    const fn title(self) -> &'static str {
        match self {
            Self::Home => "Home",
            Self::Controls => "Controls",
        }
    }
}

/// Horizontal slide between two screens.
#[derive(Clone, Copy, Debug)]
struct Slide {
    from: Screen,
    to: Screen,
    start_us: u64,
    active: bool,
}

/// The showcase application.
pub struct GuiApp {
    screen: Screen,
    slide: Slide,
    highlight: Highlight,
    highlight_state: HighlightState,
    toggles: [Toggle; 2],
    toggle_states: [ToggleState; 2],
    slider: Slider,
    slider_state: SliderState,
    toast: Toast,
    toast_state: ToastState,
    focus: u8,
}

impl Default for GuiApp {
    fn default() -> Self {
        Self::new()
    }
}

impl GuiApp {
    /// Starts on the home screen with the first card selected.
    pub fn new() -> Self {
        let mut app = Self {
            screen: Screen::Home,
            slide: Slide {
                from: Screen::Home,
                to: Screen::Home,
                start_us: 0,
                active: false,
            },
            highlight: Highlight::new(ROWS, ROW_H, 140_000, CARD_SELECTED, ACCENT),
            highlight_state: HighlightState::default(),
            toggles: [
                Toggle::new(140_000, ACCENT, TRACK_OFF, TEXT),
                Toggle::new(140_000, ACCENT, TRACK_OFF, TEXT),
            ],
            toggle_states: [ToggleState::with_value(true), ToggleState::default()],
            slider: Slider::new(0, 100, 5, 160_000),
            slider_state: SliderState::with_value(60),
            toast: Toast::plain(28),
            toast_state: ToastState::default(),
            focus: 0,
        };
        // Widgets normally get their bounds from a layout pass; arrange the ones
        // that are not re-arranged every frame.
        app.toast.arrange(TOAST_ROW, &mut app.toast_state);
        app.highlight.arrange(LIST, &mut app.highlight_state);
        app
    }

    fn go(&mut self, to: Screen, now_us: u64, dirty: &mut DirtyList) {
        if to == self.screen && !self.slide.active {
            return;
        }
        self.slide = Slide {
            from: self.screen,
            to,
            start_us: now_us,
            active: true,
        };
        self.screen = to;
        dirty.mark_full();
    }

    fn announce(&mut self, text: &'static str, now_us: u64, dirty: &mut DirtyList) {
        self.toast.show(&mut self.toast_state, text, now_us);
        dirty.add(TOAST_ROW);
    }

    fn slide_dx(&self, now_us: u64) -> Option<i16> {
        if !self.slide.active {
            return None;
        }
        let elapsed = now_us.saturating_sub(self.slide.start_us);
        let progress = (elapsed.min(SLIDE_US) * 65_535 / SLIDE_US) as u16;
        let eased = ease(Easing::EaseInOut, progress) as i32;
        Some((eased * W as i32 / 65_535) as i16)
    }

    /// Paints `screen` shifted by `dx` pixels.
    fn paint_screen(&self, cx: &mut PaintCx<'_>, screen: Screen, dx: i16, now_us: u64) {
        let shifted = |r: Rect| r.translate(dx, 0);

        // Header: title on the left, page dots on the right.
        cx.text(
            screen.title(),
            Point::new(shifted(Rect::new(MARGIN, 0, 0, 0)).x, 8),
            TextStyle::Large,
            TEXT,
        );
        let dots = Rect::new(shifted(Rect::new(W as i16 - 44, 14, 0, 0)).x, 14, 36, 8);
        for dot in 0..2u8 {
            let x = dots.x + dot as i16 * 16;
            let color = if (dot == 0) == (screen == Screen::Home) {
                ACCENT
            } else {
                MUTED
            };
            cx.fill_rect(Rect::new(x, dots.y, 10, 4), color);
        }
        cx.hline(HEADER_H as i16 - 1, 0, W as i16 - 1, MUTED);

        match screen {
            Screen::Home => self.paint_home(cx, dx, now_us),
            Screen::Controls => self.paint_controls(cx, dx, now_us),
        }

        // Footer hint (only when the screen is settled).
        if dx == 0 {
            let hint = match screen {
                Screen::Home => "MENU/PREV move  INPUT1 open",
                Screen::Controls => "MENU/PREV move  INPUT1 change",
            };
            cx.text(hint, Point::new(MARGIN, FOOTER_Y), TextStyle::Small, MUTED);
            cx.text(
                "BACK: home",
                Point::new(MARGIN, FOOTER_Y + 12),
                TextStyle::Small,
                MUTED,
            );
        }
    }

    fn paint_home(&self, cx: &mut PaintCx<'_>, dx: i16, now_us: u64) {
        let mut state = self.highlight_state;
        self.highlight.arrange(LIST.translate(dx, 0), &mut state);
        // The bar goes underneath the card content.
        self.highlight.paint(cx, &state, now_us);
        for row in 0..ROWS {
            let area = self.highlight.row_rect(&state, row).inset(4);
            let fill = if row == state.index {
                CARD_SELECTED
            } else {
                CARD
            };
            cx.fill_rect(area, fill);
            cx.stroke_rect(area, if row == state.index { ACCENT } else { MUTED });

            // A simple glyph: a rounded square with two bars.
            let icon = Rect::new(area.x + 10, area.y + 10, 28, 28);
            cx.stroke_rect(icon, if row == state.index { ACCENT } else { MUTED });
            cx.fill_rect(
                Rect::new(icon.x + 6, icon.y + 9, 16, 3),
                if row == state.index { ACCENT } else { MUTED },
            );
            cx.fill_rect(
                Rect::new(icon.x + 6, icon.y + 16, 10, 3),
                if row == state.index { ACCENT } else { MUTED },
            );

            let (title, subtitle) = ITEMS[row as usize];
            cx.text(
                title,
                Point::new(area.x + 48, area.y + 9),
                TextStyle::Medium,
                if row == state.index { ACCENT } else { TEXT },
            );
            cx.text(
                subtitle,
                Point::new(area.x + 48, area.y + 26),
                TextStyle::Small,
                MUTED,
            );
        }
    }

    fn paint_controls(&self, cx: &mut PaintCx<'_>, dx: i16, now_us: u64) {
        let mut states = self.toggle_states;
        let rows = [TOGGLE_0, TOGGLE_1];
        let labels = ["Haptics", "Auto-save"];

        for (index, (widget, row)) in self.toggles.iter().zip(rows).enumerate() {
            widget.arrange(row.translate(dx, 0), &mut states[index]);
            cx.text(
                labels[index],
                Point::new(row.x + 8 + dx, row.y + 14),
                TextStyle::Medium,
                TEXT,
            );
            widget.paint(cx, &states[index], now_us);
            if self.focus == index as u8 {
                cx.stroke_rect(row.translate(dx, 0).inset(1), ACCENT);
            }
        }

        let mut slider_state = self.slider_state;
        self.slider
            .arrange(SLIDER_ROW.translate(dx, 0), &mut slider_state);
        cx.text(
            "Brightness",
            Point::new(SLIDER_ROW.x + 8 + dx, SLIDER_ROW.y - 14),
            TextStyle::Small,
            MUTED,
        );
        self.slider.paint(cx, &slider_state, now_us);
        if self.focus == 2 {
            cx.stroke_rect(SLIDER_ROW.translate(dx, 0).inset(1), ACCENT);
        }

        // Big readout that eases towards the value, like the fill does.
        let value = self.slider.value(&slider_state);
        let mut buf = [0u8; FORMAT_BUF];
        let number = format_u16(value as u16, &mut buf);
        let size = measure("100 %", TextStyle::Large);
        let x = READOUT.x + dx + ((READOUT.w.saturating_sub(size.w)) / 2) as i16;
        cx.text(number, Point::new(x, READOUT.y), TextStyle::Large, ACCENT);
        cx.text(
            "%",
            Point::new(
                x + measure(number, TextStyle::Large).w as i16 + 4,
                READOUT.y + 6,
            ),
            TextStyle::Medium,
            MUTED,
        );
    }

    fn paint_chrome(&self, cx: &mut PaintCx<'_>, now_us: u64) {
        cx.fill_rect(SCREEN, BG);
        match self.slide_dx(now_us) {
            Some(dx) => {
                self.paint_screen(cx, self.slide.from, -dx, now_us);
                self.paint_screen(cx, self.slide.to, W as i16 - dx, now_us);
            }
            None => self.paint_screen(cx, self.screen, 0, now_us),
        }
        let state = self.toast_state;
        self.toast.paint(cx, &state, now_us);
    }
}

impl App for GuiApp {
    fn event(&mut self, event: &Event, now_us: u64, dirty: &mut DirtyList) {
        if self.slide.active {
            return;
        }

        match (self.screen, event) {
            (Screen::Home, Event::Next) => {
                let next = (self.highlight_state.index + 1) % ROWS;
                self.highlight
                    .select(&mut self.highlight_state, next, now_us);
                dirty.add(LIST);
            }
            (Screen::Home, Event::Previous) => {
                let prev = if self.highlight_state.index == 0 {
                    ROWS - 1
                } else {
                    self.highlight_state.index - 1
                };
                self.highlight
                    .select(&mut self.highlight_state, prev, now_us);
                dirty.add(LIST);
            }
            (Screen::Home, Event::Select) => {
                if self.highlight_state.index == 0 {
                    self.go(Screen::Controls, now_us, dirty);
                } else {
                    self.announce("Not wired yet", now_us, dirty);
                }
            }

            (Screen::Controls, Event::Next | Event::Previous) => {
                let step = if matches!(event, Event::Next) { 1 } else { -1 };
                self.focus = (self.focus as i8 + step).rem_euclid(3) as u8;
                dirty.add(TOGGLE_0);
                dirty.add(TOGGLE_1);
                dirty.add(SLIDER_ROW);
            }
            (Screen::Controls, Event::Select) => match self.focus {
                0 | 1 => {
                    let index = self.focus as usize;
                    let on = self.toggles[index].toggle(&mut self.toggle_states[index], now_us);
                    let rows = [TOGGLE_0, TOGGLE_1];
                    dirty.add(rows[index]);
                    self.announce(
                        match (index, on) {
                            (0, true) => "Haptics on",
                            (0, false) => "Haptics off",
                            (_, true) => "Auto-save on",
                            (_, false) => "Auto-save off",
                        },
                        now_us,
                        dirty,
                    );
                }
                _ => {
                    let value = self.slider.nudge(&mut self.slider_state, 1, now_us);
                    dirty.add(SLIDER_ROW);
                    dirty.add(READOUT);
                    self.announce(
                        if value >= 100 {
                            "Brightness maximum"
                        } else {
                            "Brightness up"
                        },
                        now_us,
                        dirty,
                    );
                }
            },
            (Screen::Controls, Event::LongPress) => {
                if self.focus == 2 {
                    self.slider.nudge(&mut self.slider_state, -1, now_us);
                    dirty.add(SLIDER_ROW);
                    dirty.add(READOUT);
                    self.announce("Brightness down", now_us, dirty);
                }
            }

            (_, Event::Back) => match self.screen {
                Screen::Controls => self.go(Screen::Home, now_us, dirty),
                Screen::Home => self.announce("Already home", now_us, dirty),
            },
            _ => {}
        }
    }

    fn tick(&mut self, now_us: u64, dirty: &mut DirtyList) -> bool {
        let mut animating = false;

        if self.slide.active {
            if now_us.saturating_sub(self.slide.start_us) >= SLIDE_US {
                self.slide.active = false;
            }
            dirty.mark_full();
            animating = true;
        }

        // Only the visible screen animates.
        if !self.slide.active {
            let mut state = self.highlight_state;
            if self.highlight.tick(&mut state, now_us, dirty) {
                self.highlight_state = state;
                animating = true;
            }
            if self.screen == Screen::Controls {
                for index in 0..2 {
                    let mut state = self.toggle_states[index];
                    if self.toggles[index].tick(&mut state, now_us, dirty) {
                        self.toggle_states[index] = state;
                        animating = true;
                    }
                }
                let mut state = self.slider_state;
                if self.slider.tick(&mut state, now_us, dirty) {
                    // The readout follows the same animation.
                    dirty.add(READOUT);
                    self.slider_state = state;
                    animating = true;
                }
            }
        }

        if self.toast.tick(&mut self.toast_state, now_us, dirty) {
            animating = true;
        }

        animating
    }

    fn paint(&self, cx: &mut PaintCx<'_>, now_us: u64) {
        self.paint_chrome(cx, now_us);
    }
}
