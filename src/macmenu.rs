//! Points the menu bar's "About DiskMap" at DiskMap's own About window.
//!
//! The window library builds a standard Mac menu whose About item opens
//! macOS's generic About panel. Here we find that item and give it a new
//! target: a tiny Objective-C object whose one method asks the app to show
//! its About window on the next frame.

use eframe::egui;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::NSApplication;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

/// Set when the menu item is chosen; the app checks it each frame.
static REQUESTED: AtomicBool = AtomicBool::new(false);
/// Lets the menu item wake the app, which may be idle and not redrawing.
static CONTEXT: OnceLock<egui::Context> = OnceLock::new();

define_class!(
    // SAFETY: NSObject has no subclassing requirements, and AboutTarget doesn't implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "DiskMapAboutTarget"]
    struct AboutTarget;

    impl AboutTarget {
        #[unsafe(method(showAbout:))]
        fn show_about(&self, _sender: Option<&AnyObject>) {
            REQUESTED.store(true, Ordering::Relaxed);
            if let Some(ctx) = CONTEXT.get() {
                ctx.request_repaint();
            }
        }
    }
);

impl AboutTarget {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(());
        // SAFETY: NSObject's init takes no arguments and returns the initialised object.
        unsafe { msg_send![super(this), init] }
    }
}

/// Redirects the About item. Returns false if the menu isn't built yet, so
/// the caller can try again on a later frame.
pub fn install(ctx: &egui::Context) -> bool {
    let Some(mtm) = MainThreadMarker::new() else { return false };
    let _ = CONTEXT.set(ctx.clone());

    let app = NSApplication::sharedApplication(mtm);
    let Some(app_menu) = app.mainMenu().and_then(|menu| menu.itemAtIndex(0)).and_then(|item| item.submenu()) else {
        return false;
    };
    for index in 0..app_menu.numberOfItems() {
        let Some(item) = app_menu.itemAtIndex(index) else { continue };
        if item.action() == Some(sel!(orderFrontStandardAboutPanel:)) {
            let target = AboutTarget::new(mtm);
            // SAFETY: AboutTarget responds to showAbout:, which takes the sender as its one argument.
            unsafe {
                item.setTarget(Some(&target));
                item.setAction(Some(sel!(showAbout:)));
            }
            // A menu item doesn't keep its target alive, so keep this one for the rest of the run.
            std::mem::forget(target);
            return true;
        }
    }
    false
}

/// True once for each time the About item was chosen.
pub fn about_requested() -> bool {
    REQUESTED.swap(false, Ordering::Relaxed)
}
