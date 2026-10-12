//! The window: toolbar, folder tree, treemap and file-type list.

use crate::access;
use crate::brand;
use crate::files;
use crate::known::{self, Known, Safety};
use crate::scan::{self, Node, Progress};
use crate::theme;
use crate::treemap::{self, Tile};
use eframe::egui::{self, Align, Align2, Color32, FontId, Layout, Mesh, Rect, RichText, Sense, Shape, Stroke, StrokeKind, Ui, pos2, vec2};
use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant};

/// Where a node sits in the tree: the child index at each level, from the root down.
type NodePath = Vec<usize>;

/// A scan running on a background thread.
struct Scan {
    root: PathBuf,
    progress: Arc<Progress>,
    result: Receiver<Node>,
    started: Instant,
}

struct Finished {
    root: PathBuf,
    tree: Node,
    took: Duration,
    errors: u64,
}

struct FileType {
    ext: String,
    size: u64,
    files: u64,
    color: Color32,
}

/// The treemap for one zoom level and window size, kept until either changes.
struct MapCache {
    zoom: NodePath,
    rect: Rect,
    /// The highlighted file type, and whether other types are hidden.
    filter: Option<(String, bool)>,
    tiles: Vec<Tile>,
    mesh: Arc<Mesh>,
    /// Nothing to draw: an empty folder, or no files of the filtered type.
    empty: bool,
}

/// Something the user asked for this frame. Drawing code records these and
/// `apply` carries them out afterwards, so the UI never changes the data it's
/// in the middle of reading. (The borrow checker insists on that, and it's a
/// good habit anyway.)
enum Action {
    Scan(PathBuf),
    ChooseFolder,
    Cancel,
    /// `reveal` scrolls the folder list to the item (wanted when it was picked on the map).
    Select {
        path: NodePath,
        reveal: bool,
    },
    Zoom(NodePath),
    Toggle(NodePath),
    RevealInFinder(NodePath),
    CopyPath(NodePath),
    OpenAccessSettings,
    CheckAccess,
    ShowAccessHelp,
    ShowAbout(bool),
    /// Opens the confirmation for moving to the Trash or deleting for good.
    AskDelete {
        path: NodePath,
        permanent: bool,
    },
    ConfirmDelete,
    CancelDelete,
    LookUp {
        path: NodePath,
        ask_claude: bool,
    },
    DismissNotice,
    /// Highlights a file type on the map, or turns it off if it's already highlighted.
    ToggleType(String),
    ClearType,
    /// Hide every other file type, laying the map out by this type alone.
    SetTypeOnly(bool),
    SetAppearance(theme::Choice),
}

/// A delete waiting for the user to confirm it.
struct PendingDelete {
    path: NodePath,
    full: PathBuf,
    name: String,
    size: u64,
    files: u64,
    is_dir: bool,
    permanent: bool,
}

/// A confirmed delete running on a background thread (big folders take a while).
struct Deleting {
    pending: PendingDelete,
    result: Receiver<Result<(), String>>,
}

#[derive(Default)]
pub struct DiskMapApp {
    running: Option<Scan>,
    done: Option<Finished>,
    zoom: NodePath,
    selected: Option<NodePath>,
    expanded: HashSet<NodePath>,
    types: Vec<FileType>,
    type_colors: HashMap<String, Color32>,
    map: Option<MapCache>,
    scroll_tree_to_selected: bool,
    /// What was under the pointer when the map's right-click menu opened.
    menu_target: Option<NodePath>,
    actions: Vec<Action>,
    /// Whether macOS lets DiskMap see protected folders (see access.rs).
    full_disk_access: bool,
    /// The app macOS gives the permission to: DiskMap.app, or the terminal it runs in.
    host_app: String,
    show_access_help: bool,
    /// Set when "Check again" still finds access off, to explain that a restart is needed.
    checked_again: bool,
    confirm: Option<PendingDelete>,
    deleting: Option<Deleting>,
    /// A message for the user, such as why a delete failed.
    notice: Option<String>,
    /// The file type highlighted on the map (its extension), if any.
    type_filter: Option<String>,
    /// Lay the map out by the highlighted type alone, hiding the rest.
    type_only: bool,
    /// The folder picker, while it's open. It answers with the chosen folder, or None if cancelled.
    folder_picker: Option<Receiver<Option<PathBuf>>>,
    show_about: bool,
    /// Whether the menu bar's About item now opens our About window (see macmenu.rs).
    about_menu_ready: bool,
    appearance: theme::Choice,
    /// The palette last applied, so a change (including macOS switching to dark mode) is noticed.
    palette: Option<&'static theme::Palette>,
}

impl DiskMapApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut app = Self {
            full_disk_access: access::has_full_disk_access(),
            host_app: access::host_app(),
            appearance: theme::Choice::load(),
            ..Self::default()
        };
        app.follow_appearance(&cc.egui_ctx);
        // `diskmap ~/Downloads` starts scanning that folder straight away.
        if let Some(folder) = std::env::args().nth(1).filter(|arg| !arg.starts_with('-')) {
            app.start_scan(PathBuf::from(folder), &cc.egui_ctx);
        }
        app
    }
}

impl eframe::App for DiskMapApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.follow_appearance(ui.ctx());
        self.poll_scan(ui.ctx());
        self.poll_delete(ui.ctx());
        self.poll_folder_picker(ui.ctx());
        #[cfg(target_os = "macos")]
        {
            if !self.about_menu_ready {
                self.about_menu_ready = crate::macmenu::install(ui.ctx());
            }
            if crate::macmenu::about_requested() {
                self.show_about = true;
            }
        }

        egui::Panel::top("toolbar").show(ui, |ui| self.toolbar(ui));
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        if self.done.is_some() && self.running.is_none() {
            // The side panels share at most half the window, so the map always gets the rest.
            let width = ui.available_width();
            let (tree_max, types_max) = (width * 0.30, width * 0.20);
            egui::Panel::left("tree")
                .resizable(true)
                .default_size(380.0_f32.min(tree_max))
                .size_range(200.0_f32.min(tree_max)..=tree_max)
                .show(ui, |ui| self.tree_view(ui));
            egui::Panel::right("types")
                .resizable(true)
                .default_size(240.0_f32.min(types_max))
                .size_range(160.0_f32.min(types_max)..=types_max)
                .show(ui, |ui| self.type_list(ui));
        }
        egui::CentralPanel::no_frame().show(ui, |ui| self.central(ui));

        if self.show_access_help {
            let modal = egui::Modal::new(egui::Id::new("access-help")).show(ui.ctx(), |ui| {
                ui.set_max_width(520.0);
                access_panel(ui, self.full_disk_access, self.checked_again, &self.host_app, &mut self.actions);
                ui.add_space(12.0);
                ui.with_layout(Layout::right_to_left(Align::Min), |ui| ui.button("Close").clicked())
                    .inner
            });
            if modal.inner || modal.should_close() {
                self.show_access_help = false;
            }
        }

        self.delete_dialogs(ui.ctx());

        if self.show_about {
            let modal = egui::Modal::new(egui::Id::new("about")).show(ui.ctx(), |ui| {
                ui.set_width(360.0);
                about_panel(ui)
            });
            if modal.inner || modal.should_close() {
                self.actions.push(Action::ShowAbout(false));
            }
        }

        let dialog_open = self.confirm.is_some() || self.notice.is_some() || self.show_access_help || self.show_about;
        if self.type_filter.is_some() && !dialog_open && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.actions.push(Action::ClearType);
        }

        for action in std::mem::take(&mut self.actions) {
            self.apply(action, ui.ctx());
        }
    }
}

