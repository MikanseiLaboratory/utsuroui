//! Settings screen: three toggle rows that change the live demo behaviour.

use utsuroui::anim::Tween;
use utsuroui::event::Event;
use utsuroui::focus::Focus;
use utsuroui::geom::{Point, Rect};
use utsuroui::paint::PaintCx;
use utsuroui::text::TextStyle;

use utsuroui::color::BLACK;

use crate::style::{ACCENT, DIM, FG, ROW_HIGHLIGHT_US, SETTING_ROW_H, setting_rect};

/// Selectable latency update rates.
pub const RATES: [u16; 3] = [20, 30, 45];
/// Labels for [`RATES`].
pub const RATE_LABELS: [&str; 3] = ["20 Hz", "30 Hz", "45 Hz"];

/// Selectable transition styles.
pub const TRANSITIONS: [&str; 4] = ["Cut", "Fade", "Slide", "Crossfade"];
/// Labels for the bar scales (tenths of a millisecond).
pub const SCALE_LABELS: [&str; 3] = ["40 ms", "60 ms", "80 ms"];
/// Bar scales in tenths of a millisecond.
pub const SCALES: [u16; 3] = [400, 600, 800];

/// Retained state of the settings screen.
#[derive(Clone, Copy, Debug)]
pub struct SettingsState {
    /// Focused row.
    pub focus: Focus,
    /// Animated highlight Y (row-local).
    pub highlight_y: Tween,
    /// Index into [`RATES`] / [`RATE_LABELS`].
    pub rate_idx: u8,
    /// Index into [`TRANSITIONS`].
    pub transition_idx: u8,
    /// Index into [`SCALES`] / [`SCALE_LABELS`].
    pub scale_idx: u8,
}

impl Default for SettingsState {
    fn default() -> Self {
        Self {
            focus: Focus::new(3),
            highlight_y: Tween::settled(0),
            rate_idx: 1,
            transition_idx: 1,
            scale_idx: 1,
        }
    }
}

/// What changed after an event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsChange {
    /// Nothing changed.
    None,
    /// Focus moved; repaint the highlight path.
    FocusMoved,
    /// A value changed; repaint that row.
    ValueChanged(u8),
}

impl SettingsState {
    /// The configured update rate in Hz.
    pub fn rate_hz(&self) -> u16 {
        RATES[(self.rate_idx as usize).min(RATES.len() - 1)]
    }

    /// The configured bar scale in tenths of a millisecond.
    pub fn scale_x10(&self) -> u16 {
        SCALES[(self.scale_idx as usize).min(SCALES.len() - 1)]
    }

    /// Label of a setting row's value.
    pub fn value_label(&self, row: u8) -> &'static str {
        match row {
            0 => RATE_LABELS[(self.rate_idx as usize).min(RATE_LABELS.len() - 1)],
            1 => TRANSITIONS[(self.transition_idx as usize).min(TRANSITIONS.len() - 1)],
            _ => SCALE_LABELS[(self.scale_idx as usize).min(SCALE_LABELS.len() - 1)],
        }
    }

    /// Labels of the three rows.
    pub const ROW_LABELS: [&'static str; 3] = ["Update rate", "Transition", "Bar scale"];

    /// The highlight rectangle at a row-local Y.
    pub fn highlight_rect_at(&self, y: i32) -> Rect {
        Rect::new(
            crate::style::MARGIN,
            crate::style::SETTINGS_TOP + y as i16,
            crate::style::CONTENT_W,
            SETTING_ROW_H,
        )
    }

    /// Union of the start and target highlight rows.
    pub fn dirty_path(&self) -> Rect {
        self.highlight_rect_at(self.highlight_y.from())
            .union(self.highlight_rect_at(self.highlight_y.target()))
    }

    /// Handles an input event.
    pub fn handle(&mut self, event: Event, now_us: u64) -> SettingsChange {
        match event {
            Event::Next => {
                self.focus.next();
                self.retarget(now_us);
                SettingsChange::FocusMoved
            }
            Event::Previous => {
                self.focus.previous();
                self.retarget(now_us);
                SettingsChange::FocusMoved
            }
            Event::Encoder(steps) => {
                self.focus.apply_encoder(steps);
                self.retarget(now_us);
                SettingsChange::FocusMoved
            }
            Event::Touch { x, y, .. } => {
                let rows: [Rect; 3] = [setting_rect(0), setting_rect(1), setting_rect(2)];
                match self.focus.hit_test(&rows, Point::new(x, y)) {
                    Some(index) => {
                        self.focus.set_index(index);
                        self.retarget(now_us);
                        SettingsChange::FocusMoved
                    }
                    None => SettingsChange::None,
                }
            }
            Event::Select => {
                let row = self.focus.index();
                match row {
                    0 => self.rate_idx = (self.rate_idx + 1) % RATES.len() as u8,
                    1 => self.transition_idx = (self.transition_idx + 1) % TRANSITIONS.len() as u8,
                    _ => self.scale_idx = (self.scale_idx + 1) % SCALES.len() as u8,
                }
                SettingsChange::ValueChanged(row)
            }
            _ => SettingsChange::None,
        }
    }

    fn retarget(&mut self, now_us: u64) {
        self.highlight_y
            .retarget(now_us, Self::row_y(self.focus.index()), ROW_HIGHLIGHT_US);
    }

    /// Row-local Y of `index`.
    pub const fn row_y(index: u8) -> i32 {
        index as i32 * SETTING_ROW_H as i32
    }

    /// Advances the highlight tween.
    pub fn tick(&mut self, now_us: u64) -> bool {
        self.highlight_y.is_running(now_us)
    }

    /// Paints the settings rows.
    pub fn paint(&self, cx: &mut PaintCx<'_>, origin: Point, now_us: u64) {
        let bar = Rect::new(
            origin.x + crate::style::MARGIN,
            origin.y + crate::style::SETTINGS_TOP + self.highlight_y.value_at(now_us) as i16,
            crate::style::CONTENT_W,
            SETTING_ROW_H,
        );

        for index in 0..3u8 {
            let rect = setting_rect(index).translate(origin.x, origin.y);
            cx.text_middle(
                Self::ROW_LABELS[index as usize],
                rect.x + 12,
                rect,
                TextStyle::Medium,
                FG,
            );
            cx.text_middle(
                self.value_label(index),
                rect.right() as i16 - 84,
                rect,
                TextStyle::Medium,
                FG,
            );
        }
        cx.fill_rect(bar, ACCENT);
        for index in 0..3u8 {
            let rect = setting_rect(index).translate(origin.x, origin.y);
            if rect.intersect(bar).is_some() {
                cx.with_clip(bar, |c| {
                    c.text_middle(
                        Self::ROW_LABELS[index as usize],
                        rect.x + 12,
                        rect,
                        TextStyle::Medium,
                        BLACK,
                    );
                    c.text_middle(
                        self.value_label(index),
                        rect.right() as i16 - 84,
                        rect,
                        TextStyle::Medium,
                        BLACK,
                    );
                });
            }
        }

        cx.text(
            "B: change   A hold: back",
            Point::new(origin.x + 16, crate::style::H as i16 - 92),
            TextStyle::Small,
            DIM,
        );
    }
}
