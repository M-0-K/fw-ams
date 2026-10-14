//! System tray icon with a live status line.

#[allow(unused_imports)]
use fw_ams_core::model::{ChargeState, Snapshot};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use crate::flyout;
use tauri::{AppHandle, Manager};

const TRAY_ID: &str = "main";

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Öffnen", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Beenden", true, None::<&str>)?;
    // AppIndicator has no hover events, so offer the flyout in the menu there
    let menu = if cfg!(target_os = "linux") {
        let quick = MenuItem::with_id(app, "flyout", "Schnellübersicht", true, None::<&str>)?;
        Menu::with_items(app, &[&quick, &show, &quit])?
    } else {
        Menu::with_items(app, &[&show, &quit])?
    };

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "flyout" => flyout::show_in_corner(app),
            "show" => crate::show_main_window(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| match event {
            TrayIconEvent::Enter { rect, .. } => flyout::show_at_tray(tray.app_handle(), &rect),
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } => {
                flyout::hide(tray.app_handle());
                toggle_main_window(tray.app_handle());
            }
            _ => {}
        });
    // The hover flyout replaces the tooltip where hover events exist
    if cfg!(target_os = "linux") {
        builder = builder.tooltip("fw-ams");
    }
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

fn toggle_main_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        if w.is_visible().unwrap_or(false) {
            let _ = w.hide();
        } else {
            crate::show_main_window(app);
        }
    }
}

/// e.g. "59 % · lädt +39.5 W · CPU 60 °C"
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn status_line(s: &Snapshot) -> String {
    let mut parts = vec![];
    if let Some(b) = &s.battery {
        let state = match b.state {
            ChargeState::Charging => "lädt",
            ChargeState::Discharging => "Akku",
            ChargeState::Idle => "Netz",
            ChargeState::Unknown => "",
        };
        let power = b.power_w.map(|w| format!(" {w:+.1} W")).unwrap_or_default();
        parts.push(format!("{:.0} % · {state}{power}", b.soc_percent));
    }
    if let Some(t) = &s.thermal {
        let hottest = t
            .sensors
            .iter()
            .filter_map(|s| s.celsius.map(|c| (s, c)))
            .max_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((sensor, c)) = hottest {
            parts.push(format!("{} {c:.0} °C", sensor.label));
        }
    }
    parts.join(" · ")
}

/// Linux only: Windows/macOS show the hover flyout instead of a tooltip.
#[cfg(target_os = "linux")]
pub fn update(app: &AppHandle, s: &Snapshot) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };
    let line = status_line(s);
    let _ = tray.set_tooltip(Some(format!("fw-ams\n{line}")));
    // AppIndicator ignores tooltips; show the text next to the icon instead
    let _ = tray.set_title(Some(&line));
}

#[cfg(not(target_os = "linux"))]
pub fn update(_app: &AppHandle, _s: &Snapshot) {}