// ---- Scanning -------------------------------------------------------------

impl DiskMapApp {
    fn start_scan(&mut self, root: PathBuf, ctx: &egui::Context) {
        self.cancel_scan();
        let progress = Arc::new(Progress::default());
        let (sender, receiver) = mpsc::channel();

        // `move` hands these clones to the new thread, which then owns them.
        let (thread_progress, thread_root, ctx) = (progress.clone(), root.clone(), ctx.clone());
        std::thread::spawn(move || {
            let tree = scan::scan(&thread_root, &thread_progress);
            let _ = sender.send(tree); // only fails if the scan was cancelled
            ctx.request_repaint();
        });

        self.running = Some(Scan {
            root,
            progress,
            result: receiver,
            started: Instant::now(),
        });
    }

    /// Opens the folder picker as a sheet on the DiskMap window.
    ///
    /// The simpler blocking picker (`rfd::FileDialog`) pauses the window's
    /// event loop while it's open, and the windowing library aborts the app
    /// when macOS delivers mouse events during that pause. The async picker
    /// opens a sheet instead, and a background thread waits for the answer.
    fn choose_folder(&mut self, ctx: &egui::Context) {
        if self.folder_picker.is_some() {
            return; // already open
        }
        let (sender, receiver) = mpsc::channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let dialog = rfd::AsyncFileDialog::new().set_directory(home()).pick_folder();
            let picked = pollster::block_on(dialog).map(|folder| folder.path().to_path_buf());
            let _ = sender.send(picked);
            ctx.request_repaint();
        });
        self.folder_picker = Some(receiver);
    }

    fn poll_folder_picker(&mut self, ctx: &egui::Context) {
        let Some(picker) = &self.folder_picker else { return };
        match picker.try_recv() {
            Ok(picked) => {
                self.folder_picker = None;
                if let Some(folder) = picked {
                    self.start_scan(folder, ctx);
                }
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => self.folder_picker = None,
        }
    }

    fn cancel_scan(&mut self) {
        if let Some(scan) = self.running.take() {
            scan.progress.cancel.store(true, Ordering::Relaxed);
        }
    }

    fn poll_scan(&mut self, ctx: &egui::Context) {
        let Some(scan) = &self.running else { return };
        match scan.result.try_recv() {
            Ok(tree) => {
                let scan = self.running.take().unwrap();
                self.types = file_types(&tree);
                self.recolor_types();
                self.zoom.clear();
                self.selected = None;
                self.expanded = HashSet::from([NodePath::new()]);
                self.type_filter = None;
                self.done = Some(Finished {
                    root: scan.root,
                    tree,
                    took: scan.started.elapsed(),
                    errors: scan.progress.errors.load(Ordering::Relaxed),
                });
            }
            // Still going: redraw a few times a second so the counters move.
            Err(TryRecvError::Empty) => ctx.request_repaint_after(Duration::from_millis(100)),
            Err(TryRecvError::Disconnected) => self.running = None,
        }
    }

    fn apply(&mut self, action: Action, ctx: &egui::Context) {
        match action {
            Action::Scan(path) => self.start_scan(path, ctx),
            Action::ChooseFolder => self.choose_folder(ctx),
            Action::Cancel => self.cancel_scan(),
            Action::Select { path, reveal } => {
                // Open every folder above the selection in the tree.
                for depth in 0..path.len() {
                    self.expanded.insert(path[..depth].to_vec());
                }
                self.scroll_tree_to_selected = reveal;
                self.selected = Some(path);
            }
            Action::Zoom(path) => self.zoom = path,
            Action::Toggle(path) => {
                if !self.expanded.remove(&path) {
                    self.expanded.insert(path);
                }
            }
            Action::RevealInFinder(path) => {
                if let Some(full) = self.full_path(&path) {
                    let _ = std::process::Command::new("open").arg("-R").arg(full).spawn();
                }
            }
            Action::OpenAccessSettings => access::open_settings(),
            Action::CheckAccess => {
                self.full_disk_access = access::has_full_disk_access();
                self.checked_again = !self.full_disk_access;
            }
            Action::ShowAccessHelp => self.show_access_help = true,
            Action::ShowAbout(show) => self.show_about = show,
            Action::AskDelete { path, permanent } => {
                let Some(done) = &self.done else { return };
                let node = done.tree.get(&path);
                self.confirm = Some(PendingDelete {
                    full: scan::full_path(&done.root, &done.tree, &path),
                    name: node.name.clone(),
                    size: node.size,
                    files: node.files,
                    is_dir: node.is_dir,
                    permanent,
                    path,
                });
            }
            Action::CancelDelete => self.confirm = None,
            Action::ConfirmDelete => {
                if let Some(pending) = self.confirm.take() {
                    self.start_delete(pending, ctx);
                }
            }
            Action::LookUp { path, ask_claude } => {
                let Some(done) = &self.done else { return };
                let node = done.tree.get(&path);
                if ask_claude {
                    let full = scan::full_path(&done.root, &done.tree, &path);
                    files::ask_claude(&display_root(&full), node.is_dir, &scan::format_size(node.size), node.files);
                } else {
                    let parent = path.split_last().map(|(_, parent)| done.tree.get(parent).name.as_str());
                    files::search_web(&node.name, parent.filter(|p| !p.contains('/')), node.is_dir);
                }
            }
            Action::DismissNotice => self.notice = None,
            Action::ToggleType(ext) => {
                self.type_filter = if self.type_filter.as_ref() == Some(&ext) { None } else { Some(ext) };
            }
            Action::ClearType => self.type_filter = None,
            Action::SetTypeOnly(only) => self.type_only = only,
            Action::SetAppearance(choice) => {
                self.appearance = choice;
                choice.save();
                self.follow_appearance(ctx);
            }
            Action::CopyPath(path) => {
                if let Some(full) = self.full_path(&path) {
                    ctx.copy_text(full.display().to_string());
                }
            }
        }
    }

    /// Applies the chosen theme's palette when it changes: when the user picks
    /// another theme, or macOS switches between light and dark under "Match system".
    fn follow_appearance(&mut self, ctx: &egui::Context) {
        let palette = self.appearance.palette(ctx.system_theme());
        if self.palette.is_none_or(|applied| !std::ptr::eq(applied, palette)) {
            self.palette = Some(palette);
            theme::apply(ctx, palette);
            self.recolor_types();
        }
    }

    /// Colours the file types from the palette, biggest first, and drops the map
    /// drawn with the old colours.
    fn recolor_types(&mut self) {
        let palette = theme::palette();
        for (i, file_type) in self.types.iter_mut().enumerate() {
            file_type.color = palette.type_colors.get(i).copied().unwrap_or(palette.type_other);
        }
        self.type_colors = self.types.iter().map(|t| (t.ext.clone(), t.color)).collect();
        self.map = None;
    }

    fn full_path(&self, path: &[usize]) -> Option<PathBuf> {
        self.done.as_ref().map(|done| scan::full_path(&done.root, &done.tree, path))
    }
}

