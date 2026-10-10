//! Folders and files DiskMap can explain without looking anything up:
//! the usual suspects when a Mac runs out of space.

use crate::scan::Node;

#[derive(Clone, Copy, PartialEq)]
pub enum Safety {
    /// Rebuilt or re-downloaded automatically when needed.
    Safe,
    /// Fine to clean up, but there's a better way than deleting it here, or a catch.
    Careful,
    /// Holds data you'd lose. Manage it from the app that owns it.
    Keep,
}

pub struct Known {
    pub title: &'static str,
    pub about: &'static str,
    pub safety: Safety,
}

struct Rule {
    matches: fn(name: &str, node: &Node, parent: Option<&Node>) -> bool,
    known: Known,
}

fn parent_is(parent: Option<&Node>, name: &str) -> bool {
    parent.is_some_and(|p| p.name == name)
}

fn has_child(node: &Node, name: &str) -> bool {
    node.children.iter().any(|c| c.name == name)
}

const fn rule(matches: fn(&str, &Node, Option<&Node>) -> bool, title: &'static str, about: &'static str, safety: Safety) -> Rule {
    Rule {
        matches,
        known: Known { title, about, safety },
    }
}

use Safety::*;

const RULES: &[Rule] = &[
    rule(
        |n, d, p| n == "DerivedData" && d.is_dir && parent_is(p, "Xcode"),
        "Xcode build files",
        "Compiled builds and indexes for every project you've opened in Xcode. Xcode recreates them; the next build is just slower.",
        Safe,
    ),
    rule(
        |n, d, p| n == "Archives" && d.is_dir && parent_is(p, "Xcode"),
        "Xcode archives",
        "Builds made with Product › Archive. You need them to read crash reports for versions you've shipped, so keep the recent ones.",
        Careful,
    ),
    rule(
        |n, d, _| n.ends_with("OS DeviceSupport") && d.is_dir,
        "Xcode device support",
        "Debug symbols copied from iPhones and other devices you've connected to Xcode. Copied again the next time you connect a device.",
        Safe,
    ),
    rule(
        |n, d, p| n == "CoreSimulator" && d.is_dir && parent_is(p, "Developer"),
        "iOS simulators",
        "Simulated devices and their apps and data. Remove old ones in Xcode › Settings › Platforms, or with `xcrun simctl delete unavailable`.",
        Careful,
    ),
    rule(
        |n, d, _| n == "node_modules" && d.is_dir,
        "JavaScript packages",
        "Packages installed for a JavaScript project. Running `npm install` in the project brings them back.",
        Safe,
    ),
    rule(
        |n, d, _| n == "target" && d.is_dir && (has_child(d, "CACHEDIR.TAG") || has_child(d, "debug") || has_child(d, "release")),
        "Rust build output",
        "Compiled files from `cargo build`. Rebuilt the next time you build the project (`cargo clean` does the same).",
        Safe,
    ),
    rule(
        |n, d, _| (n == "__pycache__" || n == ".pytest_cache" || n == ".mypy_cache") && d.is_dir,
        "Python cache",
        "Compiled Python files and tool caches, recreated automatically.",
        Safe,
    ),
    rule(
        |n, d, _| (n == ".venv" || n == "venv") && d.is_dir,
        "Python virtual environment",
        "Packages installed for one Python project. Recreate it from the project's requirements if you need it again.",
        Careful,
    ),
    rule(
        |n, d, _| n == ".git" && d.is_dir,
        "Git history",
        "Every saved version of a project. Deleting it erases the history unless it's pushed somewhere like GitHub.",
        Keep,
    ),
    rule(
        |n, d, _| n == ".Trash" && d.is_dir,
        "Your Trash",
        "Things you've moved to the Trash. Empty it from Finder to free the space.",
        Careful,
    ),
    rule(
        |n, d, p| n == "Caches" && d.is_dir && parent_is(p, "Library"),
        "App caches",
        "Temporary files apps keep to run faster. Apps rebuild them, though some may be slow the first time.",
        Safe,
    ),
    rule(
        |n, d, p| n == "Logs" && d.is_dir && parent_is(p, "Library"),
        "Log files",
        "Records apps and macOS write as they run. Only needed for troubleshooting.",
        Safe,
    ),
    rule(
        |n, d, p| n == "Backup" && d.is_dir && parent_is(p, "MobileSync"),
        "iPhone and iPad backups",
        "Backups made by Finder or iTunes. Delete old ones from Finder: select the device, then Manage Backups.",
        Careful,
    ),
    rule(
        |n, d, p| n == "Containers" && d.is_dir && parent_is(p, "Library"),
        "App data (sandboxed apps)",
        "Settings, documents and caches for App Store and other sandboxed apps. Remove the app instead of deleting this.",
        Keep,
    ),
    rule(
        |n, d, p| n == "Group Containers" && d.is_dir && parent_is(p, "Library"),
        "Shared app data",
        "Data shared between apps from the same developer, such as Office or iCloud apps.",
        Keep,
    ),
    rule(
        |n, d, p| n == "Application Support" && d.is_dir && parent_is(p, "Library"),
        "App data",
        "Apps' own files and settings. Look inside for folders belonging to apps you've removed; those are safe to delete.",
        Careful,
    ),
    rule(
        |n, d, p| n == "Mail" && d.is_dir && parent_is(p, "Library"),
        "Apple Mail",
        "Your downloaded messages and attachments. Manage them in Mail; deleting here can damage the mailbox.",
        Keep,
    ),
    rule(
        |n, d, p| n == "Messages" && d.is_dir && parent_is(p, "Library"),
        "Messages history",
        "Your conversations and attachments. Free space from System Settings › General › Storage › Messages.",
        Keep,
    ),
    rule(
        |n, _, _| n.ends_with(".photoslibrary"),
        "Photos library",
        "All your photos and edits. Manage it in the Photos app, or turn on Optimise Mac Storage in Photos settings.",
        Keep,
    ),
    rule(
        |n, d, _| n.ends_with(".app") && d.is_dir,
        "An application",
        "Drag apps to the Trash from Applications to remove them. Their data in Library stays behind.",
        Careful,
    ),
    rule(
        |n, _, _| n == "Docker.raw",
        "Docker disk image",
        "Docker Desktop's virtual disk, holding all its images and containers. Free space with Docker Desktop › Troubleshoot, or `docker system prune`.",
        Careful,
    ),
    rule(
        |n, d, p| n == "registry" && d.is_dir && parent_is(p, ".cargo"),
        "Rust package downloads",
        "Crates Cargo has downloaded. Downloaded again when a project needs them.",
        Safe,
    ),
    rule(
        |n, d, _| (n == ".npm" || n == ".yarn" || n == ".pnpm-store" || n == ".gradle") && d.is_dir,
        "Package manager cache",
        "Downloaded packages kept so installs are faster. Downloaded again when needed.",
        Safe,
    ),
    rule(
        |n, d, p| n == "Homebrew" && d.is_dir && parent_is(p, "Caches"),
        "Homebrew downloads",
        "Installers Homebrew has already used. `brew cleanup` clears them.",
        Safe,
    ),
    rule(
        |n, d, _| n == "vm" && d.is_dir,
        "Swap and sleep image",
        "macOS's overflow memory and the sleep image. macOS manages it; restarting usually shrinks it.",
        Keep,
    ),
    rule(
        |n, d, _| n == "Library" && d.is_dir,
        "Library",
        "Settings and data for macOS and your apps. Clean up the folders inside it, never the folder itself.",
        Keep,
    ),
];

/// Finds the deepest known item at or above `path`.
/// Returns how deep it is (0 = the scanned folder) and what's known about it.
pub fn lookup<'a>(tree: &'a Node, path: &[usize]) -> Option<(usize, &'static Known)> {
    let mut found = None;
    let mut parent: Option<&'a Node> = None;
    let mut node = tree;
    for depth in 0..=path.len() {
        if depth > 0 {
            parent = Some(node);
            node = &node.children[path[depth - 1]];
        }
        if let Some(rule) = RULES.iter().find(|r| (r.matches)(&node.name, node, parent)) {
            found = Some((depth, &rule.known));
        }
    }
    found
}
