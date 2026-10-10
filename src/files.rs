//! Acting on files: moving to the Trash, deleting, and looking things up online.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Moves an item to the Trash through Finder, so Finder's "Put Back" still works.
/// The first time, macOS asks whether DiskMap may control Finder.
pub fn move_to_trash(path: &Path) -> Result<(), String> {
    // The path goes in as an argument rather than being pasted into the
    // script, so no file name can break (or inject into) the AppleScript.
    let output = Command::new("osascript")
        .args([
            "-e",
            "on run argv",
            "-e",
            "set target to POSIX file (item 1 of argv) as alias",
            "-e",
            "tell application \"Finder\" to delete target",
            "-e",
            "end run",
        ])
        .arg(path)
        .output()
        .map_err(|e| format!("Couldn't run osascript: {e}"))?;
    if output.status.success() {
        return Ok(());
    }
    let error = String::from_utf8_lossy(&output.stderr);
    if error.contains("-1743") {
        Err("macOS didn't let DiskMap ask Finder to move it. Allow it in System Settings › \
             Privacy & Security › Automation, then try again."
            .into())
    } else {
        Err(format!("Finder couldn't move it to the Trash: {}", error.trim()))
    }
}

/// Deletes an item for good, skipping the Trash.
pub fn delete_permanently(path: &Path) -> Result<(), String> {
    // symlink_metadata looks at a link itself, so deleting a link never deletes what it points to.
    let meta = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    let result = if meta.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    };
    result.map_err(|e| format!("Couldn't delete it: {e}"))
}

/// Why DiskMap won't delete this item, if it won't.
pub fn protected(path: &Path, scan_root: &Path) -> Option<&'static str> {
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    if path == scan_root {
        return Some("This is the folder you scanned.");
    }
    if home.starts_with(path) {
        return Some("This is, or holds, your home folder.");
    }
    let system = [
        "/System",
        "/usr",
        "/bin",
        "/sbin",
        "/etc",
        "/var",
        "/private",
        "/Library",
        "/Applications",
        "/opt",
        "/Users",
        "/cores",
    ];
    if system.iter().any(|s| path == Path::new(s))
        || ["/System", "/bin", "/sbin", "/usr/bin", "/usr/lib", "/usr/sbin"]
            .iter()
            .any(|s| path.starts_with(s))
    {
        return Some("macOS needs this to run.");
    }
    let standard = [
        "Library",
        "Desktop",
        "Documents",
        "Downloads",
        "Movies",
        "Music",
        "Pictures",
        "Public",
        "Applications",
    ];
    if standard.iter().any(|s| path == home.join(s))
        || ["Application Support", "Containers", "Group Containers", "Preferences", "Caches"]
            .iter()
            .any(|s| path == home.join("Library").join(s))
    {
        return Some("macOS expects this folder to exist. Delete what's inside it instead.");
    }
    None
}

/// Opens a Google search about the item, with its parent folder for context.
pub fn search_web(name: &str, parent: Option<&str>, is_dir: bool) {
    let kind = if is_dir { "folder" } else { "file" };
    let query = match parent {
        Some(parent) => format!("\"{name}\" {kind} in \"{parent}\" mac what is it safe to delete"),
        None => format!("\"{name}\" {kind} mac what is it safe to delete"),
    };
    open_url(&format!("https://www.google.com/search?q={}", encode(&query)));
}

/// Opens claude.ai with a question about the item already written.
pub fn ask_claude(display_path: &str, is_dir: bool, size: &str, files: u64) {
    let what = if is_dir {
        format!("a folder using {size} ({files} files)")
    } else {
        format!("a file using {size}")
    };
    let question = format!(
        "On my Mac, DiskMap found {what} at:\n\n{display_path}\n\n\
         What is it, which app or tool created it, and is it safe to delete? \
         If it is, what's the best way to clean it up?"
    );
    open_url(&format!("https://claude.ai/new?q={}", encode(&question)));
}

fn open_url(url: &str) {
    let _ = Command::new("open").arg(url).spawn();
}

/// Percent-encodes text for a URL query: letters, digits and -_.~ stay, the rest become %XX.
fn encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_queries() {
        assert_eq!(encode("a b/\"c\""), "a%20b%2F%22c%22");
        assert_eq!(encode("é"), "%C3%A9");
    }

    #[test]
    fn deletes_folders_and_links_but_not_link_targets() {
        let base = std::env::temp_dir().join(format!("diskmap-test-{}", std::process::id()));
        let keep = base.join("keep.txt");
        let doomed = base.join("doomed");
        fs::create_dir_all(doomed.join("inner")).unwrap();
        fs::write(&keep, "keep me").unwrap();
        fs::write(doomed.join("inner/file.txt"), "bye").unwrap();
        std::os::unix::fs::symlink(&keep, doomed.join("link")).unwrap();

        delete_permanently(&doomed).unwrap();
        assert!(!doomed.exists());
        assert!(keep.exists(), "deleting a symlink must not delete its target");
        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn guards_important_folders() {
        let home = PathBuf::from(std::env::var_os("HOME").unwrap());
        let root = Path::new("/");
        assert!(protected(root, root).is_some());
        assert!(protected(&home, root).is_some());
        assert!(protected(Path::new("/Users"), root).is_some());
        assert!(protected(Path::new("/System/Library/Fonts"), root).is_some());
        assert!(protected(&home.join("Documents"), root).is_some());
        assert!(protected(&home.join("Library/Caches"), root).is_some());
        assert!(protected(&home.join("Library/Caches/com.example"), root).is_none());
        assert!(protected(&home.join("Documents/old-project"), root).is_none());
    }
}
