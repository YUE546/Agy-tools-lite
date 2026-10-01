//! Desktop lifecycle and the small menu-bar dashboard. OS login registration is
//! changed only by the explicit settings command, never on startup/config load.
use crate::{models::config::DesktopPreferences, modules};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{Emitter, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_autostart::ManagerExt;

pub const DASHBOARD_LABEL: &str = "menubar";

#[derive(Default)]
pub struct DesktopRuntime {
    tray_available: AtomicBool,
    preferences_lock: tokio::sync::Mutex<()>,
    panel_transition: std::sync::Mutex<()>,
    #[cfg(target_os = "macos")]
    appearance_lock: std::sync::Mutex<()>,
    #[cfg(target_os = "macos")]
    material_applied: AtomicBool,
}

pub fn set_tray_available(app: &tauri::AppHandle, available: bool) {
    app.state::<DesktopRuntime>()
        .tray_available
        .store(available, Ordering::Relaxed);
}

pub fn tray_available(app: &tauri::AppHandle) -> bool {
    app.state::<DesktopRuntime>()
        .tray_available
        .load(Ordering::Relaxed)
}

#[derive(Clone, Serialize)]
pub struct MenuBarAppearance {
    platform: &'static str,
    native_material: bool,
    reduced_transparency: bool,
    high_contrast: bool,
}

fn use_native_material(macos: bool, reduced_transparency: bool, high_contrast: bool) -> bool {
    macos && !reduced_transparency && !high_contrast
}

/// Read accessibility preferences without changing any system setting. Recheck
/// whenever the panel opens, including after a visit to System Settings.
#[tauri::command]
pub async fn get_menu_bar_appearance(app: tauri::AppHandle) -> Result<MenuBarAppearance, String> {
    // Native effects may dispatch to the UI thread. Never block that same thread
    // on the appearance mutex while another caller is applying a material.
    tauri::async_runtime::spawn_blocking(move || apply_menu_bar_appearance(&app))
        .await
        .map_err(|e| e.to_string())?
}

fn apply_menu_bar_appearance(app: &tauri::AppHandle) -> Result<MenuBarAppearance, String> {
    #[cfg(target_os = "macos")]
    let (reduced_transparency, high_contrast) = {
        let workspace = objc2_app_kit::NSWorkspace::sharedWorkspace();
        (
            workspace.accessibilityDisplayShouldReduceTransparency(),
            workspace.accessibilityDisplayShouldIncreaseContrast(),
        )
    };
    #[cfg(not(target_os = "macos"))]
    let (reduced_transparency, high_contrast) = (false, false);
    let native_material = use_native_material(
        cfg!(target_os = "macos"),
        reduced_transparency,
        high_contrast,
    );
    #[cfg(target_os = "macos")]
    let native_material = {
        let state = app.state::<DesktopRuntime>();
        let _guard = state.appearance_lock.lock().map_err(|e| e.to_string())?;
        if let Some(window) = app.get_webview_window(DASHBOARD_LABEL) {
            let applied = state.material_applied.load(Ordering::Relaxed);
            if native_material && !applied {
                use tauri::window::{Effect, EffectState, EffectsBuilder};
                let success = window
                    .set_effects(
                        EffectsBuilder::new()
                            .effect(Effect::Popover)
                            .state(EffectState::Active)
                            .radius(10.0)
                            .build(),
                    )
                    .is_ok();
                state.material_applied.store(success, Ordering::Relaxed);
                success
            } else if !native_material {
                if applied {
                    let _ = window.set_effects(None);
                }
                state.material_applied.store(false, Ordering::Relaxed);
                false
            } else {
                true
            }
        } else {
            false
        }
    };
    #[cfg(not(target_os = "macos"))]
    let _ = app;
    Ok(MenuBarAppearance {
        platform: std::env::consts::OS,
        native_material,
        reduced_transparency,
        high_contrast,
    })
}

#[derive(Serialize)]
pub struct DesktopStatus {
    platform: &'static str,
    tray_available: bool,
    autostart_supported: bool,
    launch_at_login: Option<bool>,
    autostart_error: Option<String>,
    hide_dock_icon: bool,
    start_minimized: bool,
}

#[derive(Default, Deserialize)]
pub struct DesktopPatch {
    launch_at_login: Option<bool>,
    hide_dock_icon: Option<bool>,
    start_minimized: Option<bool>,
}

#[tauri::command]
pub fn get_desktop_settings(app: tauri::AppHandle) -> Result<DesktopStatus, String> {
    let preferences = modules::load_app_config()?.desktop;
    // Query the OS rather than trusting a JSON flag after a System Settings edit.
    let (launch_at_login, autostart_error) = match app.autolaunch().is_enabled() {
        Ok(enabled) => (Some(enabled), None),
        Err(error) => (None, Some(error.to_string())),
    };
    Ok(DesktopStatus {
        platform: std::env::consts::OS,
        tray_available: tray_available(&app),
        autostart_supported: !cfg!(debug_assertions),
        launch_at_login,
        autostart_error,
        hide_dock_icon: preferences.hide_dock_icon,
        start_minimized: preferences.start_minimized,
    })
}

#[tauri::command]
pub async fn set_desktop_preferences(
    app: tauri::AppHandle,
    patch: DesktopPatch,
) -> Result<DesktopStatus, String> {
    let state = app.state::<DesktopRuntime>();
    let _guard = state.preferences_lock.lock().await;
    let mut config = modules::load_app_config()?;
    let old = config.desktop.clone();
    let mut next = old.clone();
    if let Some(value) = patch.hide_dock_icon {
        if !cfg!(target_os = "macos") {
            return Err("Dock visibility is only supported on macOS".into());
        }
        if value && !tray_available(&app) {
            return Err("The menu bar is unavailable. Keep the Dock icon visible so the app stays accessible.".into());
        }
        next.hide_dock_icon = value;
    }
    if let Some(value) = patch.start_minimized {
        if value && !tray_available(&app) {
            return Err(
                "A working menu bar or tray is required to start in the background.".into(),
            );
        }
        next.start_minimized = value;
    }
    let old_autostart = if let Some(value) = patch.launch_at_login {
        if cfg!(debug_assertions) {
            return Err(
                "Launch at login is available in installed release builds, not development builds."
                    .into(),
            );
        }
        let actual = app.autolaunch().is_enabled().map_err(|e| e.to_string())?;
        if value != actual {
            if value {
                app.autolaunch().enable()
            } else {
                app.autolaunch().disable()
            }
            .map_err(|e| e.to_string())?;
        }
        next.launch_at_login = value;
        Some(actual)
    } else {
        None
    };
    let result = apply_dock_preference(&app, &next).and_then(|_| {
        config.desktop = next;
        modules::save_app_config(&config)
    });
    if let Err(error) = result {
        let _ = apply_dock_preference(&app, &old);
        if let Some(was_enabled) = old_autostart {
            let rollback = if was_enabled {
                app.autolaunch().enable()
            } else {
                app.autolaunch().disable()
            };
            if let Err(rollback_error) = rollback {
                return Err(format!("{error}; login setting rollback failed: {rollback_error}. Reopen settings to inspect the actual state."));
            }
        }
        return Err(error);
    }
    let _ = app.emit("config://updated", ());
    get_desktop_settings(app.clone())
}

fn apply_dock_preference(
    app: &tauri::AppHandle,
    preferences: &DesktopPreferences,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let policy = if preferences.hide_dock_icon && tray_available(app) {
            tauri::ActivationPolicy::Accessory
        } else {
            tauri::ActivationPolicy::Regular
        };
        app.set_activation_policy(policy)
            .map_err(|e| e.to_string())?;
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (app, preferences);
    Ok(())
}

pub fn initialize(app: &tauri::AppHandle) -> Result<(), String> {
    let config = modules::load_app_config().unwrap_or_default();
    apply_dock_preference(app, &config.desktop)?;
    let autostart = std::env::args().any(|arg| arg == "--autostart");
    if !start_hidden(
        autostart,
        config.desktop.start_minimized,
        tray_available(app),
    ) {
        show_main(app)?;
    }
    Ok(())
}

fn start_hidden(autostart: bool, minimized: bool, available: bool) -> bool {
    autostart && minimized && available
}

pub fn show_main(app: &tauri::AppHandle) -> Result<(), String> {
    let preferences = modules::load_app_config().unwrap_or_default().desktop;
    apply_dock_preference(app, &preferences)?;
    if let Some(popover) = app.get_webview_window(DASHBOARD_LABEL) {
        let _ = popover.hide();
    }
    let window = app
        .get_webview_window("main")
        .ok_or("Main window is unavailable")?;
    window.unminimize().map_err(|e| e.to_string())?;
    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_app_page(app: tauri::AppHandle, page: String) -> Result<(), String> {
    let route = match page.as_str() {
        "dashboard" => "/",
        "accounts" => "/accounts",
        "settings" => "/settings",
        _ => return Err("Unknown application page".into()),
    };
    show_main(&app)?;
    app.emit_to("main", "app://navigate", route)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn hide_menu_bar_dashboard(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(DASHBOARD_LABEL) {
        window.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn quit_app(app: tauri::AppHandle) {
    app.exit(0);
}

/// Clamp in physical pixels; negative monitor origins and mixed DPI are valid.
fn panel_bounds(
    anchor: (f64, f64, f64, f64),
    area: (f64, f64, f64, f64),
    scale: f64,
) -> (f64, f64, f64, f64) {
    let (ax, ay, aw, ah) = anchor;
    let (x, y, w, h) = area;
    let margin = 8.0 * scale;
    let width = (380.0 * scale).min((w - margin * 2.0).max(1.0));
    let height = (480.0 * scale).min((h - margin * 2.0).max(1.0));
    let px =
        (ax + aw / 2.0 - width / 2.0).clamp(x + margin, (x + w - width - margin).max(x + margin));
    let below = ay + ah + margin;
    let py = if below + height <= y + h - margin {
        below
    } else {
        ay - height - margin
    };
    (
        px,
        py.clamp(y + margin, (y + h - height - margin).max(y + margin)),
        width,
        height,
    )
}

pub fn toggle_dashboard(app: &tauri::AppHandle, rect: Option<tauri::Rect>) -> Result<(), String> {
    let runtime = app.state::<DesktopRuntime>();
    let _transition = runtime.panel_transition.lock().map_err(|e| e.to_string())?;
    if let Some(window) = app.get_webview_window(DASHBOARD_LABEL) {
        if window.is_visible().unwrap_or(false) {
            return window.hide().map_err(|e| e.to_string());
        }
    }
    let window = match app.get_webview_window(DASHBOARD_LABEL) {
        Some(window) => window,
        None => {
            let builder =
                WebviewWindowBuilder::new(app, DASHBOARD_LABEL, WebviewUrl::App("menubar".into()))
                    .title("Antigravity · Quick Dashboard")
                    .inner_size(380.0, 480.0)
                    .resizable(false)
                    .decorations(false)
                    .visible(false)
                    .skip_taskbar(true)
                    .always_on_top(true)
                    .shadow(true);
            #[cfg(target_os = "macos")]
            let builder = builder
                .transparent(true)
                .background_color(tauri::window::Color(0, 0, 0, 0));
            builder.build().map_err(|e| e.to_string())?
        }
    };
    let anchor = rect.or_else(|| {
        app.tray_by_id("main")
            .and_then(|tray| tray.rect().ok().flatten())
    });
    let point = anchor
        .map(|r| r.position.to_physical::<f64>(1.0))
        .or_else(|| app.cursor_position().ok());
    let monitor = point
        .and_then(|p| app.monitor_from_point(p.x, p.y).ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten());
    if let Some(monitor) = monitor {
        let scale = monitor.scale_factor();
        let area = monitor.work_area();
        let anchor = anchor
            .map(|r| {
                let pos = r.position.to_physical::<f64>(scale);
                let size = r.size.to_physical::<f64>(scale);
                (pos.x, pos.y, size.width, size.height)
            })
            .unwrap_or((
                f64::from(area.position.x + area.size.width as i32),
                f64::from(area.position.y),
                0.0,
                0.0,
            ));
        let (x, y, width, height) = panel_bounds(
            anchor,
            (
                f64::from(area.position.x),
                f64::from(area.position.y),
                f64::from(area.size.width),
                f64::from(area.size.height),
            ),
            scale,
        );
        window
            .set_size(tauri::PhysicalSize::new(width as u32, height as u32))
            .map_err(|e| e.to_string())?;
        window
            .set_position(PhysicalPosition::new(x as i32, y as i32))
            .map_err(|e| e.to_string())?;
    } else {
        window.center().map_err(|e| e.to_string())?;
    }
    let appearance = apply_menu_bar_appearance(app)?;
    let _ = window.emit("menubar://appearance", &appearance);
    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())?;
    let _ = window.emit("menubar://opened", ());
    Ok(())
}

pub fn handle_window_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    if window.label() == DASHBOARD_LABEL {
        match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                let _ = window.hide();
            }
            tauri::WindowEvent::Focused(false) => {
                let window = window.clone();
                // Let a second tray click toggle the still-visible panel before
                // handling blur, avoiding the familiar hide-then-reopen race.
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                    if !window.is_focused().unwrap_or(false) {
                        let _ = window.hide();
                    }
                });
            }
            _ => {}
        }
    } else if window.label() == "main" {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            if tray_available(window.app_handle()) {
                // Never hide the only recovery path when the tray failed.
                if window.hide().is_ok() {
                    api.prevent_close();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{panel_bounds, start_hidden, use_native_material};
    #[test]
    fn material_requires_macos_and_accessibility_opt_in() {
        assert!(use_native_material(true, false, false));
        assert!(!use_native_material(false, false, false));
        assert!(!use_native_material(true, true, false));
        assert!(!use_native_material(true, false, true));
    }
    #[test]
    fn background_start_requires_every_safety_condition() {
        for autostart in [false, true] {
            for minimized in [false, true] {
                for tray in [false, true] {
                    assert_eq!(
                        start_hidden(autostart, minimized, tray),
                        autostart && minimized && tray
                    );
                }
            }
        }
    }
    #[test]
    fn popover_fits_small_negative_origin_monitor() {
        let (x, y, w, h) = panel_bounds(
            (-12.0, -32.0, 20.0, 24.0),
            (-800.0, -10.0, 800.0, 540.0),
            1.0,
        );
        assert!(x >= -792.0 && x + w <= -8.0);
        assert!(y >= -2.0 && y + h <= 522.0);
    }
    #[test]
    fn popover_scales_and_opens_above_bottom_tray() {
        let (x, y, w, h) = panel_bounds(
            (2800.0, 1760.0, 40.0, 40.0),
            (0.0, 0.0, 2880.0, 1760.0),
            2.0,
        );
        assert_eq!(w, 760.0);
        assert_eq!(h, 960.0);
        assert!(x + w <= 2864.0 && y + h < 1760.0);
    }
}
