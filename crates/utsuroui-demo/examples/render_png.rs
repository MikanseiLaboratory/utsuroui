//! Renders the demo screens to PNG files.
//!
//! ```sh
//! cargo run -p utsuroui-demo --example render_png
//! ```
//!
//! Outputs:
//!
//! * `target/golden/*.png` — the golden-image frames (240x240).
//! * `docs/images/*.png` — 2x screenshots for the README.
//!
//! Regenerating the committed goldens is the job of
//! `UPDATE_GOLDEN=1 cargo test -p utsuroui-demo --test golden`; this example is
//! for inspection and documentation.

use std::path::{Path, PathBuf};

use utsuroui::app::App;
use utsuroui::color::{Rgb565, raw_u16};
use utsuroui::dirty::Dirty;
use utsuroui::event::Event;
use utsuroui_demo::app::{DemoApp, NavMode, ScreenId};
use utsuroui_demo::render::{FRAME_LEN, render_frame};
use utsuroui_demo::style::{H, SCREEN, W};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
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

fn frame_to_rgb(frame: &[u8], scale: u32) -> Vec<u8> {
    let sw = W as usize * scale as usize;
    let sh = H as usize * scale as usize;
    let mut out = vec![0u8; sw * sh * 3];
    for y in 0..H as usize {
        for x in 0..W as usize {
            let idx = (y * W as usize + x) * 2;
            let raw = u16::from_be_bytes([frame[idx], frame[idx + 1]]);
            let rgb = expand(utsuroui::color::from_raw_u16(raw));
            for dy in 0..scale as usize {
                for dx in 0..scale as usize {
                    let o = ((y * scale as usize + dy) * sw + (x * scale as usize + dx)) * 3;
                    out[o..o + 3].copy_from_slice(&rgb);
                }
            }
        }
    }
    out
}

fn write_png(path: &Path, rgb: &[u8], width: u32, height: u32) {
    std::fs::create_dir_all(path.parent().expect("parent")).expect("create dir");
    let file = std::fs::File::create(path).expect("create png");
    let mut encoder = png::Encoder::new(file, width, height);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("png header");
    writer.write_image_data(rgb).expect("png data");
}

fn render(app: &DemoApp, now_us: u64) -> Vec<u8> {
    let mut frame = vec![0u8; FRAME_LEN];
    let mut buffers = utsuroui_demo::render::BandBuf::new();
    pollster::block_on(render_frame(app, now_us, &mut frame, &mut buffers));
    frame
}

fn navigate(app: &mut DemoApp, target: ScreenId) {
    app.set_nav_mode(NavMode::Cut);
    if target == ScreenId::Menu {
        return;
    }
    let mut dirty = Dirty::new(SCREEN);
    let steps = match target {
        ScreenId::Measure => 0,
        ScreenId::Settings => 1,
        ScreenId::About => 2,
        ScreenId::Menu => unreachable!(),
    };
    for _ in 0..steps {
        app.event(&Event::Next, 0, &mut dirty);
    }
    app.event(&Event::Select, 0, &mut dirty);
    assert_eq!(app.current(), target, "navigation helper failed");
}

fn main() {
    let root = workspace_root();
    let golden = root.join("target/golden");
    let docs = root.join("docs/images");

    // Plain screens, also used as README screenshots at 2x.
    let shots = [
        ("menu_initial", ScreenId::Menu),
        ("measure", ScreenId::Measure),
        ("settings", ScreenId::Settings),
        ("about", ScreenId::About),
    ];
    for (name, screen) in shots {
        let mut app = DemoApp::new();
        navigate(&mut app, screen);
        let frame = render(&app, 0);
        write_png(
            &golden.join(format!("{name}.png")),
            &frame_to_rgb(&frame, 1),
            W as u32,
            H as u32,
        );
        let big = frame_to_rgb(&frame, 2);
        write_png(
            &docs.join(format!("{name}.png")),
            &big,
            W as u32 * 2,
            H as u32 * 2,
        );
    }

    // Highlight halfway through its tween.
    let mut app = DemoApp::new();
    let mut dirty = Dirty::new(SCREEN);
    app.event(&Event::Next, 0, &mut dirty);
    let frame = render(&app, 60_000);
    write_png(
        &golden.join("menu_mid_tween.png"),
        &frame_to_rgb(&frame, 1),
        W as u32,
        H as u32,
    );

    // Fade, slide and crossfade midpoints.
    for (name, mode, at) in [
        ("fade_mid", NavMode::Fade, 45_000u64),
        ("slide_mid", NavMode::Slide, 90_000),
        ("crossfade_mid", NavMode::Crossfade, 90_000),
    ] {
        let mut app = DemoApp::new();
        app.set_nav_mode(mode);
        let mut dirty = Dirty::new(SCREEN);
        app.event(&Event::Select, 0, &mut dirty);
        let frame = render(&app, at);
        write_png(
            &golden.join(format!("{name}.png")),
            &frame_to_rgb(&frame, 1),
            W as u32,
            H as u32,
        );
    }

    println!("wrote {} and {}", golden.display(), docs.display());
}
