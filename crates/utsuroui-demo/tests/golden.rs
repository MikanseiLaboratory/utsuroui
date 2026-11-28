//! Golden-image tests.
//!
//! Each case renders a full frame through the real band pipeline and compares
//! the RGB565 pixels with a committed PNG. Set `UPDATE_GOLDEN=1` to rewrite the
//! images (do that only after looking at them).

use std::path::{Path, PathBuf};

use utsuroui::app::App;
use utsuroui::color::{Rgb565, raw_u16};
use utsuroui::dirty::Dirty;
use utsuroui::event::Event;
use utsuroui_demo::app::{DemoApp, NavMode, ScreenId};
use utsuroui_demo::render::{FRAME_LEN, render_frame};
use utsuroui_demo::style::{H, SCREEN, W};

fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

fn expand(color: Rgb565) -> [u8; 3] {
    let raw = raw_u16(color);
    let r = ((raw >> 11) & 0x1F) as u8;
    let g = ((raw >> 5) & 0x3F) as u8;
    let b = (raw & 0x1F) as u8;
    [
        (r << 3) | (r >> 2),
        (g << 2) | (g >> 4),
        (b << 3) | (b >> 2),
    ]
}

fn frame_to_rgb(frame: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(W as usize * H as usize * 3);
    for i in (0..frame.len()).step_by(2) {
        let raw = u16::from_be_bytes([frame[i], frame[i + 1]]);
        out.extend_from_slice(&expand(utsuroui::color::from_raw_u16(raw)));
    }
    out
}

fn read_png(path: &Path) -> Vec<u8> {
    let file = std::fs::File::open(path).unwrap_or_else(|e| panic!("open {path:?}: {e}"));
    let decoder = png::Decoder::new(std::io::BufReader::new(file));
    let mut reader = decoder.read_info().expect("png header");
    let mut buf = vec![0u8; reader.output_buffer_size().expect("png size")];
    let info = reader.next_frame(&mut buf).expect("png data");
    assert_eq!(info.width as u16, W, "golden width for {path:?}");
    assert_eq!(info.height as u16, H, "golden height for {path:?}");
    assert_eq!(info.color_type, png::ColorType::Rgb, "golden must be RGB8");
    buf.truncate(info.buffer_size());
    buf
}

fn write_png(path: &Path, rgb: &[u8]) {
    let file = std::fs::File::create(path).expect("create golden");
    let mut encoder = png::Encoder::new(file, W as u32, H as u32);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("png header");
    writer.write_image_data(rgb).expect("png data");
}

fn render(app: &DemoApp, now_us: u64) -> Vec<u8> {
    let mut frame = vec![0u8; FRAME_LEN];
    let mut buffers = utsuroui_demo::render::BandBuf::new();
    pollster::block_on(render_frame(
        app,
        now_us,
        &mut frame,
        &mut buffers,
    ));
    frame
}

fn check(name: &str, app: &DemoApp, now_us: u64) {
    let frame = render(app, now_us);
    let rgb = frame_to_rgb(&frame);
    let path = golden_dir().join(format!("{name}.png"));
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(golden_dir()).expect("golden dir");
        write_png(&path, &rgb);
        return;
    }
    let expected = read_png(&path);
    assert_eq!(expected.len(), rgb.len(), "golden size mismatch for {name}");
    let diff = expected
        .iter()
        .zip(rgb.iter())
        .filter(|(a, b)| a != b)
        .count();
    if diff != 0 {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/golden-diff");
        std::fs::create_dir_all(&dir).ok();
        write_png(&dir.join(format!("{name}.png")), &rgb);
        panic!("{name}: {diff} differing RGB bytes (actual written to target/golden-diff)");
    }
}

fn navigate_to(app: &mut DemoApp, target: ScreenId) {
    app.set_nav_mode(NavMode::Cut);
    let mut dirty = Dirty::new(SCREEN);
    let steps = match target {
        ScreenId::Menu => 0,
        ScreenId::Measure => 0,
        ScreenId::Settings => 1,
        ScreenId::About => 2,
    };
    for _ in 0..steps {
        app.event(&Event::Next, 0, &mut dirty);
    }
    app.event(&Event::Select, 0, &mut dirty);
    assert_eq!(app.current(), target);
}

#[test]
fn golden_menu_initial() {
    let app = DemoApp::new();
    check("menu_initial", &app, 0);
}

#[test]
fn golden_menu_mid_tween() {
    let mut app = DemoApp::new();
    let mut dirty = Dirty::new(SCREEN);
    app.event(&Event::Next, 0, &mut dirty);
    // Halfway through the 120 ms highlight tween.
    check("menu_mid_tween", &app, 60_000);
}

#[test]
fn golden_measure() {
    let mut app = DemoApp::new();
    navigate_to(&mut app, ScreenId::Measure);
    check("measure", &app, 0);
}

#[test]
fn golden_settings() {
    let mut app = DemoApp::new();
    navigate_to(&mut app, ScreenId::Settings);
    check("settings", &app, 0);
}

#[test]
fn golden_about() {
    let mut app = DemoApp::new();
    navigate_to(&mut app, ScreenId::About);
    check("about", &app, 0);
}

#[test]
fn golden_fade_mid() {
    let mut app = DemoApp::new();
    app.set_nav_mode(NavMode::Fade);
    let mut dirty = Dirty::new(SCREEN);
    app.event(&Event::Select, 0, &mut dirty);
    // Midway through the 90 ms fade-out.
    check("fade_mid", &app, 45_000);
}

#[test]
fn golden_slide_mid() {
    let mut app = DemoApp::new();
    app.set_nav_mode(NavMode::Slide);
    let mut dirty = Dirty::new(SCREEN);
    app.event(&Event::Select, 0, &mut dirty);
    // Midway through the 180 ms slide.
    check("slide_mid", &app, 90_000);
}

#[test]
fn golden_crossfade_mid() {
    let mut app = DemoApp::new();
    app.set_nav_mode(NavMode::Crossfade);
    let mut dirty = Dirty::new(SCREEN);
    app.event(&Event::Select, 0, &mut dirty);
    // Midway through the 180 ms crossfade.
    check("crossfade_mid", &app, 90_000);
}

#[test]
fn screen_and_golden_geometry_is_240x320() {
    // The README screenshots and every golden use the same geometry.
    assert_eq!(SCREEN.w, W);
    assert_eq!(SCREEN.h, H);
    assert_eq!(FRAME_LEN, W as usize * H as usize * 2);
}