// ---- Moving to the Trash and deleting --------------------------------------

impl DiskMapApp {
    fn start_delete(&mut self, pending: PendingDelete, ctx: &egui::Context) {
        let (sender, receiver) = mpsc::channel();
        let (full, permanent, ctx) = (pending.full.clone(), pending.permanent, ctx.clone());
        std::thread::spawn(move || {
            let result = if permanent {
                files::delete_permanently(&full)
            } else {
                files::move_to_trash(&full)
            };
            let _ = sender.send(result);
            ctx.request_repaint();
        });
        self.deleting = Some(Deleting { pending, result: receiver });
    }

    fn poll_delete(&mut self, ctx: &egui::Context) {
        let Some(deleting) = &self.deleting else { return };
        let result = match deleting.result.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return ctx.request_repaint_after(Duration::from_millis(100)),
            Err(TryRecvError::Disconnected) => Err("The delete stopped unexpectedly.".into()),
        };
        let pending = self.deleting.take().unwrap().pending;
        let gone = pending.full.symlink_metadata().is_err();
        if gone {
            self.remove_from_tree(&pending.path);
        }
        if let Err(error) = result {
            self.notice = Some(if gone {
                error
            } else {
                format!("{error}\n\nSome files inside may already be gone. Rescan to see the current sizes.")
            });
        }
    }

    /// Updates the tree after a delete without rescanning. Indexes shift when
    /// a folder shrinks and moves down its list, so the zoom, open folders and
    /// selection are remembered by name and found again afterwards.
    fn remove_from_tree(&mut self, path: &NodePath) {
        let Some(done) = &mut self.done else { return };
        let tree = &mut done.tree;
        let zoom = tree.names(&self.zoom);
        let expanded: Vec<Vec<String>> = self.expanded.iter().map(|p| tree.names(p)).collect();
        let parent = tree.names(&path[..path.len() - 1]);

        tree.remove(path);

        self.zoom = tree.find(&zoom);
        self.expanded = expanded
            .iter()
            .filter_map(|names| Some(tree.find(names)).filter(|found| found.len() == names.len()))
            .collect();
        self.selected = Some(tree.find(&parent));
        self.types = file_types(tree);
        self.recolor_types();
        if !self.types.iter().any(|t| Some(&t.ext) == self.type_filter.as_ref()) {
            self.type_filter = None;
        }
    }

    fn delete_dialogs(&mut self, ctx: &egui::Context) {
        if let Some(pending) = &self.confirm {
            let modal = egui::Modal::new(egui::Id::new("confirm-delete")).show(ctx, |ui| {
                ui.set_max_width(480.0);
                confirm_panel(ui, pending, &mut self.actions);
            });
            if modal.should_close() {
                self.actions.push(Action::CancelDelete);
            }
        }
        if let Some(deleting) = &self.deleting {
            egui::Modal::new(egui::Id::new("deleting")).show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new().color(theme::palette().tone));
                    let verb = if deleting.pending.permanent {
                        "Deleting"
                    } else {
                        "Moving to the Trash:"
                    };
                    ui.label(format!("{verb} {}", deleting.pending.name));
                });
            });
        }
        if let Some(notice) = &self.notice {
            let modal = egui::Modal::new(egui::Id::new("notice")).show(ctx, |ui| {
                ui.set_max_width(440.0);
                ui.label(notice.as_str());
                ui.add_space(12.0);
                ui.with_layout(Layout::right_to_left(Align::Min), |ui| ui.button("OK").clicked())
                    .inner
            });
            if modal.inner || modal.should_close() {
                self.actions.push(Action::DismissNotice);
            }
        }
    }
}

fn confirm_panel(ui: &mut Ui, pending: &PendingDelete, actions: &mut Vec<Action>) {
    let kind = if pending.is_dir { "folder" } else { "file" };
    ui.label(group_label(if pending.permanent { "Delete permanently" } else { "Move to Trash" }));
    ui.add_space(4.0);
    ui.label(RichText::new(&pending.name).font(theme::display_font(22.0)));
    ui.label(RichText::new(display_root(&pending.full)).color(theme::palette().ink_soft));
    ui.add_space(6.0);
    let mut amount = scan::format_size(pending.size);
    if pending.is_dir {
        amount += &format!(" · {} files", group_digits(pending.files));
    }
    ui.label(RichText::new(amount).monospace());
    ui.add_space(10.0);
    if pending.permanent {
        ui.label(
            RichText::new(format!(
                "This {kind} will skip the Trash and be gone for good. This can't be undone."
            ))
            .color(theme::palette().bad),
        );
    } else {
        ui.label(format!("You can get this {kind} back from the Trash until you empty it."));
    }
    ui.add_space(16.0);
    ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
        let button = if pending.permanent {
            egui::Button::new(RichText::new("Delete permanently").color(theme::palette().bad_ink)).fill(theme::palette().bad)
        } else {
            egui::Button::new(RichText::new("Move to Trash").color(Color32::WHITE)).fill(theme::palette().accent)
        };
        if ui.add(button.stroke(Stroke::NONE)).clicked() {
            actions.push(Action::ConfirmDelete);
        }
        if ui.button("Cancel").clicked() {
            actions.push(Action::CancelDelete);
        }
    });
}

