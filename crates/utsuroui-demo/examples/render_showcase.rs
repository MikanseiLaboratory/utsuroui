//! Renders the `anim-showcase` scenes to PNG on the host, for inspection.
//!
//! ```sh
//! cargo run -p utsuroui-demo --example render_showcase
//! ```
//!
//! This drives the *same* scene code the firmware uses (`scenes.rs` is pulled in
//! with `#[path]`) through the same band pipeline, so a rendering bug shows up
//! here instead of only on the panel. Output goes to `target/showcase/*.png`.

#[path = "../../../examples/anim-showcase/src/scenes.rs"]
mod scenes;

use std::path::Path;

use utsuroui::app::App;
use utsuroui::band::MemorySink;
use utsuroui::color::{Rgb565, raw_u16};
use utsuroui::dirty::Dirty;
use utsuroui::event::Event;
use utsuroui::flush::{RenderCfg, flush};

const W: u16 = 240;
const H: u16 = 320;
const BAND_H: u16 = 16;
const BAND_LEN: usize = W as usize * BAND_H as usize * 2;
const FRAME_LEN: usize = W as usize * H as usize * 2;
const STEP_US: u64 = 16_000;

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

fn write_png(path: &Path, rgb: &[u8]) {
    std::fs::create_dir_all(path.parent().expect("parent")).expect("create dir");
    let file = std::fs::File::create(path).expect("create png");
    let mut encoder = png::Encoder::new(file, W as u32, H as u32);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("png header");
    writer.write_image_data(rgb).expect("png data");
}

/// Applies one frame: tick, flush the dirty rectangles, clear them.
fn step(app: &mut scenes::Showcase, now: u64, frame: &mut [u8]) {
    let mut dirty = Dirty::new(utsuroui::SCREEN);
    app.tick(now, &mut dirty);
    if dirty.is_empty() {
        return;
    }
    let cfg = RenderCfg::new(utsuroui::SCREEN, BAND_H);
    let mut buffers = utsuroui::BandBuffers::<BAND_LEN>::new();
    let mut sink = MemorySink::new(frame, W);
    pollster::block_on(flush(
        app,
        &mut sink,
        &dirty,
        now,
        &mut buffers,
        &cfg,
    ))
    .unwrap();
}

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf();
    let out = root.join("target/showcase");

    for (index, name) in scenes::SCENE_NAMES.iter().enumerate() {
        let mut app = scenes::Showcase::new();
        let mut dirty = Dirty::new(utsuroui::SCREEN);
        for _ in 0..index {
            app.event(&Event::Next, 0, &mut dirty);
        }
        // The first frame is a full redraw.
        dirty.mark_full();
        let mut frame = vec![0u8; FRAME_LEN];
        {
            let cfg = RenderCfg::new(utsuroui::SCREEN, BAND_H);
            let mut buffers = utsuroui::BandBuffers::<BAND_LEN>::new();
            let mut sink = MemorySink::new(&mut frame, W);
            pollster::block_on(flush(
                &app,
                &mut sink,
                &dirty,
                STEP_US,
                &mut buffers,
                &cfg,
            ))
            .unwrap();
        }
        // Then two seconds of animation.
        let mut now = STEP_US;
        for _ in 0..125 {
            now += STEP_US;
            step(&mut app, now, &mut frame);
        }

        let path = out.join(format!("{}.png", name.to_lowercase()));
        write_png(&path, &frame_to_rgb(&frame));
        println!("wrote {}", path.display());
    }
}
