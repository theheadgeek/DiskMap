//! DiskMap: see what's taking up space on your Mac, WinDirStat-style.

mod access;
mod app;
mod brand;
mod files;
mod known;
#[cfg(target_os = "macos")]
mod macmenu;
mod scan;
mod theme;
mod treemap;

use eframe::egui;

fn main() -> eframe::Result {
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("DiskMap")
        .with_inner_size([1360.0, 860.0])
        .with_min_inner_size([900.0, 560.0]);
    // The Dock icon when running with `cargo run`. DiskMap.app gets it from AppIcon.icns instead.
    if let Ok(icon) = eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png")) {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native("DiskMap", options, Box::new(|cc| Ok(Box::new(app::DiskMapApp::new(cc)))))
}
