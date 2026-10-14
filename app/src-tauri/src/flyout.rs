//! Small borderless status window that pops up while hovering the tray icon.
//!
//! Windows/macOS report tray Enter/Leave events; a watcher thread hides the flyout once the
//! cursor has left both the icon and the flyout. AppIndicator (Linux) has no hover events,
//! so there the flyout is opened from the tray menu and hidden when it loses focus.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tauri::{
    AppHandle, Manager, PhysicalPosition, PhysicalSize, Rect, WebviewUrl, WebviewWindowBuilder,
};

pub const LABEL: &str = "flyout";
const WIDTH: f64 = 380.0;
const HEIGHT: f64 = 420.0;
/// Gap between tray icon and flyout
const GAP: i32 = 12;
/// How long the cursor may be outside icon and flyout before hiding
const HIDE_DELAY: Duration = Duration::from_millis(350);
const POLL: Duration = Duration::from_millis(100);

static WATCHING: AtomicBool = AtomicBool::new(false);

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html?view=flyout".into()))
        .title("fw-ams")
        .inner_size(WIDTH, HEIGHT)
        .decorations(false)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible(false)
        .focused(false)
        // Linux hides the flyout on blur, so it must be able to take focus there
        .focusable(cfg!(target_os = "linux"))
        .shadow(true)
        .build()?;
    Ok(())
}

#[derive(Clone, Copy)]
struct PxRect {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

impl PxRect {
    fn contains(&self, p: PhysicalPosition<f64>, margin: i32) -> bool {
        let (x, y) = (p.x as i32, p.y as i32);
        x >= self.x - margin && x <= self.x + self.w + margin && y >= self.y - margin && y <= self.y + self.h + margin
    }
}

/// Show the flyout next to the tray icon (above it, or below for a top taskbar).
pub fn show_at_tray(app: &AppHandle, icon: &Rect) {
    let Some(w) = app.get_webview_window(LABEL) else {
        return;
    };
    let scale = w.scale_factor().unwrap_or(1.0);
    let pos: PhysicalPosition<i32> = icon.position.to_physical(scale);
    let size: PhysicalSize<u32> = icon.size.to_physical(scale);
    let icon = PxRect {
        x: pos.x,
        y: pos.y,
        w: size.width as i32,
        h: size.height as i32,
    };

    let win = w.outer_size().unwrap_or(PhysicalSize::new(
        (WIDTH * scale) as u32,
        (HEIGHT * scale) as u32,
    ));
    let (ww, wh) = (win.width as i32, win.height as i32);
    let mut x = icon.x + icon.w / 2 - ww / 2;
    let mut y = icon.y - wh - GAP;

    let monitor = app
        .monitor_from_point(icon.x as f64, icon.y as f64)
        .ok()
        .flatten()
        .or_else(|| app.primary_monitor().ok().flatten());
    if let Some(m) = monitor {
        let wa = m.work_area();
        let (ax, ay) = (wa.position.x, wa.position.y);
        let (aw, ah) = (wa.size.width as i32, wa.size.height as i32);
        if y < ay {
            // Taskbar at the top
            y = icon.y + icon.h + GAP;
        }
        x = x.clamp(ax + GAP, (ax + aw - ww - GAP).max(ax));
        y = y.clamp(ay + GAP, (ay + ah - wh - GAP).max(ay));
    }

    let _ = w.set_position(PhysicalPosition::new(x, y));
    let _ = w.show();
    watch(app.clone(), icon);
}

/// Open in the corner where the tray usually is: top right on Linux, bottom right otherwise.
pub fn show_in_corner(app: &AppHandle) {
    let Some(w) = app.get_webview_window(LABEL) else {
        return;
    };
    if let (Ok(Some(m)), Ok(size)) = (app.primary_monitor(), w.outer_size()) {
        let wa = m.work_area();
        let x = wa.position.x + wa.size.width as i32 - size.width as i32 - GAP;
        let y = if cfg!(target_os = "linux") {
            wa.position.y + GAP
        } else {
            wa.position.y + wa.size.height as i32 - size.height as i32 - GAP
        };
        let _ = w.set_position(PhysicalPosition::new(x, y));
    }
    let _ = w.show();
    let _ = w.set_focus();
}

pub fn hide(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.hide();
    }
}

fn watch(app: AppHandle, icon: PxRect) {
    if WATCHING.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(move || {
        let mut outside_since: Option<Instant> = None;
        loop {
            std::thread::sleep(POLL);
            let Some(w) = app.get_webview_window(LABEL) else {
                break;
            };
            if !w.is_visible().unwrap_or(false) {
                break;
            }
            let (Ok(cursor), Ok(pos), Ok(size)) = (app.cursor_position(), w.outer_position(), w.outer_size()) else {
                break;
            };
            let flyout = PxRect {
                x: pos.x,
                y: pos.y,
                w: size.width as i32,
                h: size.height as i32,
            };
            // Margin bridges the gap between icon and flyout
            if icon.contains(cursor, GAP) || flyout.contains(cursor, GAP) {
                outside_since = None;
            } else if outside_since.get_or_insert_with(Instant::now).elapsed() >= HIDE_DELAY {
                let _ = w.hide();
                break;
            }
        }
        WATCHING.store(false, Ordering::SeqCst);
    });
}