// ---- Toolbar and status bar ---------------------------------------------

impl DiskMapApp {
    fn toolbar(&mut self, ui: &mut Ui) {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("DiskMap").font(theme::display_font(22.0)).color(theme::palette().ink));
            ui.add_space(8.0);
            let idle = self.running.is_none();
            ui.add_enabled_ui(idle, |ui| {
                if ui.button("Choose folder…").clicked() {
                    self.actions.push(Action::ChooseFolder);
                }
                if ui.button("Home folder").clicked() {
                    self.actions.push(Action::Scan(home()));
                }
                if ui.button("Whole disk").clicked() {
                    self.actions.push(Action::Scan(PathBuf::from("/")));
                }
                if let Some(done) = &self.done
                    && ui.button("Rescan").on_hover_text(done.root.display().to_string()).clicked()
                {
                    self.actions.push(Action::Scan(done.root.clone()));
                }
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("About").clicked() {
                    self.actions.push(Action::ShowAbout(true));
                }
                ui.menu_button("Appearance", |ui| {
                    for choice in theme::Choice::ALL {
                        if ui.radio(self.appearance == choice, choice.label()).clicked() {
                            self.actions.push(Action::SetAppearance(choice));
                            ui.close();
                        }
                    }
                });
                if !self.full_disk_access {
                    let warning = egui::Button::new(RichText::new("Full Disk Access is off").color(theme::palette().bad));
                    if ui
                        .add(warning)
                        .on_hover_text("Some folders are hidden from DiskMap. Click for how to fix it.")
                        .clicked()
                    {
                        self.actions.push(Action::ShowAccessHelp);
                    }
                }
            });
        });

        // Breadcrumbs for the treemap's zoom level.
        if let (Some(done), None) = (&self.done, &self.running) {
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                let can_go_up = !self.zoom.is_empty();
                if ui.add_enabled(can_go_up, egui::Button::new("Up one level")).clicked() {
                    self.actions.push(Action::Zoom(self.zoom[..self.zoom.len() - 1].to_vec()));
                }
                let mut node = &done.tree;
                if ui.link(display_root(&done.root)).clicked() {
                    self.actions.push(Action::Zoom(NodePath::new()));
                }
                for depth in 0..self.zoom.len() {
                    node = &node.children[self.zoom[depth]];
                    ui.label(RichText::new("/").color(theme::palette().ink_soft));
                    if ui.link(&node.name).clicked() {
                        self.actions.push(Action::Zoom(self.zoom[..=depth].to_vec()));
                    }
                }
            });
        }
        ui.add_space(4.0);
    }

    fn status_bar(&mut self, ui: &mut Ui) {
        ui.add_space(3.0);
        ui.horizontal(|ui| {
            let Some(done) = &self.done else {
                ui.label(RichText::new("Pick a folder to scan.").color(theme::palette().ink_soft));
                return;
            };
            if let Some(path) = &self.selected {
                let node = done.tree.get(path);
                let share = percent(node.size, done.tree.size);
                ui.label(RichText::new(scan::format_size(node.size)).monospace());
                ui.label(RichText::new(format!("{share:.1}%")).monospace().color(theme::palette().ink_soft));
                if node.is_dir {
                    ui.label(RichText::new(format!("{} files", group_digits(node.files))).color(theme::palette().ink_soft));
                }
                ui.add(egui::Label::new(scan::full_path(&done.root, &done.tree, path).display().to_string()).truncate());
            } else {
                ui.label(format!(
                    "{} in {} files · scanned in {:.1}s",
                    scan::format_size(done.tree.size),
                    group_digits(done.tree.files),
                    done.took.as_secs_f32()
                ));
            }
            if done.errors > 0 {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("{} items couldn't be read", group_digits(done.errors))).color(theme::palette().ink_soft),
                    )
                    .on_hover_text(
                        "macOS protects some folders. To include them, turn on Full Disk Access \
                             (the button at the top right explains how).",
                    );
                });
            }
        });
        ui.add_space(3.0);
    }
}

// ---- Folder tree ----------------------------------------------------------

struct Row {
    path: NodePath,
    depth: usize,
}

/// Lists every row the tree currently shows: each node whose parent is expanded.
fn visible_rows(node: &Node, path: &mut NodePath, expanded: &HashSet<NodePath>, rows: &mut Vec<Row>) {
    rows.push(Row {
        path: path.clone(),
        depth: path.len(),
    });
    if node.is_dir && expanded.contains(path) {
        for (i, child) in node.children.iter().enumerate() {
            path.push(i);
            visible_rows(child, path, expanded, rows);
            path.pop();
        }
    }
}

impl DiskMapApp {
    fn tree_view(&mut self, ui: &mut Ui) {
        let Some(done) = &self.done else { return };
        ui.add_space(8.0);
        ui.label(group_label("Folders"));
        ui.add_space(4.0);

        let mut rows = Vec::new();
        visible_rows(&done.tree, &mut NodePath::new(), &self.expanded, &mut rows);

        let row_height = ui.spacing().interact_size.y;
        let mut area = egui::ScrollArea::vertical().auto_shrink(false);
        if std::mem::take(&mut self.scroll_tree_to_selected)
            && let Some(index) = rows.iter().position(|row| Some(&row.path) == self.selected.as_ref())
        {
            let step = row_height + ui.spacing().item_spacing.y;
            area = area.vertical_scroll_offset((index as f32 * step - 160.0).max(0.0));
        }
        // show_rows only builds the rows that are on screen, so huge folders stay fast.
        area.show_rows(ui, row_height, rows.len(), |ui, range| {
            for row in &rows[range] {
                tree_row(ui, done, row, &self.selected, &self.expanded, &mut self.actions);
            }
        });
    }
}

