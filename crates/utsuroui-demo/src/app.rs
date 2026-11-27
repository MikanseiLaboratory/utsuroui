//! The demo application: menu, measurement, settings and about screens wired
//! to a router, transitions and tweens.
//!
//! All screen states are kept alive for the lifetime of the app. That is what
//! lets a slide or a crossfade paint the outgoing screen again instead of
//! keeping a 153,600-byte snapshot in RAM.

use utsuroui::app::App;
use utsuroui::dirty::DirtyList;
use utsuroui::event::Event;
use utsuroui::geom::{Point, Rect};
use utsuroui::nav::{
    NavAction, Router, SlideDir, Transition, TransitionKind, TransitionPhase, Trigger,
};
use utsuroui::paint::PaintCx;
use utsuroui::text::{FORMAT_BUF, TextStyle, format_ms_x10};

use crate::about;
use crate::measure::MeasureState;
use crate::menu::{MenuAction, MenuActionChoice, MenuState};
use crate::settings::{SettingsChange, SettingsState};
use crate::style::{
    ACCENT, BAR_H, BAR_RECT, BAR_Y, BG, CROSSFADE_US, DEFAULT_SCALE_X10, DIM, FADE_US, FG,
    FOOTER_Y, HEADER_H, LATENCY_INTERVAL_US, MARGIN, READOUT_RECT, READOUT_TWEEN_US, READOUT_Y,
    SCREEN, SLIDE_US, W,
};

/// Screen identifiers; the router stores these.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScreenId {
    /// Menu screen.
    Menu,
    /// Measurement screen.
    Measure,
    /// Settings screen.
    Settings,
    /// About screen.
    About,
}

impl ScreenId {
    /// Header title for this screen.
    pub const fn title(self) -> &'static str {
        match self {
            Self::Menu => crate::style::TITLE,
            Self::Measure => "MEASURE",
            Self::Settings => "SETTINGS",
            Self::About => "ABOUT",
        }
    }

    fn from_menu(action: MenuAction) -> Self {
        match action {
            MenuAction::Measure => Self::Measure,
            MenuAction::Settings => Self::Settings,
            MenuAction::About => Self::About,
        }
    }
}

/// How screen changes are animated.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NavMode {
    /// Instant.
    Cut,
    /// Fade through black (default).
    #[default]
    Fade,
    /// Horizontal slide.
    Slide,
    /// Crossfade between the two screens.
    Crossfade,
}

impl NavMode {
    /// All modes in selection order.
    pub const ALL: [NavMode; 4] = [Self::Cut, Self::Fade, Self::Slide, Self::Crossfade];

    /// Human-readable label.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Cut => "Cut",
            Self::Fade => "Fade",
            Self::Slide => "Slide",
            Self::Crossfade => "Crossfade",
        }
    }

    /// Transition timing for this mode.
    pub const fn kind(self) -> TransitionKind {
        match self {
            Self::Cut => TransitionKind::Cut,
            Self::Fade => TransitionKind::Fade {
                out_us: FADE_US,
                in_us: FADE_US,
            },
            Self::Slide => TransitionKind::Slide {
                dir: SlideDir::Horizontal,
                dur_us: SLIDE_US,
            },
            Self::Crossfade => TransitionKind::Crossfade {
                dur_us: CROSSFADE_US,
            },
        }
    }
}

/// The demo application.
pub struct DemoApp {
    router: Router<ScreenId, 4>,
    transition: Transition<ScreenId>,
    nav_mode: NavMode,
    menu: MenuState,
    measure: MeasureState,
    settings: SettingsState,
    /// The outgoing screen while a slide/crossfade is running.
    prev: Option<ScreenId>,
    /// Animated footer readout.
    shown_x10: utsuroui::anim::Tween,
    last_sample_us: Option<u64>,
    /// Latest latency sample in tenths of a millisecond.
    pub latency_x10: u16,
    /// Bar full scale in tenths of a millisecond.
    pub scale_x10: u16,
}

impl Default for DemoApp {
    fn default() -> Self {
        let measure = MeasureState::default();
        let nav_mode = NavMode::Fade;
        Self {
            router: Router::new(ScreenId::Menu),
            transition: Transition::new(nav_mode.kind()),
            nav_mode,
            menu: MenuState::default(),
            measure,
            settings: SettingsState::default(),
            prev: None,
            shown_x10: utsuroui::anim::Tween::settled(measure.value_x10 as i32),
            last_sample_us: None,
            latency_x10: measure.value_x10,
            scale_x10: DEFAULT_SCALE_X10,
        }
    }
}

impl DemoApp {
    /// Creates the app in its default state.
    pub fn new() -> Self {
        Self::default()
    }

    /// The visible screen.
    pub fn current(&self) -> ScreenId {
        self.router.current()
    }

    /// The active transition mode.
    pub fn nav_mode(&self) -> NavMode {
        self.nav_mode
    }

