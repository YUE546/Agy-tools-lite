use std::fs;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use super::account::get_data_dir;
use crate::models::config::DesktopPreferences;
use crate::models::AppConfig;

const CONFIG_FILE: &str = "gui_config.json";
// All in-process configuration reads and writes share one lock. In particular,
// a stale whole-settings snapshot cannot race the dedicated localization toggle.
static CONFIG_LOCK: Mutex<()> = Mutex::new(());

fn lock_config() -> Result<MutexGuard<'static, ()>, String> {
    CONFIG_LOCK
        .lock()
        .map_err(|_| "config_lock_unavailable".to_string())
}

// These helpers are called only while CONFIG_LOCK is held. Missing files are
// distinct from unreadable or malformed files, which must never be overwritten.
fn read_config_unlocked(path: &Path) -> Result<Option<AppConfig>, String> {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("failed_to_read_config_file: {error}")),
    };
    serde_json::from_str(&content)
        .map(Some)
        .map_err(|error| format!("failed_to_parse_config_file: {error}"))
}

fn write_config_unlocked(path: &Path, config: &AppConfig) -> Result<(), String> {
    let content = serde_json::to_string_pretty(config)
        .map_err(|error| format!("failed_to_serialize_config: {error}"))?;
    crate::utils::fs::write_atomic(path, content.as_bytes())
        .map_err(|error| format!("failed_to_save_config: {error}"))
}

fn load_config_at(path: &Path) -> Result<AppConfig, String> {
    let _guard = lock_config()?;
    if let Some(config) = read_config_unlocked(path)? {
        return Ok(config);
    }
    let config = AppConfig::new();
    // Preserve best-effort first-run persistence without recursively locking.
    let _ = write_config_unlocked(path, &config);
    Ok(config)
}

fn save_config_at(path: &Path, config: &AppConfig) -> Result<(), String> {
    let _guard = lock_config()?;
    let mut next = config.clone();
    // Only dedicated setters may change these preferences. Ordinary settings
    // saves preserve current disk values even if their UI snapshot is stale.
    // New/legacy configurations always start with both opt-ins disabled.
    let current = read_config_unlocked(path)?.unwrap_or_default();
    next.app_localization = current.app_localization;
    next.desktop = current.desktop;
    write_config_unlocked(path, &next)
}

fn set_localization_enabled_at(path: &Path, enabled: bool) -> Result<(), String> {
    let _guard = lock_config()?;
    let mut config = read_config_unlocked(path)?.unwrap_or_default();
    config.app_localization.enabled = enabled;
    write_config_unlocked(path, &config)
}

fn set_desktop_preferences_at(path: &Path, preferences: &DesktopPreferences) -> Result<(), String> {
    let _guard = lock_config()?;
    let mut config = read_config_unlocked(path)?.unwrap_or_default();
    config.desktop = preferences.clone();
    write_config_unlocked(path, &config)
}

/// Load application configuration.
pub fn load_app_config() -> Result<AppConfig, String> {
    load_config_at(&get_data_dir()?.join(CONFIG_FILE))
}

/// Save ordinary application settings atomically, preserving the separately
/// managed localization and desktop preferences from current disk configuration.
pub fn save_app_config(config: &AppConfig) -> Result<(), String> {
    save_config_at(&get_data_dir()?.join(CONFIG_FILE), config)
}

/// Atomically change only the App-localization preference under the same lock
/// used by ordinary configuration writes, preserving all other current settings.
pub fn set_app_localization_enabled(enabled: bool) -> Result<(), String> {
    set_localization_enabled_at(&get_data_dir()?.join(CONFIG_FILE), enabled)
}