fn tree_row(ui: &mut Ui, done: &Finished, row: &Row, selected: &Option<NodePath>, expanded: &HashSet<NodePath>, actions: &mut Vec<Action>) {
    let node = done.tree.get(&row.path);
    let parent_size = match row.path.split_last() {
        Some((_, parent)) => done.tree.get(parent).size,
        None => node.size,
    };
    let height = ui.spacing().interact_size.y;

    ui.horizontal(|ui| {
        ui.add_space(row.depth as f32 * 14.0);

        // Disclosure triangle, drawn by hand so every row lines up.
        let (rect, toggle) = ui.allocate_exact_size(vec2(14.0, height), Sense::click());
        if node.is_dir && !node.children.is_empty() {
            let open = expanded.contains(&row.path);
            let color = if toggle.hovered() {
                theme::palette().ink
            } else {
                theme::palette().ink_soft
            };
            let c = rect.center();
            let points = if open {
                vec![pos2(c.x - 4.0, c.y - 2.0), pos2(c.x + 4.0, c.y - 2.0), pos2(c.x, c.y + 3.0)]
            } else {
                vec![pos2(c.x - 2.0, c.y - 4.0), pos2(c.x + 3.0, c.y), pos2(c.x - 2.0, c.y + 4.0)]
            };
            ui.painter().add(Shape::convex_polygon(points, color, Stroke::NONE));
            if toggle.clicked() {
                actions.push(Action::Toggle(row.path.clone()));
            }
        }

        // Share of the parent folder, as a small bar.
        let (bar, _) = ui.allocate_exact_size(vec2(40.0, height), Sense::hover());
        let track = Rect::from_center_size(bar.center(), vec2(bar.width(), 6.0));
        ui.painter().rect_filled(track, 3, theme::palette().surface_2);
        let fraction = if parent_size > 0 {
            node.size as f32 / parent_size as f32
        } else {
            0.0
        };
        let fill = Rect::from_min_size(track.min, vec2(track.width() * fraction, track.height()));
        ui.painter().rect_filled(fill, 3, theme::palette().tone);

        ui.allocate_ui_with_layout(vec2(74.0, height), Layout::right_to_left(Align::Center), |ui| {
            ui.label(
                RichText::new(scan::format_size(node.size))
                    .monospace()
                    .color(theme::palette().ink_soft),
            );
        });

        let name = if row.path.is_empty() {
            display_root(&done.root)
        } else {
            node.name.clone()
        };
        let is_selected = selected.as_ref() == Some(&row.path);
        // Long names end in "…" rather than widening the panel; hovering shows the whole name.
        let label = ui
            .add(egui::Button::selectable(is_selected, name.as_str()).truncate())
            .on_hover_text(&name);
        if label.clicked() {
            actions.push(Action::Select {
                path: row.path.clone(),
                reveal: false,
            });
        }
        if label.double_clicked() && node.is_dir {
            actions.push(Action::Zoom(row.path.clone()));
        }
        label.context_menu(|ui| item_menu(ui, done, &row.path, actions));
    });
}

/// The right-click menu shared by the tree and the map.
fn item_menu(ui: &mut Ui, done: &Finished, path: &NodePath, actions: &mut Vec<Action>) {
    let node = done.tree.get(path);
    ui.set_max_width(320.0);

    if let Some((depth, known)) = known::lookup(&done.tree, path) {
        let inside = (depth < path.len()).then(|| done.tree.get(&path[..depth]).name.as_str());
        about(ui, known, inside);
        ui.separator();
    }

    if ui.button("Reveal in Finder").clicked() {
        actions.push(Action::RevealInFinder(path.clone()));
        ui.close();
    }
    if ui.button("Copy path").clicked() {
        actions.push(Action::CopyPath(path.clone()));
        ui.close();
    }
    let folder = if node.is_dir {
        path.clone()
    } else {
        path[..path.len().saturating_sub(1)].to_vec()
    };
    if ui.button("Show this folder on the map").clicked() {
        actions.push(Action::Zoom(folder));
        ui.close();
    }

    ui.separator();
    ui.menu_button("What is this?", |ui| {
        if ui
            .button("Ask Claude")
            .on_hover_text("Opens claude.ai with a question about this item")
            .clicked()
        {
            actions.push(Action::LookUp {
                path: path.clone(),
                ask_claude: true,
            });
            ui.close();
        }
        if ui
            .button("Search Google")
            .on_hover_text("Searches for its name and the folder it's in")
            .clicked()
        {
            actions.push(Action::LookUp {
                path: path.clone(),
                ask_claude: false,
            });
            ui.close();
        }
    });

    ui.separator();
    let blocked = files::protected(&scan::full_path(&done.root, &done.tree, path), &done.root);
    if ui.add_enabled(blocked.is_none(), egui::Button::new("Move to Trash…")).clicked() {
        actions.push(Action::AskDelete {
            path: path.clone(),
            permanent: false,
        });
        ui.close();
    }
    let delete = egui::Button::new(RichText::new("Delete permanently…").color(theme::palette().bad));
    if ui.add_enabled(blocked.is_none(), delete).clicked() {
        actions.push(Action::AskDelete {
            path: path.clone(),
            permanent: true,
        });
        ui.close();
    }
    if let Some(reason) = blocked {
        ui.label(RichText::new(reason).size(12.5).color(theme::palette().ink_soft));
    }
}

/// What DiskMap knows about an item, with a coloured dot and a word for how safe it is to delete.
fn about(ui: &mut Ui, known: &Known, inside: Option<&str>) {
    if let Some(parent) = inside {
        ui.label(
            RichText::new(format!("Inside {parent}"))
                .size(12.5)
                .color(theme::palette().ink_soft),
        );
    }
    ui.label(RichText::new(known.title).strong());
    ui.label(RichText::new(known.about).size(12.5).color(theme::palette().ink_soft));
    let (color, verdict) = match known.safety {
        Safety::Safe => (theme::palette().ok, "Usually safe to delete"),
        Safety::Careful => (theme::palette().caution, "Clean up with care"),
        Safety::Keep => (theme::palette().bad, "Don't delete"),
    };
    ui.horizontal(|ui| {
        let (dot, _) = ui.allocate_exact_size(vec2(8.0, 8.0), Sense::hover());
        ui.painter().circle_filled(dot.center(), 4.0, color);
        ui.label(RichText::new(verdict).size(12.5).color(color));
    });
}

// ---- File types -----------------------------------------------------------

fn file_types(tree: &Node) -> Vec<FileType> {
    fn walk(node: &Node, totals: &mut HashMap<String, (u64, u64)>) {
        if node.is_dir {
            node.children.iter().for_each(|child| walk(child, totals));
        } else {
            let entry = totals.entry(scan::extension(&node.name)).or_default();
            entry.0 += node.size;
            entry.1 += 1;
        }
    }
    let mut totals = HashMap::new();
    walk(tree, &mut totals);

    let mut types: Vec<FileType> = totals
        .into_iter()
        .map(|(ext, (size, files))| FileType {
            ext,
            size,
            files,
            color: Color32::TRANSPARENT, // set by recolor_types
        })
        .collect();
    types.sort_unstable_by_key(|t| Reverse(t.size));
    types
}

