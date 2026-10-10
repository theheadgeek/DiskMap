//! Walks the file system and builds an in-memory tree of sizes.

use rayon::prelude::*;
use std::cmp::Reverse;
use std::collections::HashSet;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// One file or folder.
pub struct Node {
    pub name: String,
    /// Bytes actually used on disk (so iCloud files that aren't downloaded count as 0).
    pub size: u64,
    /// Number of files at or below this node (1 for a file).
    pub files: u64,
    pub is_dir: bool,
    /// Sorted largest first.
    pub children: Vec<Node>,
}

impl Node {
    /// Follows a list of child indexes down from this node.
    pub fn get(&self, path: &[usize]) -> &Node {
        path.iter().fold(self, |node, &i| &node.children[i])
    }

    /// Takes the node at `path` out of the tree (after it's been deleted from
    /// disk) and shrinks every folder above it to match.
    pub fn remove(&mut self, path: &[usize]) -> Node {
        let (&first, rest) = path.split_first().expect("the scanned folder itself can't be removed");
        let removed = if rest.is_empty() {
            self.children.remove(first)
        } else {
            self.children[first].remove(rest)
        };
        self.size -= removed.size;
        self.files -= removed.files;
        // A folder that shrank may now belong further down the list.
        self.children.sort_by_key(|c| Reverse(c.size));
        removed
    }

    /// The child index path to the node with these names, as far down as they still exist.
    pub fn find(&self, names: &[String]) -> Vec<usize> {
        let mut path = Vec::new();
        let mut node = self;
        for name in names {
            let Some(i) = node.children.iter().position(|c| &c.name == name) else {
                break;
            };
            path.push(i);
            node = &node.children[i];
        }
        path
    }

    /// The names along `path`, which (unlike indexes) stay valid when the tree changes.
    pub fn names(&self, path: &[usize]) -> Vec<String> {
        let mut node = self;
        path.iter()
            .map(|&i| {
                node = &node.children[i];
                node.name.clone()
            })
            .collect()
    }
}

/// Live counters the window reads while a scan runs on another thread.
/// Atomics can be updated from many threads at once without a lock.
#[derive(Default)]
pub struct Progress {
    pub files: AtomicU64,
    pub bytes: AtomicU64,
    pub errors: AtomicU64,
    pub cancel: AtomicBool,
}

/// Places that aren't real data on this disk: other volumes, device files,
/// network automounts (which can hang), and the second view of the data
/// volume that macOS mounts under /System/Volumes.
const SKIP: &[&str] = &["/System/Volumes", "/Volumes", "/dev", "/net", "/home"];

struct Walker<'a> {
    progress: &'a Progress,
    /// Hard-linked files seen so far, so each is only counted once.
    seen_links: Mutex<HashSet<(u64, u64)>>,
}

pub fn scan(root: &Path, progress: &Progress) -> Node {
    let walker = Walker {
        progress,
        seen_links: Mutex::new(HashSet::new()),
    };
    walker.dir(root, root.display().to_string())
}

impl Walker<'_> {
    fn dir(&self, path: &Path, name: String) -> Node {
        let mut children: Vec<Node> = match fs::read_dir(path) {
            Ok(entries) => {
                let entries: Vec<fs::DirEntry> = entries.filter_map(Result::ok).collect();
                // rayon's into_par_iter spreads the entries across every CPU core.
                entries.into_par_iter().filter_map(|entry| self.entry(entry)).collect()
            }
            Err(_) => {
                self.progress.errors.fetch_add(1, Ordering::Relaxed);
                Vec::new()
            }
        };
        children.sort_unstable_by_key(|c| Reverse(c.size));
        Node {
            name,
            size: children.iter().map(|c| c.size).sum(),
            files: children.iter().map(|c| c.files).sum(),
            is_dir: true,
            children,
        }
    }

    fn entry(&self, entry: fs::DirEntry) -> Option<Node> {
        if self.progress.cancel.load(Ordering::Relaxed) {
            return None;
        }
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        // DirEntry::metadata doesn't follow symlinks, so a link is counted as a link.
        let Ok(meta) = entry.metadata() else {
            self.progress.errors.fetch_add(1, Ordering::Relaxed);
            return None;
        };

        if meta.is_dir() {
            if SKIP.iter().any(|skip| path == Path::new(skip)) {
                return None;
            }
            return Some(self.dir(&path, name));
        }

        let mut size = meta.blocks() * 512;
        if meta.nlink() > 1 && !self.seen_links.lock().unwrap().insert((meta.dev(), meta.ino())) {
            size = 0;
        }
        self.progress.files.fetch_add(1, Ordering::Relaxed);
        self.progress.bytes.fetch_add(size, Ordering::Relaxed);
        Some(Node {
            name,
            size,
            files: 1,
            is_dir: false,
            children: Vec::new(),
        })
    }
}

/// The on-disk path of the node at `path` below the scanned folder.
pub fn full_path(root: &Path, tree: &Node, path: &[usize]) -> PathBuf {
    let mut full = root.to_path_buf();
    let mut node = tree;
    for &i in path {
        node = &node.children[i];
        full.push(&node.name);
    }
    full
}

/// "photo.JPG" -> "jpg". Names with no extension, and dotfiles like ".zshrc", give "".
pub fn extension(name: &str) -> String {
    match name.rfind('.') {
        Some(i) if i > 0 && i + 1 < name.len() => name[i + 1..].to_lowercase(),
        _ => String::new(),
    }
}

/// Bytes as Finder shows them (1 KB = 1000 bytes).
pub fn format_size(bytes: u64) -> String {
    if bytes < 1000 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64;
    let mut unit = "B";
    for next in ["KB", "MB", "GB", "TB", "PB"] {
        value /= 1000.0;
        unit = next;
        if value < 1000.0 {
            break;
        }
    }
    format!("{value:.1} {unit}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(name: &str, size: u64) -> Node {
        Node {
            name: name.into(),
            size,
            files: 1,
            is_dir: false,
            children: vec![],
        }
    }

    fn folder(name: &str, children: Vec<Node>) -> Node {
        let mut node = Node {
            name: name.into(),
            size: 0,
            files: 0,
            is_dir: true,
            children,
        };
        node.children.sort_by_key(|c| Reverse(c.size));
        node.size = node.children.iter().map(|c| c.size).sum();
        node.files = node.children.iter().map(|c| c.files).sum();
        node
    }

    #[test]
    fn removing_updates_sizes_and_order() {
        let mut tree = folder(
            "root",
            vec![
                folder("big", vec![file("a", 50), file("b", 40)]),
                folder("medium", vec![file("c", 60)]),
            ],
        );
        assert_eq!(tree.names(&[0]), ["big"]);

        // Removing "a" leaves "big" at 40, so it drops below "medium" (60).
        let removed = tree.remove(&[0, 0]);
        assert_eq!(removed.name, "a");
        assert_eq!(tree.size, 100);
        assert_eq!(tree.files, 2);
        assert_eq!(tree.names(&[0]), ["medium"]);
        assert_eq!(tree.find(&["big".into(), "b".into()]), [1, 0]);
    }

    #[test]
    fn extensions() {
        assert_eq!(extension("photo.JPG"), "jpg");
        assert_eq!(extension("archive.tar.gz"), "gz");
        assert_eq!(extension(".zshrc"), "");
        assert_eq!(extension("Makefile"), "");
    }

    #[test]
    fn sizes() {
        assert_eq!(format_size(999), "999 B");
        assert_eq!(format_size(1_500), "1.5 KB");
        assert_eq!(format_size(42_100_000_000), "42.1 GB");
    }
}