/// Persist only the desktop preferences after their OS-backed transaction.
/// The dedicated writer retains localization and ordinary settings saved by
/// another window while the OS operation was in progress.
pub fn set_saved_desktop_preferences(preferences: &DesktopPreferences) -> Result<(), String> {
    set_desktop_preferences_at(&get_data_dir()?.join(CONFIG_FILE), preferences)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_configuration_persists_localization_off_without_recursive_locking() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join(CONFIG_FILE);
        let config = load_config_at(&path).unwrap();
        assert!(!config.app_localization.enabled);
        assert!(path.is_file());
        assert!(!load_config_at(&path).unwrap().app_localization.enabled);
    }

    #[test]
    fn ordinary_first_save_cannot_enable_localization() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join(CONFIG_FILE);
        let mut proposed = AppConfig::new();
        proposed.app_localization.enabled = true;
        proposed.theme = "dark".into();
        save_config_at(&path, &proposed).unwrap();
        let actual = load_config_at(&path).unwrap();
        assert!(!actual.app_localization.enabled);
        assert_eq!(actual.theme, "dark");
    }

    #[test]
    fn stale_theme_save_cannot_undo_a_dedicated_disable() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join(CONFIG_FILE);
        set_localization_enabled_at(&path, true).unwrap();
        let mut stale = load_config_at(&path).unwrap();
        set_localization_enabled_at(&path, false).unwrap();
        stale.theme = "dark".into();
        save_config_at(&path, &stale).unwrap();
        let actual = load_config_at(&path).unwrap();
        assert!(!actual.app_localization.enabled);
        assert_eq!(actual.theme, "dark");
    }

    #[test]
    fn stale_language_save_cannot_undo_a_dedicated_enable() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join(CONFIG_FILE);
        let mut stale = load_config_at(&path).unwrap();
        set_localization_enabled_at(&path, true).unwrap();
        stale.language = "en".into();
        stale.refresh_interval = 60;
        save_config_at(&path, &stale).unwrap();
        let actual = load_config_at(&path).unwrap();
        assert!(actual.app_localization.enabled);
        assert_eq!(actual.language, "en");
        assert_eq!(actual.refresh_interval, 60);
        set_localization_enabled_at(&path, false).unwrap();
        let actual = load_config_at(&path).unwrap();
        assert!(!actual.app_localization.enabled);
        assert_eq!(actual.language, "en");
        assert_eq!(actual.refresh_interval, 60);
    }

    #[test]
    fn malformed_configuration_is_not_overwritten_by_any_writer() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join(CONFIG_FILE);
        let original = b"{\"theme\": broken existing data";
        fs::write(&path, original).unwrap();
        assert!(load_config_at(&path)
            .unwrap_err()
            .starts_with("failed_to_parse_config_file:"));
        assert!(save_config_at(&path, &AppConfig::new())
            .unwrap_err()
            .starts_with("failed_to_parse_config_file:"));
        assert!(set_localization_enabled_at(&path, false)
            .unwrap_err()
            .starts_with("failed_to_parse_config_file:"));
        assert_eq!(fs::read(&path).unwrap(), original);
    }

    #[test]
    fn unreadable_configuration_is_not_treated_as_a_missing_file() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join(CONFIG_FILE);
        fs::create_dir(&path).unwrap();
        fs::write(path.join("existing"), b"preserve").unwrap();
        assert!(load_config_at(&path)
            .unwrap_err()
            .starts_with("failed_to_read_config_file:"));
        assert!(save_config_at(&path, &AppConfig::new())
            .unwrap_err()
            .starts_with("failed_to_read_config_file:"));
        assert!(set_localization_enabled_at(&path, true)
            .unwrap_err()
            .starts_with("failed_to_read_config_file:"));
        assert_eq!(fs::read(path.join("existing")).unwrap(), b"preserve");
    }

    #[test]
    fn concurrent_ordinary_saves_preserve_the_final_dedicated_toggle() {
        use std::sync::{Arc, Barrier};
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join(CONFIG_FILE);
        set_localization_enabled_at(&path, true).unwrap();
        let mut stale = load_config_at(&path).unwrap();
        stale.theme = "dark".into();
        let start = Arc::new(Barrier::new(2));
        std::thread::scope(|scope| {
            let writer_start = start.clone();
            let writer_path = path.clone();
            scope.spawn(move || {
                writer_start.wait();
                for _ in 0..16 {
                    save_config_at(&writer_path, &stale).unwrap();
                }
            });
            start.wait();
            for _ in 0..16 {
                set_localization_enabled_at(&path, true).unwrap();
                set_localization_enabled_at(&path, false).unwrap();
            }
        });
        let actual = load_config_at(&path).unwrap();
        assert!(!actual.app_localization.enabled);
        assert_eq!(actual.theme, "dark");
    }

    #[test]
    fn ordinary_first_save_cannot_enable_desktop_preferences() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join(CONFIG_FILE);
        let mut proposed = AppConfig::new();
        proposed.desktop = DesktopPreferences {
            launch_at_login: true,
            hide_dock_icon: true,
            start_minimized: true,
        };
        save_config_at(&path, &proposed).unwrap();
        let actual = load_config_at(&path).unwrap();
        assert!(!actual.desktop.launch_at_login);
        assert!(!actual.desktop.hide_dock_icon);
        assert!(!actual.desktop.start_minimized);
    }

    #[test]
    fn dedicated_preferences_survive_stale_ordinary_and_each_others_writes() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join(CONFIG_FILE);
        let mut stale = load_config_at(&path).unwrap();
        set_localization_enabled_at(&path, true).unwrap();
        let desktop = DesktopPreferences {
            launch_at_login: true,
            hide_dock_icon: true,
            start_minimized: true,
        };
        set_desktop_preferences_at(&path, &desktop).unwrap();
        stale.theme = "dark".into();
        save_config_at(&path, &stale).unwrap();
        let actual = load_config_at(&path).unwrap();
        assert!(actual.app_localization.enabled);
        assert!(actual.desktop.launch_at_login);
        assert!(actual.desktop.hide_dock_icon);
        assert!(actual.desktop.start_minimized);
        assert_eq!(actual.theme, "dark");

        let mut stale = actual;
        set_localization_enabled_at(&path, false).unwrap();
        set_desktop_preferences_at(&path, &DesktopPreferences::default()).unwrap();
        stale.language = "en".into();
        save_config_at(&path, &stale).unwrap();
        let actual = load_config_at(&path).unwrap();
        assert!(!actual.app_localization.enabled);
        assert!(!actual.desktop.launch_at_login);
        assert!(!actual.desktop.hide_dock_icon);
        assert!(!actual.desktop.start_minimized);
        assert_eq!(actual.theme, "dark");
        assert_eq!(actual.language, "en");
    }

    #[test]
    fn desktop_writer_refuses_to_overwrite_malformed_configuration() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join(CONFIG_FILE);
        let original = b"{broken existing config";
        fs::write(&path, original).unwrap();
        assert!(set_desktop_preferences_at(&path, &DesktopPreferences::default()).is_err());
        assert_eq!(fs::read(&path).unwrap(), original);
    }

    #[test]
    fn concurrent_desktop_localization_and_ordinary_writers_preserve_final_opt_outs() {
        use std::sync::{Arc, Barrier};
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join(CONFIG_FILE);
        let enabled = DesktopPreferences {
            launch_at_login: true,
            hide_dock_icon: true,
            start_minimized: true,
        };
        set_desktop_preferences_at(&path, &enabled).unwrap();
        set_localization_enabled_at(&path, true).unwrap();
        let mut stale = load_config_at(&path).unwrap();
        stale.theme = "dark".into();
        let start = Arc::new(Barrier::new(3));
        std::thread::scope(|scope| {
            let ordinary_path = path.clone();
            let ordinary_start = start.clone();
            scope.spawn(move || {
                ordinary_start.wait();
                for _ in 0..16 {
                    save_config_at(&ordinary_path, &stale).unwrap();
                }
            });
            let localization_path = path.clone();
            let localization_start = start.clone();
            scope.spawn(move || {
                localization_start.wait();
                for _ in 0..16 {
                    set_localization_enabled_at(&localization_path, true).unwrap();
                    set_localization_enabled_at(&localization_path, false).unwrap();
                }
            });
            start.wait();
            for _ in 0..16 {
                set_desktop_preferences_at(&path, &enabled).unwrap();
                set_desktop_preferences_at(&path, &DesktopPreferences::default()).unwrap();
            }
        });
        let actual = load_config_at(&path).unwrap();
        assert!(!actual.app_localization.enabled);
        assert!(!actual.desktop.launch_at_login);
        assert!(!actual.desktop.hide_dock_icon);
        assert!(!actual.desktop.start_minimized);
        assert_eq!(actual.theme, "dark");
    }
}