impl DiskMapApp {
    fn type_list(&mut self, ui: &mut Ui) {
        let Some(done) = &self.done else { return };
        ui.add_space(8.0);
        ui.label(group_label("File types"));
        ui.add_space(4.0);
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            for file_type in self.types.iter().take(200) {
                let selected = self.type_filter.as_ref() == Some(&file_type.ext);
                let (fill, text, soft) = if selected {
                    (
                        theme::palette().selection,
                        theme::palette().on_selection,
                        theme::palette().on_selection,
                    )
                } else {
                    (Color32::TRANSPARENT, theme::palette().ink, theme::palette().ink_soft)
                };
                let row = egui::Frame::new()
                    .fill(fill)
                    .corner_radius(6)
                    .inner_margin(egui::Margin::symmetric(6, 2))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let (swatch, _) = ui.allocate_exact_size(vec2(12.0, 12.0), Sense::hover());
                            ui.painter().rect_filled(swatch, 3, file_type.color);
                            // Size first (from the right), so a long type name is what gets shortened.
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(RichText::new(scan::format_size(file_type.size)).monospace().color(soft));
                                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                                    ui.add(egui::Label::new(RichText::new(type_name(&file_type.ext)).color(text)).truncate());
                                });
                            });
                        });
                    })
                    .response;
                // Make the whole row one click target.
                let row = ui.interact(row.rect, ui.id().with(("file-type", &file_type.ext)), Sense::click());
                if row.hovered() && !selected {
                    ui.painter()
                        .rect_stroke(row.rect, 6, Stroke::new(1.0, theme::palette().line), StrokeKind::Inside);
                }
                let hint = if selected {
                    "Click to stop highlighting"
                } else {
                    "Click to highlight on the map"
                };
                let row = row.on_hover_text(format!(
                    "{} files · {:.1}% of the scan\n{hint}",
                    group_digits(file_type.files),
                    percent(file_type.size, done.tree.size)
                ));
                if row.clicked() {
                    self.actions.push(Action::ToggleType(file_type.ext.clone()));
                }
            }
            if self.types.len() > 200 {
                ui.label(RichText::new(format!("and {} more", self.types.len() - 200)).color(theme::palette().ink_soft));
            }
        });
    }
}

// ---- Treemap and the screens around it ----------------------------------

impl DiskMapApp {
    fn central(&mut self, ui: &mut Ui) {
        let rect = ui.available_rect_before_wrap();
        ui.painter().rect_filled(rect, 0, theme::palette().paper);

        if let Some(scan) = &self.running {
            self.scanning_screen(ui, rect, scan.progress.clone(), scan.root.clone(), scan.started);
        } else if self.done.is_some() {
            self.treemap(ui, rect);
        } else {
            self.welcome_screen(ui, rect);
        }
    }

