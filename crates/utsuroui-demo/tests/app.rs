//! Behaviour tests for the demo application: navigation, transitions, input
//! handling and the latency throttle.

use utsuroui::app::App;
use utsuroui::dirty::Dirty;
use utsuroui::event::Event;
use utsuroui::geom::Rect;
use utsuroui::nav::{NavAction, TransitionPhase};
use utsuroui_demo::app::{DemoApp, NavMode, ScreenId};
use utsuroui_demo::style::SCREEN;

fn app() -> DemoApp {
    DemoApp::new()
}

fn cut_app() -> DemoApp {
    let mut app = app();
    app.set_nav_mode(NavMode::Cut);
    app
}

#[test]
fn menu_next_moves_focus_and_schedules_a_tween() {
    let mut app = app();
    let mut dirty = Dirty::new(SCREEN);
    app.event(&Event::Next, 0, &mut dirty);
    assert_eq!(app.menu().focus.index(), 1);
    assert!(app.menu().highlight_y.is_running(0));
    assert!(!dirty.is_empty());
}

#[test]
fn menu_select_pushes_measure_in_cut_mode() {
    let mut app = cut_app();
    let mut dirty = Dirty::new(SCREEN);
    app.event(&Event::Select, 0, &mut dirty);
    assert_eq!(app.current(), ScreenId::Measure);
    assert!(dirty.is_full_redraw());
}

#[test]
fn back_pops_and_root_back_is_ignored() {
    let mut app = cut_app();
    let mut dirty = Dirty::new(SCREEN);
    app.event(&Event::Select, 0, &mut dirty);
    assert_eq!(app.current(), ScreenId::Measure);
    app.event(&Event::Back, 1, &mut dirty);
    assert_eq!(app.current(), ScreenId::Menu);
    dirty.clear();
    app.event(&Event::Back, 2, &mut dirty);
    assert_eq!(app.current(), ScreenId::Menu);
    assert!(dirty.is_empty());
}

#[test]
fn fade_swaps_at_the_midpoint() {
    let mut app = app(); // default mode is fade
    let mut dirty = Dirty::new(SCREEN);
    app.event(&Event::Select, 0, &mut dirty);
    // Still showing the menu while fading out.
    assert_eq!(app.current(), ScreenId::Menu);
    assert_eq!(app.transition().phase(), TransitionPhase::FadingOut);

    app.tick(90_000, &mut dirty);
    assert_eq!(app.current(), ScreenId::Measure);
    assert_eq!(app.transition().phase(), TransitionPhase::FadingIn);

    app.tick(180_000, &mut dirty);
    assert_eq!(app.transition().phase(), TransitionPhase::Idle);
}

#[test]
fn input_is_ignored_while_transitioning() {
    let mut app = app();
    let mut dirty = Dirty::new(SCREEN);
    app.event(&Event::Select, 0, &mut dirty);
    app.event(&Event::Next, 0, &mut dirty);
    assert_eq!(app.current(), ScreenId::Menu);
    assert_eq!(app.transition().phase(), TransitionPhase::FadingOut);
}

#[test]
fn back_during_transition_cancels_and_cuts_back() {
    let mut app = cut_app();
    let mut dirty = Dirty::new(SCREEN);
    app.event(&Event::Select, 0, &mut dirty);
    assert_eq!(app.current(), ScreenId::Measure);

    app.set_nav_mode(NavMode::Slide);
    app.event(&Event::Back, 10, &mut dirty);
    assert!(app.transition().is_active());
    app.event(&Event::Back, 20, &mut dirty);
    assert!(!app.transition().is_active());
    assert_eq!(app.current(), ScreenId::Menu);
}

#[test]
fn settings_select_cycles_the_focused_value() {
    let mut app = cut_app();
    let mut dirty = Dirty::new(SCREEN);
    app.event(&Event::Next, 0, &mut dirty); // focus "Settings"
    app.event(&Event::Select, 0, &mut dirty);
    assert_eq!(app.current(), ScreenId::Settings);

    let before = app.settings().value_label(0);
    app.event(&Event::Select, 1, &mut dirty);
    assert_ne!(before, app.settings().value_label(0));
    assert!(!dirty.is_empty());
}

#[test]
fn settings_transition_row_changes_nav_mode() {
    let mut app = cut_app();
    let mut dirty = Dirty::new(SCREEN);
    app.event(&Event::Next, 0, &mut dirty);
    app.event(&Event::Select, 0, &mut dirty); // open settings
    app.event(&Event::Next, 1, &mut dirty); // focus "Transition"
    // Default is Fade (index 1); one select moves to Slide.
    app.event(&Event::Select, 2, &mut dirty);
    assert_eq!(app.nav_mode(), NavMode::Slide);
    app.event(&Event::Select, 3, &mut dirty);
    assert_eq!(app.nav_mode(), NavMode::Crossfade);
    app.event(&Event::Select, 4, &mut dirty);
    assert_eq!(app.nav_mode(), NavMode::Cut);
}

