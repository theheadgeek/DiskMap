//! DiskMap: see what's taking up space on your Mac, WinDirStat-style.

mod access;
mod app;
mod brand;
mod files;
mod known;
mod scan;
mod theme;
mod treemap;

use eframe::egui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("DiskMap")
            .with_inner_size([1360.0, 860.0])
            .with_min_inner_size([900.0, 560.0]),
        ..Default::default()
    };
    eframe::run_native("DiskMap", options, Box::new(|cc| Ok(Box::new(app::DiskMapApp::new(cc)))))
}
