use crate::modules::{cli_credentials, db, device, process, version};
use std::fs;
#[cfg(target_os = "macos")]
use std::process::Command;

pub trait SystemIntegration: Send + Sync {
    /// 当切换账号时执行的系统层操作（如杀进程、写入文件、注入数据库）
    async fn on_account_switch(
        &self,
        account: &crate::models::Account,
        target_ide: Option<&str>,
    ) -> Result<(), String>;

    /// 更新系统托盘（如果适用）
    fn update_tray(&self);

    /// 发送系统通知
    fn show_notification(&self, title: &str, body: &str);
}

/// 桌面版实现：包含完整的进程控制 and UI 同步
pub struct DesktopIntegration {
    pub app_handle: tauri::AppHandle,
}

impl SystemIntegration for DesktopIntegration {
    async fn on_account_switch(
        &self,
        account: &crate::models::Account,
        target_ide: Option<&str>,
    ) -> Result<(), String> {
        let integration = DesktopIntegration {
            app_handle: self.app_handle.clone(),
        };
        let account = account.clone();
        let target = target_ide.map(str::to_owned);
        tokio::task::spawn_blocking(move || integration.switch_sync(&account, target.as_deref()))
            .await
            .map_err(|_| "Account switch worker failed.".to_string())?
    }

    fn update_tray(&self) {
        let _ = crate::modules::tray::update_tray_menus(&self.app_handle);
    }

    fn show_notification(&self, title: &str, body: &str) {
        // 使用 tauri-plugin-dialog 或原生通知（此处简化）
        crate::modules::logger::log_info(&format!("[Notification] {}: {}", title, body));
    }
}