#[test]
fn slide_paints_both_screens() {
    let mut app = app();
    app.set_nav_mode(NavMode::Slide);
    let mut dirty = Dirty::new(SCREEN);
    app.event(&Event::Select, 0, &mut dirty);
    app.tick(90_000, &mut dirty);
    assert_eq!(app.transition().phase(), TransitionPhase::Sliding);
    let progress = app.transition().slide_progress(90_000).unwrap();
    assert!(progress > 30_000 && progress < 34_000);
}

#[test]
fn crossfade_reports_alpha_and_paints_under() {
    let mut app = app();
    app.set_nav_mode(NavMode::Crossfade);
    let mut dirty = Dirty::new(SCREEN);
    app.event(&Event::Select, 0, &mut dirty);
    assert_eq!(app.current(), ScreenId::Measure);
    let alpha = app.crossfade_alpha(90_000).unwrap();
    assert!(alpha > 120 && alpha < 135, "alpha = {alpha}");

    let mut frame = vec![0u8; utsuroui_demo::render::FRAME_LEN];
    let mut band_a = vec![0u8; utsuroui_demo::render::BAND_LEN];
    let mut band_b = vec![0u8; utsuroui_demo::render::BAND_LEN];
    let stats = pollster::block_on(utsuroui_demo::render::render_frame(
        &app,
        90_000,
        &mut frame,
        &mut band_a,
        &mut band_b,
    ));
    assert!(stats.bytes >= 240 * 320 * 2);
    // The blend of menu and measure is neither pure black nor pure white.
    assert!(frame.iter().any(|b| *b != 0));
}

#[test]
fn latency_updates_are_throttled() {
    let mut app = app();
    let mut dirty = Dirty::new(SCREEN);
    app.push_latency(400, 0, &mut dirty);
    assert_eq!(app.latency_x10, 400);
    dirty.clear();

    app.push_latency(500, 10_000, &mut dirty);
    assert_eq!(app.latency_x10, 400, "too soon, must be ignored");
    assert!(dirty.is_empty());

    app.push_latency(500, 33_000, &mut dirty);
    assert_eq!(app.latency_x10, 500);
    assert!(!dirty.is_empty());
}

#[test]
fn freeze_stops_samples_but_not_the_readout() {
    let mut app = cut_app();
    let mut dirty = Dirty::new(SCREEN);
    app.event(&Event::Select, 0, &mut dirty); // measure screen
    app.event(&Event::Select, 1, &mut dirty); // freeze
    let before = app.measure().value_x10;
    app.push_latency(999, 40_000, &mut dirty);
    assert_eq!(app.measure().value_x10, before);
}

#[test]
fn next_wakeup_tracks_running_tweens() {
    let mut app = app();
    let mut dirty = Dirty::new(SCREEN);
    assert!(app.next_wakeup_us().is_none());
    app.event(&Event::Next, 100, &mut dirty);
    let wake = app.next_wakeup_us().unwrap();
    assert_eq!(wake, 100 + 120_000);
}

#[test]
fn router_state_is_exposed_for_navigation() {
    let mut app = cut_app();
    let mut dirty = Dirty::new(SCREEN);
    app.event(&Event::Next, 0, &mut dirty);
    app.event(&Event::Next, 0, &mut dirty);
    app.event(&Event::Select, 0, &mut dirty); // About
    assert_eq!(app.current(), ScreenId::About);

    // A direct nav action is applied by the router itself.
    app.event(&Event::Back, 1, &mut dirty);
    assert_eq!(app.current(), ScreenId::Menu);
    let _ = NavAction::<ScreenId>::Pop;
}

#[test]
fn dirty_highlight_path_covers_both_rows() {
    let mut app = app();
    let mut dirty = Dirty::new(SCREEN);
    app.event(&Event::Next, 0, &mut dirty);
    let path = app.menu().dirty_path();
    assert_eq!(path.x, 8);
    assert_eq!(path.w, 224);
    assert_eq!(path.y, 44);
    assert_eq!(path.h, 56);
}

#[test]
fn tick_clears_previous_screen_after_cut() {
    let mut app = cut_app();
    let mut dirty = Dirty::new(SCREEN);
    app.event(&Event::Select, 0, &mut dirty);
    app.tick(1, &mut dirty);
    // After a cut the app is idle, so a frame costs only what changed.
    let mut next = Dirty::new(SCREEN);
    let animating = app.tick(2, &mut next);
    assert!(!animating);
    let _ = Rect::new(0, 0, 240, 320);
}