    /// Switches the transition mode (also used by the settings screen).
    pub fn set_nav_mode(&mut self, mode: NavMode) {
        self.nav_mode = mode;
        self.transition.set_kind(mode.kind());
    }

    /// The transition state machine (for tests).
    pub fn transition(&self) -> &Transition<ScreenId> {
        &self.transition
    }

    /// The measurement screen state (for tests).
    pub fn measure(&self) -> &MeasureState {
        &self.measure
    }

    /// The menu state (for tests).
    pub fn menu(&self) -> &MenuState {
        &self.menu
    }

    /// The settings state (for tests).
    pub fn settings(&self) -> &SettingsState {
        &self.settings
    }

    /// Pushes a latency sample from the measurement side.
    pub fn push_latency(&mut self, value_x10: u16, now_us: u64, dirty: &mut DirtyList) {
        self.latency(value_x10, now_us, dirty);
    }

    fn navigate(&mut self, action: NavAction<ScreenId>, now_us: u64, dirty: &mut DirtyList) {
        let current = self.router.current();
        match self.transition.trigger(now_us, action) {
            Trigger::Immediate(action) => {
                self.apply_nav(action);
                self.prev = Some(current);
                dirty.mark_full();
            }
            Trigger::Deferred => {
                // The action is applied at the fade swap point in `tick`.
                dirty.mark_full();
            }
        }
    }

    fn apply_nav(&mut self, action: NavAction<ScreenId>) {
        self.router.apply(action);
    }

    fn event_menu(&mut self, event: Event, now_us: u64, dirty: &mut DirtyList) {
        let before = self.menu.dirty_path();
        match self.menu.handle(event, now_us) {
            MenuActionChoice::Ignored => {}
            MenuActionChoice::Changed => {
                dirty.add_union(before, self.menu.dirty_path());
            }
            MenuActionChoice::Select => {
                if let Some(action) = self.menu.action() {
                    let target = ScreenId::from_menu(action);
                    self.navigate(NavAction::Push(target), now_us, dirty);
                }
            }
        }
    }

    fn event_settings(&mut self, event: Event, now_us: u64, dirty: &mut DirtyList) {
        let before = self.settings.dirty_path();
        match self.settings.handle(event, now_us) {
            SettingsChange::None => {}
            SettingsChange::FocusMoved => {
                dirty.add_union(before, self.settings.dirty_path());
            }
            SettingsChange::ValueChanged(row) => {
                dirty.add(crate::style::setting_rect(row));
                self.apply_settings();
            }
        }
    }

    fn event_measure(&mut self, event: Event, dirty: &mut DirtyList) {
        if let Event::Select = event {
            self.measure.frozen = !self.measure.frozen;
            dirty.add(self.measure.dirty_rect());
        }
    }

    fn apply_settings(&mut self) {
        let mode = match self.settings.transition_idx {
            0 => NavMode::Cut,
            1 => NavMode::Fade,
            2 => NavMode::Slide,
            _ => NavMode::Crossfade,
        };
        self.nav_mode = mode;
        self.transition.set_kind(mode.kind());
        self.scale_x10 = self.settings.scale_x10();
    }

    fn paint_header(&self, cx: &mut PaintCx<'_>, id: ScreenId, origin: Point) {
        let title = id.title();
        cx.text(
            title,
            Point::new(origin.x + 12, origin.y + 8),
            TextStyle::Large,
            ACCENT,
        );
        let right = origin.x + W as i16 - 12;
        cx.text_right("240x320", right, origin.y + 12, TextStyle::Small, DIM);
        cx.hline(
            origin.y + (HEADER_H as i16 - 3),
            origin.x + MARGIN,
            origin.x + W as i16 - MARGIN,
            DIM,
        );
    }

    fn paint_footer(&self, cx: &mut PaintCx<'_>, origin: Point, now_us: u64) {
        let y = origin.y + FOOTER_Y;
        cx.hline(y, origin.x + MARGIN, origin.x + W as i16 - MARGIN, DIM);

        let value = self.shown_x10.value_at(now_us).clamp(0, u16::MAX as i32) as u16;
        let mut buf = [0u8; FORMAT_BUF];
        let text = format_ms_x10(value, &mut buf);
        cx.text(
            text,
            Point::new(origin.x + 12, origin.y + READOUT_Y),
            TextStyle::Large,
            FG,
        );

        let frame = Rect::new(origin.x + BAR_RECT.x, origin.y + BAR_Y, BAR_RECT.w, BAR_H);
        let fill_w = crate::style::bar_width(value, self.scale_x10);
        if fill_w > 0 {
            cx.fill_rect(
                Rect::new(frame.x + 1, frame.y + 1, fill_w, frame.h.saturating_sub(2)),
                ACCENT,
            );
        }
        cx.stroke_rect(frame, FG);
    }

