//! WebUI 服务配置（web_config.json，独立于桌面版共享的 gui_config.json）
//!
//! 独立文件的原因：gui_config.json 与桌面版 Antigravity Tools Lite 共享，
//! 桌面版不认识 web 段，整文件回写会静默抹掉服务配置。
//!
//! 环境变量覆盖（headless 启动时持久化回写，修改密码的正规途径）：
//! - `ABV_WEB_PASSWORD` > `WEB_PASSWORD`：Web UI 登录密码
//! - `ABV_PORT` > `PORT`：服务端口
//! - `ABV_BIND_LOCAL_ONLY`：置 1 仅绑定 127.0.0.1

use std::fs;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use serde::{Deserialize, Serialize};

use crate::modules::account::get_data_dir;

const WEB_CONFIG_FILE: &str = "web_config.json";

static WEB_CONFIG_LOCK: Mutex<()> = Mutex::new(());

fn lock_web_config() -> Result<MutexGuard<'static, ()>, String> {
    WEB_CONFIG_LOCK
        .lock()
        .map_err(|_| "web_config_lock_unavailable".to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WebConfig {
    /// WebUI + /api 服务端口
    pub port: u16,
    /// Web UI 登录密码；为空时首启自动生成并打印到日志
    pub admin_password: Option<String>,
    /// 是否允许局域网访问（决定绑定 0.0.0.0 还是 127.0.0.1）
    pub allow_lan: bool,
}

impl Default for WebConfig {
    fn default() -> Self {
        Self {
            port: 8045,
            admin_password: None,
            allow_lan: false,
        }
    }
}

fn read_at(path: &Path) -> Result<Option<WebConfig>, String> {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("failed_to_read_web_config: {error}")),
    };
    serde_json::from_str(&content)
        .map(Some)
        .map_err(|error| format!("failed_to_parse_web_config: {error}"))
}

fn write_at(path: &Path, config: &WebConfig) -> Result<(), String> {
    let content = serde_json::to_string_pretty(config)
        .map_err(|error| format!("failed_to_serialize_web_config: {error}"))?;
    crate::utils::fs::write_atomic(path, content.as_bytes())
        .map_err(|error| format!("failed_to_save_web_config: {error}"))
}

pub fn web_config_path() -> Result<std::path::PathBuf, String> {
    Ok(get_data_dir()?.join(WEB_CONFIG_FILE))
}

/// 读取服务配置；文件缺失时写入默认值。
pub fn load_web_config() -> Result<WebConfig, String> {
    let _guard = lock_web_config()?;
    let path = web_config_path()?;
    if let Some(config) = read_at(&path)? {
        return Ok(config);
    }
    let config = WebConfig::default();
    let _ = write_at(&path, &config);
    Ok(config)
}

/// 持久化服务配置。
pub fn save_web_config(config: &WebConfig) -> Result<(), String> {
    let _guard = lock_web_config()?;
    write_at(&web_config_path()?, config)
}

/// headless 启动入口：应用环境变量覆盖并持久化，确保密码非空（为空则生成随机密码）。
/// 返回 (生效配置, 绑定地址)。
pub fn effective_service_config() -> Result<(WebConfig, String), String> {
    let mut config = load_web_config()?;
    let mut modified = false;

    let bind_local_only = std::env::var("ABV_BIND_LOCAL_ONLY")
        .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false);
    if bind_local_only && config.allow_lan {
        config.allow_lan = false;
        modified = true;
    }

    if let Ok(port) = std::env::var("ABV_PORT")
        .or_else(|_| std::env::var("PORT"))
    {
        if let Ok(port) = port.trim().parse::<u16>() {
            if port != config.port {
                config.port = port;
                modified = true;
            }
        }
    }

    let env_password = std::env::var("ABV_WEB_PASSWORD")
        .or_else(|_| std::env::var("WEB_PASSWORD"))
        .ok()
        .filter(|p| !p.trim().is_empty());
    if let Some(pwd) = env_password {
        config.admin_password = Some(pwd);
        modified = true;
    }

    // 服务强制鉴权：密码为空时生成随机密码，避免局域网裸奔
    let generated = match &config.admin_password {
        Some(pwd) if !pwd.is_empty() => None,
        _ => {
            let pwd = format!("abv-{}", uuid::Uuid::new_v4());
            config.admin_password = Some(pwd.clone());
            modified = true;
            Some(pwd)
        }
    };

    if modified {
        save_web_config(&config)?;
    }

    if let Some(pwd) = generated {
        tracing::info!("No Web UI password configured; generated one for this run: {}", pwd);
        tracing::info!("Set WEB_PASSWORD (or ABV_WEB_PASSWORD) to choose your own password.");
    }

    let bind_address = if config.allow_lan {
        "0.0.0.0".to_string()
    } else {
        "127.0.0.1".to_string()
    };

    Ok((config, bind_address))
}