    fn welcome_screen(&mut self, ui: &mut Ui, rect: Rect) {
        let mut column = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(rect.shrink(40.0))
                .layout(Layout::top_down(Align::Center)),
        );
        let ui = &mut column;
        ui.add_space((rect.height() / 2.0 - 300.0).max(24.0));
        ui.label(
            RichText::new("DISKMAP")
                .size(12.0)
                .strong()
                .extra_letter_spacing(2.0)
                .color(theme::palette().ink_soft),
        );
        ui.add_space(6.0);
        ui.label(
            RichText::new("See what's using your disk")
                .font(theme::display_font(40.0))
                .color(theme::palette().ink),
        );
        ui.add_space(8.0);
        ui.label(
            RichText::new("Scan a folder and every file becomes a rectangle sized by the space it takes. Click one to find it, double-click to zoom in.")
                .size(15.5)
                .color(theme::palette().ink_soft),
        );
        ui.add_space(24.0);
        ui.horizontal(|ui| {
            // Centre the three buttons as a group.
            ui.add_space((ui.available_width() - 410.0).max(0.0) / 2.0);
            let primary = egui::Button::new(RichText::new("Scan home folder").color(Color32::WHITE))
                .fill(theme::palette().accent)
                .stroke(Stroke::NONE);
            if ui.add(primary).clicked() {
                self.actions.push(Action::Scan(home()));
            }
            if ui.button("Scan whole disk").clicked() {
                self.actions.push(Action::Scan(PathBuf::from("/")));
            }
            if ui.button("Choose folder…").clicked() {
                self.actions.push(Action::ChooseFolder);
            }
        });
        ui.add_space(28.0);
        if self.full_disk_access {
            // Just a one-line confirmation, centred under the buttons.
            ui.allocate_ui_with_layout(vec2(390.0, 24.0), Layout::left_to_right(Align::Center), |ui| {
                access_panel(ui, true, false, &self.host_app, &mut self.actions);
            });
        } else {
            egui::Frame::new()
                .fill(theme::palette().surface)
                .stroke(Stroke::new(1.0, theme::palette().line))
                .corner_radius(16)
                .inner_margin(20)
                .show(ui, |ui| {
                    ui.set_max_width(560.0);
                    ui.with_layout(Layout::top_down(Align::Min), |ui| {
                        access_panel(ui, self.full_disk_access, self.checked_again, &self.host_app, &mut self.actions);
                    });
                });
        }
        ui.add_space(32.0);
        brand_footer(ui);
    }

    fn scanning_screen(&mut self, ui: &mut Ui, rect: Rect, progress: Arc<Progress>, root: PathBuf, started: Instant) {
        let mut column = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(rect.shrink(40.0))
                .layout(Layout::top_down(Align::Center)),
        );
        let ui = &mut column;
        ui.add_space((rect.height() / 2.0 - 90.0).max(0.0));
        ui.add(egui::Spinner::new().size(28.0).color(theme::palette().tone));
        ui.add_space(12.0);
        ui.label(RichText::new(format!("Scanning {}", display_root(&root))).font(theme::display_font(26.0)));
        ui.add_space(6.0);
        ui.label(
            RichText::new(format!(
                "{} files · {} · {:.0}s",
                group_digits(progress.files.load(Ordering::Relaxed)),
                scan::format_size(progress.bytes.load(Ordering::Relaxed)),
                started.elapsed().as_secs_f32()
            ))
            .monospace()
            .color(theme::palette().ink_soft),
        );
        ui.add_space(16.0);
        if ui.button("Cancel").clicked() {
            self.actions.push(Action::Cancel);
        }
    }

    fn treemap(&mut self, ui: &mut Ui, rect: Rect) {
        let mut rect = rect;
        if let Some(ext) = self.type_filter.clone() {
            let (bar, rest) = rect.split_top_bottom_at_y(rect.top() + 40.0);
            self.type_bar(ui, bar, &ext);
            rect = rest;
        }
        let Some(done) = &self.done else { return };
        let response = ui.allocate_rect(rect, Sense::click());
        let map_rect = rect.shrink(6.0);

        // Lay the map out again only when the zoom, window size or type filter changes.
        let filter = self.type_filter.clone().map(|ext| (ext, self.type_only));
        let stale = self
            .map
            .as_ref()
            .is_none_or(|m| m.zoom != self.zoom || m.rect != map_rect || m.filter != filter);
        if stale {
            let view = done.tree.get(&self.zoom);
            let only = filter.as_ref().filter(|(_, only)| *only).map(|(ext, _)| ext.clone());
            let weights = only.map(|ext| treemap::weigh(view, &move |node: &Node| scan::extension(&node.name) == ext));

            let type_colors = &self.type_colors;
            let type_color = |ext: &str| *type_colors.get(ext).unwrap_or(&theme::palette().type_other);
            let color = |node: &Node| match (&weights, &filter) {
                // With other types hidden, a folder too small to split holds only this type.
                (Some(_), Some((ext, _))) if node.is_dir => type_color(ext),
                _ if node.is_dir => theme::palette().folder,
                _ => type_color(&scan::extension(&node.name)),
            };
            let tiles = treemap::layout(view, weights.as_ref(), map_rect, &color);

            // Highlighting without hiding: fade every tile that isn't this type.
            let dimmed = |tile: &Tile| match &filter {
                Some((ext, false)) => {
                    let node = view.get(&tile.path);
                    node.is_dir || &scan::extension(&node.name) != ext
                }
                _ => false,
            };
            let mesh = Arc::new(cushion_mesh(&tiles, &dimmed));
            let empty = weights.as_ref().map_or(view.size, |w| w.size) == 0;
            self.map = Some(MapCache {
                zoom: self.zoom.clone(),
                rect: map_rect,
                filter: filter.clone(),
                tiles,
                mesh,
                empty,
            });
        }
        let map = self.map.as_ref().unwrap();
        let painter = ui.painter_at(rect);
        painter.add(Shape::mesh(map.mesh.clone()));

        if map.empty {
            let message = match &filter {
                Some((ext, true)) => format!("No {} files in this folder", type_name(ext)),
                _ => "This folder is empty".to_owned(),
            };
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                message,
                FontId::proportional(15.0),
                theme::palette().ink_soft,
            );
        }

        // Outline the selection: in accent, Inactive Node's colour for interaction, or in ink
        // where blue is both the accent and a file type.
        if let Some(rel) = self.selected.as_ref().and_then(|s| s.strip_prefix(self.zoom.as_slice())) {
            // The selected item itself, or the closest tile containing it when it's too small to draw.
            let tile = map.tiles.iter().filter(|t| rel.starts_with(&t.path)).max_by_key(|t| t.path.len());
            if let Some(tile) = tile {
                painter.rect_stroke(tile.rect, 0, Stroke::new(2.0, theme::palette().map_selection), StrokeKind::Inside);
            }
        }

        let hovered = response
            .hover_pos()
            .and_then(|pos| map.tiles.iter().find(|t| t.is_leaf && t.rect.contains(pos)));
        let hovered_path = hovered.map(|t| [self.zoom.as_slice(), &t.path].concat());

        let mut response = response;
        if let (Some(tile), Some(path)) = (hovered, &hovered_path) {
            painter.rect_stroke(tile.rect, 0, Stroke::new(1.0, theme::palette().ink), StrokeKind::Inside);
            let node = done.tree.get(path);
            let root = &done.root;
            response = response.on_hover_ui_at_pointer(|ui| {
                ui.label(RichText::new(&node.name).strong());
                ui.label(RichText::new(scan::full_path(root, &done.tree, path).display().to_string()).color(theme::palette().ink_soft));
                let mut line = scan::format_size(node.size);
                if node.is_dir {
                    line += &format!(" · {} files", group_digits(node.files));
                }
                ui.label(RichText::new(line).monospace());
                if let Some((depth, known)) = known::lookup(&done.tree, path) {
                    ui.separator();
                    let inside = (depth < path.len()).then(|| done.tree.get(&path[..depth]).name.as_str());
                    about(ui, known, inside);
                }
            });
        }

        if let Some(path) = &hovered_path {
            if response.clicked() {
                self.actions.push(Action::Select {
                    path: path.clone(),
                    reveal: true,
                });
            }
            // Double-click zooms in one level, into the folder under the pointer.
            if response.double_clicked() && path.len() > self.zoom.len() {
                let next = path[..=self.zoom.len()].to_vec();
                if done.tree.get(&next).is_dir {
                    self.actions.push(Action::Zoom(next));
                }
            }
            if response.secondary_clicked() {
                self.menu_target = Some(path.clone());
            }
        }

        let target = self.menu_target.clone();
        let zoom = &self.zoom;
        let actions = &mut self.actions;
        response.context_menu(|ui| {
            if let Some(path) = &target {
                item_menu(ui, done, path, actions);
                ui.separator();
            }
            if ui.add_enabled(!zoom.is_empty(), egui::Button::new("Zoom out")).clicked() {
                actions.push(Action::Zoom(zoom[..zoom.len() - 1].to_vec()));
                ui.close();
            }
        });
    }
}

impl DiskMapApp {
    /// The strip above the map while a file type is highlighted.
    fn type_bar(&mut self, ui: &mut Ui, bar: Rect, ext: &str) {
        ui.painter().rect_filled(bar, 0, theme::palette().surface);
        ui.painter()
            .hline(bar.x_range(), bar.bottom() - 0.5, Stroke::new(1.0, theme::palette().line));
        let Some(file_type) = self.types.iter().find(|t| t.ext == ext) else {
            return;
        };

        let mut strip = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(bar.shrink2(vec2(12.0, 0.0)))
                .layout(Layout::left_to_right(Align::Center)),
        );
        let ui = &mut strip;
        let (swatch, _) = ui.allocate_exact_size(vec2(12.0, 12.0), Sense::hover());
        ui.painter().rect_filled(swatch, 3, file_type.color);
        ui.label(RichText::new(format!("Highlighting {}", type_name(ext))).strong());
        ui.label(
            RichText::new(format!(
                "{} in {} files",
                scan::format_size(file_type.size),
                group_digits(file_type.files)
            ))
            .monospace()
            .color(theme::palette().ink_soft),
        );
        ui.add_space(12.0);
        let mut only = self.type_only;
        if ui.checkbox(&mut only, "Hide other types").changed() {
            self.actions.push(Action::SetTypeOnly(only));
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.button("Clear").on_hover_text("Or press Esc").clicked() {
                self.actions.push(Action::ClearType);
            }
        });
    }
}