impl DesktopIntegration {
    fn switch_sync(
        &self,
        account: &crate::models::Account,
        target_ide: Option<&str>,
    ) -> Result<(), String> {
        crate::modules::logger::log_info(&format!(
            "[Desktop] Executing system switch for: {} (target_ide: {:?})",
            account.email, target_ide
        ));

        #[cfg(target_os = "linux")]
        let cli_only = target_ide == Some("agy")
            || (target_ide.is_none()
                && process::get_antigravity_executable_path(target_ide).is_none()
                && crate::modules::linux_paths::find_executable("agy").is_some()
                && cli_session_path()?.is_some());
        #[cfg(not(target_os = "linux"))]
        let cli_only = target_ide == Some("agy");

        if cli_only {
            write_to_system_keyring(account, true)?;

            if let Ok(storage_path) = device::get_storage_path(target_ide) {
                if let Some(ref profile) = account.device_profile {
                    let _ = device::write_profile(&storage_path, profile);
                }
            }

            let is_running = process::is_process_running_by_name("agy");
            let msg = if is_running {
                format!(
                    "Account {} activated. Agy is running; start a new CLI command to use the updated session.",
                    account.email
                )
            } else {
                format!(
                    "Account {} activated. Token is ready for your next CLI command.",
                    account.email
                )
            };
            self.show_notification("Antigravity CLI", &msg);
            self.update_tray();

            return Ok(());
        }

        #[cfg(target_os = "linux")]
        if process::get_antigravity_executable_path(target_ide).is_none() {
            return Err("Cannot find Antigravity. Set its executable path in Settings, or install and initialize agy before switching accounts.".into());
        }

        // 1. 先关闭外部正在运行的进程（无论是原生还是IDE，先安全关闭，避免文件或凭据冲突）
        if process::is_antigravity_running(target_ide) {
            process::close_antigravity(20, target_ide)?;
        }

        // 2. 智能决策：是否使用最新的系统 Keychain 凭据管理器方式存储 Token
        let mut is_ide = target_ide == Some("ide");

        // Auto-detect IDE: if the located executable is the IDE, treat as IDE mode
        if !is_ide {
            if let Some(exe_path) = process::get_antigravity_executable_path(target_ide) {
                let path_lower = exe_path.to_string_lossy().to_lowercase();
                if path_lower.contains("antigravity ide") || path_lower.contains("antigravity-ide")
                {
                    is_ide = true;
                    crate::modules::logger::log_info(
                        "[Desktop] Auto-detected Antigravity IDE executable, using IDE account switch logic.",
                    );
                }
            }
        }

        let mut use_keyring = false;

        if !is_ide {
            // 经典原生版：自动探测版本号
            match version::get_antigravity_version(target_ide) {
                Ok(ver) => {
                    // 如果版本号 >= 2.0.0
                    if version::compare_version(&ver.short_version, "2.0.0")
                        != std::cmp::Ordering::Less
                    {
                        use_keyring = true;
                        crate::modules::logger::log_info(&format!(
                            "[Desktop] Detected Antigravity version {} >= 2.0.0, using system Keyring.",
                            ver.short_version
                        ));
                    } else {
                        crate::modules::logger::log_info(&format!(
                            "[Desktop] Detected Antigravity version {} < 2.0.0, falling back to legacy SQLite injection.",
                            ver.short_version
                        ));
                    }
                }
                Err(e) => {
                    // 如果探测失败，为防止对最新版由于没有 storage.json 造成报错阻断，默认作为新凭据注入
                    use_keyring = true;
                    crate::modules::logger::log_warn(&format!(
                        "[Desktop] Failed to detect Antigravity version ({}), defaulting to system Keyring for robustness.",
                        e
                    ));
                }
            }
        }

        if use_keyring {
            // ================== 最新版 Antigravity 原生应用逻辑 (>= 2.0.0) ==================
            // 2.1 写入系统 Keychain/Keyring
            write_to_system_keyring(account, false)?;

            // 2.2 原生应用可能没有 storage.json，但如果有的话，我们也可以尝试安全地写入设备 Profile，以兼容指纹信息
            if let Ok(storage_path) = device::get_storage_path(target_ide) {
                if let Some(ref profile) = account.device_profile {
                    let _ = device::write_profile(&storage_path, profile);
                }
            }
        } else {
            // ================== 原有 Antigravity 旧版或定制 IDE 逻辑 (< 2.0.0) ==================
            // 2.1 获取存储路径
            let storage_path = device::get_storage_path(target_ide)?;

            // 2.2 写入设备 Profile
            if let Some(ref profile) = account.device_profile {
                device::write_profile(&storage_path, profile)?;
            }

            // 2.3 数据库处理与 Token 注入
            let db_path = db::get_db_path(target_ide)?;
            if db_path.exists() {
                let backup_path = db_path.with_extension("vscdb.backup");
                let _ = fs::copy(&db_path, &backup_path);
            }

            db::inject_token(
                &db_path,
                &account.token.access_token,
                &account.token.refresh_token,
                account.token.expiry_timestamp,
                &account.email,
                account.token.is_gcp_tos,
                account.token.project_id.as_deref(),
                account.token.id_token.as_deref(),
                account.token.oauth_client_key.as_deref(),
            )?;

            // Legacy native APPs still share the initialized agy session. IDE
            // switches keep their existing, independent SQLite-only behavior.
            if !is_ide {
                let payload = cli_credentials::payload(&account.token)?;
                cli_session_path()
                    .and_then(|path| {
                        cli_credentials::sync_session(path.as_deref(), &payload, false)
                    })
                    .map_err(|error| {
                        format!("APP database was updated, but agy session sync failed: {error}")
                    })?;
            }
        }

        // 3. 重启外部进程
        process::start_antigravity(target_ide)?;

        // 4. 更新托盘
        let _ = crate::modules::tray::update_tray_menus(&self.app_handle);

        Ok(())
    }
}

fn cli_session_path() -> Result<Option<std::path::PathBuf>, String> {
    let home = dirs::home_dir().ok_or("Failed to resolve user home directory")?;
    cli_credentials::session_path(&home)
}

pub fn read_cli_credentials() -> Result<crate::modules::migration::ImportedOAuthState, String> {
    let path = cli_session_path()?.ok_or("No initialized agy CLI session found.")?;
    let payload = std::fs::read_to_string(path)
        .map_err(|error| format!("Cannot read agy session: {error}"))?;
    parse_keyring_payload(&payload)
}

