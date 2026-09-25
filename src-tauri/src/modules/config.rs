use std::fs;

use super::account::get_data_dir;
use crate::models::AppConfig;

const CONFIG_FILE: &str = "gui_config.json";

/// Load application configuration
pub fn load_app_config() -> Result<AppConfig, String> {
    let data_dir = get_data_dir()?;
    let config_path = data_dir.join(CONFIG_FILE);

    if !config_path.exists() {
        let config = AppConfig::new();
        // [FIX #1460] Persist initial config to prevent new API Key on every refresh
        let _ = save_app_config(&config);
        return Ok(config);
    }

    let content = fs::read_to_string(&config_path)
        .map_err(|e| format!("failed_to_read_config_file: {}", e))?;

    serde_json::from_str(&content).map_err(|e| format!("failed_to_parse_config_file: {}", e))
}

/// Save application configuration (atomic write)
pub fn save_app_config(config: &AppConfig) -> Result<(), String> {
    let data_dir = get_data_dir()?;
    let config_path = data_dir.join(CONFIG_FILE);

    let content = serde_json::to_string_pretty(config)
        .map_err(|e| format!("failed_to_serialize_config: {}", e))?;

    crate::utils::fs::write_atomic(&config_path, content.as_bytes())
        .map_err(|e| format!("failed_to_save_config: {}", e))
}
