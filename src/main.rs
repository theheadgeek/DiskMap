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
    // DiskMap.app's icon comes from its bundle, where macOS switches it to match the Icon & widget
    // style. Any image set here would replace that, and with none at all eframe uses its own default
    // icon, so the app passes an empty one. Under `cargo run` there's no bundle, so it sets the cube.
    if access::is_bundled() {
        viewport = viewport.with_icon(egui::IconData::default());
    } else if let Ok(icon) = eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png")) {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native("DiskMap", options, Box::new(|cc| Ok(Box::new(app::DiskMapApp::new(cc)))))
}
