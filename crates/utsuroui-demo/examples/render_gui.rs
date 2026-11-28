//! Renders the `gui-showcase` screens to PNG on the host, for review and for the
//! documentation screenshots.
//!
//! ```sh
//! cargo run -p utsuroui-demo --example render_gui
//! ```

#[path = "../../../examples/gui-showcase/src/style.rs"]
mod style;

#[path = "../../../examples/gui-showcase/src/app.rs"]
mod app;

use std::path::Path;

use utsuroui::band::MemorySink;
use utsuroui::color::{Rgb565, raw_u16};
use utsuroui::dirty::Dirty;
use utsuroui::event::Event;
use utsuroui::flush::{RenderCfg, flush};

const BAND_H: u16 = 16;
const BAND_LEN: usize = style::W as usize * BAND_H as usize * 2;
const FRAME_LEN: usize = style::W as usize * style::H as usize * 2;

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
    let mut out = Vec::with_capacity(style::W as usize * style::H as usize * 3);
    for i in (0..frame.len()).step_by(2) {
        let raw = u16::from_be_bytes([frame[i], frame[i + 1]]);
        out.extend_from_slice(&expand(utsuroui::color::from_raw_u16(raw)));
    }
    out
}

fn write_png(path: &Path, rgb: &[u8]) {
    std::fs::create_dir_all(path.parent().expect("parent")).expect("create dir");
    let file = std::fs::File::create(path).expect("create png");
    let mut encoder = png::Encoder::new(file, style::W as u32, style::H as u32);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("png header");
    writer.write_image_data(rgb).expect("png data");
}

fn render_full(app: &app::GuiApp, now: u64, frame: &mut [u8]) {
    let mut dirty = Dirty::new(utsuroui::SCREEN);
    dirty.mark_full();
    let cfg = RenderCfg::new(utsuroui::SCREEN, BAND_H);
    let mut buffers = utsuroui::BandBuffers::<BAND_LEN>::new();
    let mut sink = MemorySink::new(frame, style::W);
    pollster::block_on(flush(app, &mut sink, &dirty, now, &mut buffers, &cfg)).expect("flush");
}

fn main() {
    use utsuroui::app::App;

    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf();
    let out = root.join("target/gui");
    let docs = root.join("docs/images");

    // 1. Home, first card selected.
    let app = app::GuiApp::new();
    let mut frame = vec![0u8; FRAME_LEN];
    render_full(&app, 0, &mut frame);
    write_png(&out.join("home.png"), &frame_to_rgb(&frame));

    // 2. Home, third card selected after the bar slid.
    let mut app = app::GuiApp::new();
    let mut dirty = Dirty::new(utsuroui::SCREEN);
    app.event(&Event::Next, 0, &mut dirty);
    app.event(&Event::Next, 0, &mut dirty);
    app.tick(200_000, &mut Dirty::new(utsuroui::SCREEN));
    render_full(&app, 200_000, &mut frame);
    write_png(&out.join("home_selected.png"), &frame_to_rgb(&frame));

    // 3. Halfway through the slide to the controls screen.
    let mut app = app::GuiApp::new();
    let mut dirty = Dirty::new(utsuroui::SCREEN);
    app.event(&Event::Select, 0, &mut dirty);
    render_full(&app, 90_000, &mut frame);
    write_png(&out.join("slide_mid.png"), &frame_to_rgb(&frame));

    // 4. Controls, with the toggle flipped and the toast sliding up.
    let mut app = app::GuiApp::new();
    let mut dirty = Dirty::new(utsuroui::SCREEN);
    app.event(&Event::Select, 0, &mut dirty); // open controls
    app.tick(200_000, &mut Dirty::new(utsuroui::SCREEN));
    let mut dirty = Dirty::new(utsuroui::SCREEN);
    app.event(&Event::Select, 200_000, &mut dirty); // flip the focused toggle
    app.tick(260_000, &mut Dirty::new(utsuroui::SCREEN));
    render_full(&app, 260_000, &mut frame);
    write_png(&out.join("controls.png"), &frame_to_rgb(&frame));
    write_png(&docs.join("gui_home.png"), &frame_to_rgb(&{
        let mut f = vec![0u8; FRAME_LEN];
        render_full(&app::GuiApp::new(), 0, &mut f);
        f
    }));

    println!("wrote {}", out.display());
}
