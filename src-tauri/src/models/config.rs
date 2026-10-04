use serde::{Deserialize, Serialize};

/// Application configuration for the account and quota dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub desktop: DesktopPreferences,
    pub menu_bar: MenuBarPreferences,
    pub language: String,
    pub theme: String,
    pub auto_refresh: bool,
    pub refresh_interval: i32,
    pub auto_sync: bool,
    pub sync_interval: i32,
    pub antigravity_executable: Option<String>,
    pub antigravity_ide_executable: Option<String>,
    pub antigravity_args: Option<Vec<String>>,
    pub quota_protection: QuotaProtectionConfig,
    pub pinned_quota_models: PinnedQuotaModelsConfig,
}

/// Preferences are opt-in and migrate safely from older config files.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DesktopPreferences {
    pub launch_at_login: bool,
    pub hide_dock_icon: bool,
    pub start_minimized: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MenuBarQuotaScope {
    #[default]
    All,
    Gemini,
    Other,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MenuBarPreferences {
    pub quota_scope: MenuBarQuotaScope,
}

/// Quota protection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaProtectionConfig {
    /// Whether quota protection is enabled
    pub enabled: bool,

    /// Reserved quota percentage (1-99)
    pub threshold_percentage: u32,

    /// List of monitored models (e.g. gemini-3-flash, gemini-3-pro-high, gemini-3.1-pro-high, claude-sonnet-4-6)
    #[serde(default = "default_monitored_models")]
    pub monitored_models: Vec<String>,
}

fn default_monitored_models() -> Vec<String> {
    vec![
        "claude".to_string(),
        "gemini-3-pro-high".to_string(),
        "gemini-3-flash".to_string(),
        "gemini-3.1-flash-image".to_string(),
    ]
}

impl QuotaProtectionConfig {
    pub fn new() -> Self {
        Self {
            enabled: false,
            threshold_percentage: 10, // Default 10% reserve
            monitored_models: default_monitored_models(),
        }
    }
}

impl Default for QuotaProtectionConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Pinned quota models configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinnedQuotaModelsConfig {
    /// List of pinned models (displayed outside the account list)
    #[serde(default = "default_pinned_models")]
    pub models: Vec<String>,
}

fn default_pinned_models() -> Vec<String> {
    vec![
        "gemini-3-pro-high".to_string(),
        "gemini-3-flash".to_string(),
        "gemini-3.1-flash-image".to_string(),
        "claude-sonnet-4-6-thinking".to_string(),
    ]
}

impl PinnedQuotaModelsConfig {
    pub fn new() -> Self {
        Self {
            models: default_pinned_models(),
        }
    }
}

impl Default for PinnedQuotaModelsConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl AppConfig {
    pub fn new() -> Self {
        Self {
            desktop: DesktopPreferences::default(),
            menu_bar: MenuBarPreferences::default(),
            language: crate::modules::i18n::default_language(),
            theme: "system".to_string(),
            auto_refresh: true,
            refresh_interval: 15,
            auto_sync: false,
            sync_interval: 5,
            antigravity_executable: None,
            antigravity_ide_executable: None,
            antigravity_args: None,
            quota_protection: QuotaProtectionConfig::default(),
            pinned_quota_models: PinnedQuotaModelsConfig::default(),
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::AppConfig;

    #[test]
    fn desktop_preferences_are_opt_in_for_new_and_legacy_configs() {
        for config in [
            AppConfig::new(),
            serde_json::from_str::<AppConfig>(r#"{"language":"zh"}"#).unwrap(),
        ] {
            assert!(!config.desktop.launch_at_login);
            assert!(!config.desktop.hide_dock_icon);
            assert!(!config.desktop.start_minimized);
        }
    }

    #[test]
    fn partial_desktop_config_keeps_new_fields_disabled() {
        let config: AppConfig =
            serde_json::from_str(r#"{"desktop":{"hide_dock_icon":true}}"#).unwrap();
        assert!(config.desktop.hide_dock_icon);
        assert!(!config.desktop.launch_at_login);
        assert!(!config.desktop.start_minimized);
    }

    #[test]
    fn removed_localization_preference_is_ignored_in_legacy_config() {
        let saved: AppConfig =
            serde_json::from_str(r#"{"language":"en","app_localization":{"enabled":true},"desktop":{"launch_at_login":true}}"#)
                .unwrap();
        assert_eq!(saved.language, "en");
        assert!(saved.desktop.launch_at_login);
        assert!(serde_json::to_value(saved).unwrap().get("app_localization").is_none());
    }

    #[test]
    fn saved_language_is_preserved_when_loading_config() {
        let mut config = AppConfig::new();
        for language in ["en", "zh"] {
            config.language = language.to_string();
            let saved = serde_json::to_string(&config).unwrap();
            let restored: AppConfig = serde_json::from_str(&saved).unwrap();
            assert_eq!(restored.language, language);
        }
    }
}