    fn paint_screen(&self, cx: &mut PaintCx<'_>, id: ScreenId, origin: Point, now_us: u64) {
        cx.fill_rect(SCREEN.translate(origin.x, origin.y), BG);
        self.paint_header(cx, id, origin);
        match id {
            ScreenId::Menu => self.menu.paint(cx, origin, now_us),
            ScreenId::Measure => self.measure.paint(cx, origin, now_us, self.scale_x10),
            ScreenId::Settings => self.settings.paint(cx, origin, now_us),
            ScreenId::About => about::paint(cx, origin),
        }
        self.paint_footer(cx, origin, now_us);
    }
}

impl App for DemoApp {
    fn event(&mut self, event: &Event, now_us: u64, dirty: &mut DirtyList) {
        // A transition swallows input except Back, which aborts it with a cut
        // (DESIGN.md open question 3, recommended answer).
        if self.transition.is_active() {
            if matches!(event, Event::Back) {
                self.transition.cancel();
                self.prev = None;
                self.apply_nav(NavAction::Pop);
                dirty.mark_full();
            }
            return;
        }

        match event {
            Event::Back => {
                if self.router.can_pop() {
                    self.navigate(NavAction::Pop, now_us, dirty);
                }
            }
            Event::Next
            | Event::Previous
            | Event::Select
            | Event::Encoder(_)
            | Event::Touch { .. } => match self.router.current() {
                ScreenId::Menu => self.event_menu(*event, now_us, dirty),
                ScreenId::Settings => self.event_settings(*event, now_us, dirty),
                ScreenId::Measure => self.event_measure(*event, dirty),
                ScreenId::About => {}
            },
            Event::LongPress | Event::Repeat => {}
        }
    }

    fn tick(&mut self, now_us: u64, dirty: &mut DirtyList) -> bool {
        let mut animating = false;

        if self.transition.is_active() {
            if let Some(action) = self.transition.tick(now_us) {
                let old = self.router.current();
                self.apply_nav(action);
                self.prev = Some(old);
            }
            if !self.transition.is_active() {
                self.prev = None;
            }
            // Fades, slides and crossfades repaint the whole panel.
            dirty.mark_full();
            animating = true;
        } else if self.prev.is_some() {
            self.prev = None;
        }

        if self.menu.tick(now_us) {
            dirty.add(self.menu.dirty_path());
            animating = true;
        }
        if self.settings.tick(now_us) {
            dirty.add(self.settings.dirty_path());
            animating = true;
        }
        if self.measure.tick(now_us) {
            dirty.add(self.measure.dirty_rect());
            animating = true;
        }
        if self.shown_x10.is_running(now_us) {
            dirty.add(READOUT_RECT);
            animating = true;
        }
        animating
    }

    fn paint(&self, cx: &mut PaintCx<'_>, now_us: u64) {
        let current = self.router.current();
        let phase = self.transition.phase();

        if matches!(phase, TransitionPhase::Sliding) {
            if let Some(prev) = self.prev {
                let progress = self.transition.slide_progress(now_us).unwrap_or(65_535) as u32;
                let dx = (progress * W as u32 / 65_535) as i16;
                self.paint_screen(cx, prev, Point::new(-dx, 0), now_us);
                self.paint_screen(cx, current, Point::new(W as i16 - dx, 0), now_us);
                return;
            }
        }

        self.paint_screen(cx, current, Point::ZERO, now_us);

        if matches!(
            phase,
            TransitionPhase::FadingOut | TransitionPhase::FadingIn
        ) {
            let alpha = self.transition.overlay_alpha(now_us);
            if alpha > 0 {
                cx.fill_rect_alpha(SCREEN, utsuroui::BLACK, alpha);
            }
        }
    }

    fn paint_under(&self, cx: &mut PaintCx<'_>, now_us: u64) -> bool {
        match self.prev {
            Some(prev) if self.transition.crossfade_alpha(now_us).is_some() => {
                self.paint_screen(cx, prev, Point::ZERO, now_us);
                true
            }
            _ => false,
        }
    }

    fn crossfade_alpha(&self, now_us: u64) -> Option<u8> {
        self.transition.crossfade_alpha(now_us)
    }

    fn next_wakeup_us(&self) -> Option<u64> {
        [
            self.menu.highlight_y.end_us(),
            self.settings.highlight_y.end_us(),
            self.measure.shown_x10.end_us(),
            self.shown_x10.end_us(),
        ]
        .into_iter()
        .flatten()
        .max()
    }

    fn latency(&mut self, value_x10: u16, now_us: u64, dirty: &mut DirtyList) {
        if let Some(last) = self.last_sample_us {
            if now_us.saturating_sub(last) < LATENCY_INTERVAL_US {
                return;
            }
        }
        self.last_sample_us = Some(now_us);
        self.latency_x10 = value_x10;
        if self.measure.frozen {
            return;
        }
        if self.measure.push(value_x10, now_us) {
            dirty.add(self.measure.dirty_rect());
        }
        self.shown_x10
            .retarget(now_us, value_x10 as i32, READOUT_TWEEN_US);
        dirty.add(READOUT_RECT);
    }
}
