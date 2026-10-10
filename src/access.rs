//! Full Disk Access: the Mac's version of "Run as administrator".
//!
//! macOS keeps some folders (Mail, Messages, Safari, parts of other apps'
//! data) private even from administrators, and sudo doesn't get around it.
//! Only the Full Disk Access switch in System Settings does, and an app
//! can't turn that on for itself, so the best we can do is check, explain,
//! and open the right Settings page.

use std::fs::File;
use std::path::PathBuf;
use std::process::Command;

/// Tries to open files that only Full Disk Access unlocks.
pub fn has_full_disk_access() -> bool {
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    let probes = [
        home.join("Library/Application Support/com.apple.TCC/TCC.db"),
        home.join("Library/Safari/Bookmarks.plist"),
        PathBuf::from("/Library/Application Support/com.apple.TCC/TCC.db"),
    ];
    probes.iter().any(|path| File::open(path).is_ok())
}

/// Opens System Settings at Privacy & Security → Full Disk Access.
pub fn open_settings() {
    let _ = Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles")
        .spawn();
}

/// The app macOS gives the permission to. Run as DiskMap.app, that's DiskMap;
/// started from a terminal, macOS asks the app that owns the terminal instead.
pub fn host_app() -> String {
    if let Ok(exe) = std::env::current_exe() {
        let bundle = exe.ancestors().find(|p| p.extension().is_some_and(|ext| ext == "app"));
        if let Some(name) = bundle.and_then(|p| p.file_stem()) {
            return name.to_string_lossy().into_owned();
        }
    }
    match std::env::var("TERM_PROGRAM").as_deref() {
        Ok("vscode") => "Visual Studio Code".into(),
        Ok("Apple_Terminal") => "Terminal".into(),
        Ok("iTerm.app") => "iTerm".into(),
        Ok("ghostty") => "Ghostty".into(),
        _ => "the app you started DiskMap from".into(),
    }
}

/// True when running as DiskMap.app rather than from a terminal.
pub fn is_bundled() -> bool {
    std::env::current_exe().is_ok_and(|exe| exe.to_string_lossy().contains(".app/Contents/MacOS/"))
}