/// Sync the APP's platform credential store and an existing native agy session.
fn write_to_system_keyring(account: &crate::models::Account, cli_only: bool) -> Result<(), String> {
    let payload_json = cli_credentials::payload(&account.token)?;
    let cli_path = cli_session_path()?;

    // Explicit CLI switches do not need or modify the APP credential store.
    // Require initialization so a missing session cannot be reported as success.
    if cli_only {
        return cli_credentials::sync_session(cli_path.as_deref(), &payload_json, true);
    }

    crate::modules::logger::log_info(&format!(
        "[Desktop] Writing token to system credential store for: {}",
        account.email
    ));

    // 2. 跨平台凭据注入
    #[cfg(target_os = "macos")]
    {
        use base64::{engine::general_purpose::STANDARD, Engine as _};
        let encoded_payload = STANDARD.encode(&payload_json);
        let full_keyring_value = format!("go-keyring-base64:{}", encoded_payload);

        // 2.1 macOS Keychain Access
        // 删除旧的
        let _ = Command::new("security")
            .args([
                "delete-generic-password",
                "-s",
                "gemini",
                "-a",
                "antigravity",
            ])
            .output();

        // 写入新的 (-A 参数允许所有本地应用免密码、无感直接读取凭据)
        let output = Command::new("security")
            .args([
                "add-generic-password",
                "-s",
                "gemini",
                "-a",
                "antigravity",
                "-w",
                &full_keyring_value,
                "-A",
            ])
            .output()
            .map_err(|e| format!("Failed to execute security command: {}", e))?;

        if !output.status.success() {
            let err_msg = String::from_utf8_lossy(&output.stderr);
            return Err(format!("macOS security command failed: {}", err_msg.trim()));
        }
    }

    #[cfg(target_os = "windows")]
    {
        // 2.2 Windows Credential Manager direct Win32 API calls to write raw UTF-8 bytes
        use std::os::windows::ffi::OsStrExt;
        use std::ptr;

        #[repr(C)]
        struct FILETIME {
            dw_low_date_time: u32,
            dw_high_date_time: u32,
        }

        #[repr(C)]
        struct CREDENTIALW {
            flags: u32,
            cred_type: u32,
            target_name: *const u16,
            comment: *const u16,
            last_written: FILETIME,
            credential_blob_size: u32,
            credential_blob: *const u8,
            persist: u32,
            attribute_count: u32,
            attributes: *const std::ffi::c_void,
            target_alias: *const u16,
            user_name: *const u16,
        }

        #[link(name = "advapi32")]
        extern "system" {
            fn CredWriteW(credential: *const CREDENTIALW, flags: u32) -> i32;
            fn CredDeleteW(target_name: *const u16, type_: u32, flags: u32) -> i32;
        }

        let target = "gemini:antigravity";
        let user = "antigravity";
        let secret = payload_json.as_bytes();

        let target_wide: Vec<u16> = std::ffi::OsStr::new(target)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let user_wide: Vec<u16> = std::ffi::OsStr::new(user)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let cred = CREDENTIALW {
            flags: 0,
            cred_type: 1, // CRED_TYPE_GENERIC
            target_name: target_wide.as_ptr(),
            comment: ptr::null(),
            last_written: FILETIME {
                dw_low_date_time: 0,
                dw_high_date_time: 0,
            },
            credential_blob_size: secret.len() as u32,
            credential_blob: secret.as_ptr(),
            persist: 2, // CRED_PERSIST_LOCAL_MACHINE
            attribute_count: 0,
            attributes: ptr::null(),
            target_alias: ptr::null(),
            user_name: user_wide.as_ptr(),
        };

        unsafe {
            // Delete first to ensure we write clean
            let _ = CredDeleteW(target_wide.as_ptr(), 1, 0);

            let res = CredWriteW(&cred, 0);
            if res == 0 {
                let err = std::io::Error::last_os_error();
                return Err(format!("Windows CredWriteW failed: {}", err));
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(path) = cli_path {
            let cli_payload = payload_json.clone();
            crate::modules::linux_credentials::write_with_commit(&payload_json, move || {
                cli_credentials::write_session(&path, &cli_payload)
            })?;
        } else {
            crate::modules::linux_credentials::write(&payload_json)?;
        }
    }

    #[cfg(not(target_os = "linux"))]
    cli_credentials::sync_session(cli_path.as_deref(), &payload_json, false).map_err(|error| {
        format!("System credential store was updated, but agy session sync failed: {error}")
    })?;

    crate::modules::logger::log_info(
        "[Desktop] Successfully wrote token to system credential store.",
    );

    Ok(())
}

/// 辅助方法：从宿主操作系统的 Keychain/Credentials Manager 读取 Token
pub fn read_from_system_keyring() -> Result<crate::modules::migration::ImportedOAuthState, String> {
    #[cfg(target_os = "macos")]
    {
        use base64::{engine::general_purpose::STANDARD, Engine as _};
        let output = Command::new("security")
            .args([
                "find-generic-password",
                "-s",
                "gemini",
                "-a",
                "antigravity",
                "-w",
            ])
            .output()
            .map_err(|e| format!("Failed to execute security command: {}", e))?;

        if !output.status.success() {
            return Err("No credential found in macOS Keychain".to_string());
        }

        let secret_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let payload_str = if secret_str.starts_with("go-keyring-base64:") {
            let b64_part = &secret_str["go-keyring-base64:".len()..];
            let decoded = STANDARD
                .decode(b64_part)
                .map_err(|e| format!("Base64 decode failed: {}", e))?;
            String::from_utf8(decoded).map_err(|e| format!("UTF-8 decode failed: {}", e))?
        } else {
            secret_str
        };

        return parse_keyring_payload(&payload_str);
    }

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::ffi::OsStrExt;
        use std::ptr;

        #[repr(C)]
        struct FILETIME {
            dw_low_date_time: u32,
            dw_high_date_time: u32,
        }

        #[repr(C)]
        struct CREDENTIALW {
            flags: u32,
            cred_type: u32,
            target_name: *const u16,
            comment: *const u16,
            last_written: FILETIME,
            credential_blob_size: u32,
            credential_blob: *mut u8,
            persist: u32,
            attribute_count: u32,
            attributes: *const std::ffi::c_void,
            target_alias: *const u16,
            user_name: *const u16,
        }

        #[link(name = "advapi32")]
        extern "system" {
            fn CredReadW(
                target_name: *const u16,
                type_: u32,
                flags: u32,
                credential: *mut *mut CREDENTIALW,
            ) -> i32;
            fn CredFree(buffer: *mut std::ffi::c_void);
        }

        let target = "gemini:antigravity";
        let target_wide: Vec<u16> = std::ffi::OsStr::new(target)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let mut cred_ptr: *mut CREDENTIALW = ptr::null_mut();
        unsafe {
            let res = CredReadW(target_wide.as_ptr(), 1, 0, &mut cred_ptr);
            if res == 0 || cred_ptr.is_null() {
                return Err("No credential found in Windows Credential Manager".to_string());
            }

            let cred = &*cred_ptr;
            let blob = std::slice::from_raw_parts(
                cred.credential_blob,
                cred.credential_blob_size as usize,
            );
            let payload_str = String::from_utf8_lossy(blob).to_string();
            CredFree(cred_ptr as *mut std::ffi::c_void);

            return parse_keyring_payload(&payload_str);
        }
    }

    #[cfg(target_os = "linux")]
    {
        return parse_keyring_payload(&crate::modules::linux_credentials::read()?);
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        Err("Keyring not supported on this operating system".to_string())
    }
}

fn parse_keyring_payload(
    payload_str: &str,
) -> Result<crate::modules::migration::ImportedOAuthState, String> {
    let json: serde_json::Value = serde_json::from_str(payload_str)
        .map_err(|e| format!("Failed to parse keyring payload JSON: {}", e))?;

    let refresh_token = json
        .get("token")
        .and_then(|t| t.get("refresh_token"))
        .and_then(|v| v.as_str())
        .or_else(|| json.get("refresh_token").and_then(|v| v.as_str()))
        .ok_or_else(|| "Refresh Token not found in keyring payload".to_string())?
        .to_string();

    Ok(crate::modules::migration::ImportedOAuthState {
        refresh_token,
        is_gcp_tos: true,
        project_id: None,
    })
}

/// Desktop integration manager used by account and OAuth services.
#[derive(Clone)]
pub struct SystemManager {
    pub app_handle: tauri::AppHandle,
}

impl SystemManager {
    pub fn new(app_handle: tauri::AppHandle) -> Self {
        Self { app_handle }
    }

    pub fn update_tray(&self) {
        DesktopIntegration {
            app_handle: self.app_handle.clone(),
        }
        .update_tray();
    }
}

impl SystemIntegration for SystemManager {
    async fn on_account_switch(
        &self,
        account: &crate::models::Account,
        target_ide: Option<&str>,
    ) -> Result<(), String> {
        DesktopIntegration {
            app_handle: self.app_handle.clone(),
        }
        .on_account_switch(account, target_ide)
        .await
    }

    fn update_tray(&self) {
        SystemManager::update_tray(self);
    }

    fn show_notification(&self, title: &str, body: &str) {
        DesktopIntegration {
            app_handle: self.app_handle.clone(),
        }
        .show_notification(title, body);
    }
}

#[cfg(all(test, target_os = "linux"))]
mod linux_tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    #[ignore = "requires a disposable D-Bus/keyring session"]
    fn isolated_linux_app_and_cli_sync() {
        assert_eq!(
            std::env::var("ANTIGRAVITY_TEST_SECRET_SERVICE").as_deref(),
            Ok("isolated")
        );
        let home = dirs::home_dir().unwrap();
        std::fs::create_dir_all(home.join(".gemini/antigravity-cli")).unwrap();
        let path = cli_session_path().unwrap().unwrap();
        let mut account = crate::models::Account::new(
            "fixture".into(),
            "fixture@example.invalid".into(),
            crate::models::TokenData::new(
                "fixture-access".into(),
                "fixture-app".into(),
                3600,
                None,
                None,
                None,
                true,
                Some("fixture-id".into()),
            ),
        );
        write_to_system_keyring(&account, false).unwrap();
        let app_payload = crate::modules::linux_credentials::read().unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), app_payload);

        // An explicit CLI switch must leave the APP's active credential alone.
        account.token.refresh_token = "fixture-cli-only".into();
        write_to_system_keyring(&account, true).unwrap();
        assert_eq!(
            read_cli_credentials().unwrap().refresh_token,
            "fixture-cli-only"
        );
        assert_eq!(
            crate::modules::linux_credentials::read().unwrap(),
            app_payload
        );

        // Force the real APP+CLI commit callback to fail and verify rollback.
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        let failure = write_to_system_keyring(&account, false).unwrap_err();
        assert!(failure.contains("Previous credentials were restored"));
        assert_eq!(
            crate::modules::linux_credentials::read().unwrap(),
            app_payload
        );
        assert!(path.is_dir());
        std::fs::remove_dir(&path).unwrap();
    }

    #[test]
    #[ignore = "requires a disposable HOME; run with --test-threads=1"]
    fn isolated_linux_cli_switch() {
        assert_eq!(
            std::env::var("ANTIGRAVITY_TEST_SECRET_SERVICE").as_deref(),
            Ok("isolated")
        );
        let home = dirs::home_dir().unwrap();
        let bin = home.join(".local/bin");
        let session = home.join(".gemini/antigravity-cli");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::create_dir_all(&session).unwrap();
        let executable = bin.join("agy");
        std::fs::write(&executable, b"#!/bin/sh\nexit 0\n").unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
        let unrelated = home.join(".gemini/oauth_creds.json");
        let unrelated_accounts = home.join(".gemini/google_accounts.json");
        std::fs::write(&unrelated, b"unrelated-gemini-fixture").unwrap();
        std::fs::write(&unrelated_accounts, b"unrelated-accounts-fixture").unwrap();
        let mut account = crate::models::Account::new(
            "fixture".into(),
            "fixture@example.invalid".into(),
            crate::models::TokenData::new(
                "fixture-access".into(),
                "fixture-old".into(),
                3600,
                None,
                None,
                None,
                true,
                None,
            ),
        );
        write_to_system_keyring(&account, true).unwrap();
        assert_eq!(read_cli_credentials().unwrap().refresh_token, "fixture-old");
        account.token.refresh_token = "fixture-new".into();
        write_to_system_keyring(&account, true).unwrap();
        assert_eq!(read_cli_credentials().unwrap().refresh_token, "fixture-new");
        let metadata = std::fs::metadata(session.join("antigravity-oauth-token")).unwrap();
        assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
        assert_eq!(
            std::fs::read(unrelated).unwrap(),
            b"unrelated-gemini-fixture"
        );
        assert_eq!(
            std::fs::read(unrelated_accounts).unwrap(),
            b"unrelated-accounts-fixture"
        );
    }
}
