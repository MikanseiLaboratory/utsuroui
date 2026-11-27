//! The main menu: three rows and a sliding highlight bar.

use utsuroui::anim::Tween;
use utsuroui::event::Event;
use utsuroui::focus::Focus;
use utsuroui::geom::Rect;
use utsuroui::paint::PaintCx;
use utsuroui::text::TextStyle;

use utsuroui::color::BLACK;

use crate::style::{ACCENT, CONTENT_W, FG, MARGIN, ROW_H, ROW_HIGHLIGHT_US, row_rect};

/// Menu entries, in display order.
pub const ITEMS: &[&str] = &["Measure", "Settings", "About"];

/// What the menu asked the router to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuAction {
    /// Open the measurement screen.
    Measure,
    /// Open the settings screen.
    Settings,
    /// Open the about screen.
    About,
}

/// Retained state of the menu.
#[derive(Clone, Copy, Debug)]
pub struct MenuState {
    /// Focus index and item count.
    pub focus: Focus,
    /// Animated Y of the highlight bar (row-local, 0 = first row).
    pub highlight_y: Tween,
}

impl Default for MenuState {
    fn default() -> Self {
        Self {
            focus: Focus::new(ITEMS.len() as u8),
            highlight_y: Tween::settled(0),
        }
    }
}

impl MenuState {
    /// Y of row `index` relative to the first row.
    pub const fn row_y(index: u8) -> i32 {
        index as i32 * ROW_H as i32
    }

    /// The highlight rectangle at a given row-local Y.
    pub fn highlight_rect_at(&self, y: i32) -> Rect {
        Rect::new(MARGIN, crate::style::MENU_TOP + y as i16, CONTENT_W, ROW_H)
    }

    /// The rectangle union covering the current animation path.
    ///
    /// Dirty rectangles use the union of the start and target rows, so the
    /// whole path is repainted while the bar slides.
    pub fn dirty_path(&self) -> Rect {
        let a = self.highlight_rect_at(self.highlight_y.from());
        let b = self.highlight_rect_at(self.highlight_y.target());
        a.union(b)
    }

    /// Handles a navigation event. Returns the menu action on select.
    pub fn handle(&mut self, event: Event, now_us: u64) -> MenuActionChoice {
        match event {
            Event::Next => {
                self.focus.next();
                self.retarget(now_us);
                MenuActionChoice::Changed
            }
            Event::Previous => {
                self.focus.previous();
                self.retarget(now_us);
                MenuActionChoice::Changed
            }
            Event::Encoder(steps) => {
                self.focus.apply_encoder(steps);
                self.retarget(now_us);
                MenuActionChoice::Changed
            }
            Event::Touch { x, y, down } => {
                let rows: [Rect; 3] = [row_rect(0), row_rect(1), row_rect(2)];
                if let Some(index) = self.focus.hit_test(&rows, utsuroui::geom::Point::new(x, y)) {
                    self.focus.set_index(index);
                    self.retarget(now_us);
                    if down {
                        MenuActionChoice::Changed
                    } else {
                        MenuActionChoice::Select
                    }
                } else {
                    MenuActionChoice::Ignored
                }
            }
            Event::Select => MenuActionChoice::Select,
            _ => MenuActionChoice::Ignored,
        }
    }

    fn retarget(&mut self, now_us: u64) {
        self.highlight_y
            .retarget(now_us, Self::row_y(self.focus.index()), ROW_HIGHLIGHT_US);
    }

    /// Advances the highlight tween; returns whether it is still moving.
    pub fn tick(&mut self, now_us: u64) -> bool {
        self.highlight_y.is_running(now_us)
    }

    /// The current menu action for the focused row.
    pub fn action(&self) -> Option<MenuAction> {
        match self.focus.index() {
            0 => Some(MenuAction::Measure),
            1 => Some(MenuAction::Settings),
            2 => Some(MenuAction::About),
            _ => None,
        }
    }

    /// Paints the rows and the highlight bar.
    pub fn paint(&self, cx: &mut PaintCx<'_>, origin: utsuroui::geom::Point, now_us: u64) {
        let dx = origin.x;
        let dy = origin.y;
        let bar = Rect::new(
            dx + MARGIN,
            dy + crate::style::MENU_TOP + self.highlight_y.value_at(now_us) as i16,
            CONTENT_W,
            ROW_H,
        );

        // Labels are drawn twice: white everywhere, then black where the
        // yellow bar covers them. That keeps the text readable while the bar
        // is halfway between two rows.
        for (index, label) in ITEMS.iter().enumerate() {
            let rect = row_rect(index as u8).translate(dx, dy);
            cx.text_middle(label, rect.x + 12, rect, TextStyle::Medium, FG);
        }
        cx.fill_rect(bar, ACCENT);
        for (index, label) in ITEMS.iter().enumerate() {
            let rect = row_rect(index as u8).translate(dx, dy);
            if rect.intersect(bar).is_some() {
                cx.with_clip(bar, |c| {
                    c.text_middle(label, rect.x + 12, rect, TextStyle::Medium, BLACK)
                });
            }
        }
        // Footer hint on the menu itself.
        let hint_y = dy + crate::style::FOOTER_Y - 18;
        cx.text(
            "A: next   B: select   A hold: back",
            utsuroui::geom::Point::new(dx + MARGIN, hint_y),
            TextStyle::Small,
            FG,
        );
    }
}

/// Result of feeding one event to the menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuActionChoice {
    /// Nothing visible changed.
    Ignored,
    /// The focus moved; repaint the highlight path.
    Changed,
    /// The user selected the current row.
    Select,
}