/// One mesh for the whole map. Each tile is lighter at its top-left corner and
/// darker at its bottom-right, which gives WinDirStat's raised "cushion" look
/// and makes neighbouring files of the same type easy to tell apart.
fn cushion_mesh(tiles: &[Tile], dimmed: &dyn Fn(&Tile) -> bool) -> Mesh {
    let mut mesh = Mesh::default();
    for tile in tiles.iter().filter(|t| t.is_leaf) {
        let color = if dimmed(tile) { theme::dim(tile.color) } else { tile.color };
        let r = if tile.rect.width() > 3.0 && tile.rect.height() > 3.0 {
            tile.rect.shrink(0.5)
        } else {
            tile.rect
        };
        let base = mesh.vertices.len() as u32;
        mesh.colored_vertex(r.left_top(), theme::shade(color, 1.2));
        mesh.colored_vertex(r.right_top(), color);
        mesh.colored_vertex(r.left_bottom(), color);
        mesh.colored_vertex(r.right_bottom(), theme::shade(color, 0.5));
        mesh.add_triangle(base, base + 1, base + 2);
        mesh.add_triangle(base + 1, base + 3, base + 2);
    }
    mesh
}

// ---- Full Disk Access -----------------------------------------------------

/// Explains Full Disk Access and how to turn it on. Shown on the start screen
/// and in the pop-up from the toolbar's warning.
fn access_panel(ui: &mut Ui, granted: bool, checked_again: bool, host: &str, actions: &mut Vec<Action>) {
    if granted {
        ui.horizontal(|ui| {
            let (dot, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
            ui.painter().circle_filled(dot.center(), 5.0, theme::palette().ok);
            ui.label("Full Disk Access is on. Scans can see every folder.");
        });
        return;
    }

    ui.label(group_label("Full Disk Access is off"));
    ui.add_space(4.0);
    ui.label(RichText::new("DiskMap can't see every folder yet").font(theme::display_font(22.0)));
    ui.add_space(8.0);
    ui.label(
        RichText::new(
            "macOS keeps some folders private, even from administrators: Mail, Messages, Safari, \
             and parts of other apps' data. Until you allow Full Disk Access, scans skip them and \
             your totals will come up short. It's the Mac's version of \"Run as administrator\", \
             and sudo doesn't get around it.",
        )
        .color(theme::palette().ink_soft),
    );
    ui.add_space(10.0);
    for step in [
        "1. Open Full Disk Access in System Settings.".to_owned(),
        format!("2. Turn on {host}. If it isn't in the list, click + and choose it."),
        format!("3. Quit {host} and open it again. macOS only applies the change on restart."),
    ] {
        ui.label(step);
    }
    if !access::is_bundled() {
        ui.add_space(6.0);
        ui.label(
            RichText::new(format!(
                "DiskMap is running from {host}, so macOS gives the permission to {host}. \
                 Run as DiskMap.app, it asks for itself."
            ))
            .size(12.5)
            .color(theme::palette().ink_soft),
        );
    }
    ui.add_space(12.0);
    ui.horizontal(|ui| {
        if ui.button("Open Full Disk Access settings").clicked() {
            actions.push(Action::OpenAccessSettings);
        }
        if ui.button("Check again").clicked() {
            actions.push(Action::CheckAccess);
        }
    });
    if checked_again {
        ui.add_space(6.0);
        ui.label(RichText::new(format!("Still off. If you just turned it on, quit {host} and open it again.")).color(theme::palette().bad));
    }
}

// ---- About ----------------------------------------------------------------

/// The repository on GitHub.
const REPOSITORY: &str = "https://github.com/theheadgeek/DiskMap";

/// The About window. Returns true when the user closes it.
fn about_panel(ui: &mut Ui) -> bool {
    let mut close = false;
    ui.vertical_centered(|ui| {
        ui.add_space(4.0);
        ui.label(RichText::new("DiskMap").font(theme::display_font(30.0)));
        ui.label(
            RichText::new(format!("Version {}", env!("CARGO_PKG_VERSION")))
                .monospace()
                .color(theme::palette().ink_soft),
        );
        ui.add_space(10.0);
        ui.label(
            RichText::new("See what's using your disk. Every file becomes a rectangle sized by the space it takes.")
                .color(theme::palette().ink_soft),
        );
        ui.add_space(8.0);
        ui.hyperlink_to("github.com/theheadgeek/DiskMap", REPOSITORY);
        ui.label(
            RichText::new("Free and open source, under the MIT license.")
                .size(12.5)
                .color(theme::palette().ink_soft),
        );
        ui.add_space(20.0);
        brand_footer(ui);
        ui.add_space(12.0);
        close = ui.button("Close").clicked();
    });
    close
}

/// The design system's footer: the cube at 40px, centred, above the copyright line.
fn brand_footer(ui: &mut Ui) {
    ui.vertical_centered(|ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(40.0, 40.0), Sense::hover());
        brand::paint_cube(ui.painter(), rect);
        ui.add_space(6.0);
        ui.label(RichText::new(brand::copyright()).size(12.5).color(theme::palette().ink_soft));
    });
}

// ---- Small helpers --------------------------------------------------------

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"))
}

/// Shows the scanned folder as "~/Documents" when it's inside the home folder.
fn display_root(root: &Path) -> String {
    match root.strip_prefix(home()) {
        Ok(rest) if rest.as_os_str().is_empty() => "~".to_owned(),
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => root.display().to_string(),
    }
}

/// ".jpg", or "(no extension)" for files without one.
fn type_name(ext: &str) -> String {
    if ext.is_empty() {
        "(no extension)".to_owned()
    } else {
        format!(".{ext}")
    }
}

/// The design system's `group-label` style: small, uppercase, tracked, ink-soft.
fn group_label(text: &str) -> RichText {
    RichText::new(text.to_uppercase())
        .size(11.5)
        .strong()
        .extra_letter_spacing(1.6)
        .color(theme::palette().ink_soft)
}

fn percent(part: u64, whole: u64) -> f64 {
    if whole == 0 { 0.0 } else { part as f64 * 100.0 / whole as f64 }
}

/// 1234567 -> "1,234,567"
fn group_digits(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}
