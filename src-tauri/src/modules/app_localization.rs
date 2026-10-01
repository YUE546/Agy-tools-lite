//! Experimental App localization preflight. No live adapter is enabled in this
//! build: source-derived selectors still require real-App validation. The three
//! commands deliberately do not open sockets, start processes or inject scripts.

use serde::Serialize;
use serde_json::Value;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

const DICTIONARY: &str = include_str!("../../resources/app-localization/zh-CN.json");
const MAX_ASAR_HEADER: usize = 4 * 1024 * 1024;
const MAX_PACKAGE: usize = 16 * 1024;
const UNVERIFIED: &str = "unverified_adapter";

#[derive(Debug, Serialize)]
pub struct LocalizationStatus {
    enabled: bool,
    state: &'static str,
    installed_version: Option<String>,
    dictionary_version: String,
    dictionary_entries: usize,
    supported_versions: Vec<String>,
    can_apply: bool,
    active: bool,
    detail: Option<&'static str>,
}

/// Empty until a release's actual DOM and lifecycle have passed live QA.
/// A dictionary's claimed version is not evidence that its DOM adapter is safe.
fn supported_versions() -> &'static [&'static str] {
    &[]
}

fn package_version(package: &[u8]) -> Option<String> {
    let package: Value = serde_json::from_slice(package).ok()?;
    // Antigravity IDE also uses the Antigravity name. Require the standalone
    // App's package identity; never run --version or execute a selected binary.
    if package.get("name")?.as_str()? != "antigravity"
        || package.get("productName")?.as_str()? != "Antigravity"
        || package.get("description")?.as_str()? != "Antigravity - Agentic Desktop Application"
    {
        return None;
    }
    let version = package.get("version")?.as_str()?;
    if version.len() > 32
        || version.split('.').count() != 3
        || !version.split('.').all(|part| {
            !part.is_empty() && part.len() <= 8 && part.bytes().all(|b| b.is_ascii_digit())
        })
    {
        return None;
    }
    Some(version.to_string())
}

/// Read only the bounded package.json entry from an ASAR; no extraction or code
/// execution. Reject links, unpacked entries, oversized values and bad offsets.
fn read_asar_version(path: &Path) -> Option<String> {
    let mut file = File::open(path).ok()?;
    let file_size = file.metadata().ok()?.len();
    let mut prefix = [0_u8; 16];
    file.read_exact(&mut prefix).ok()?;
    let number =
        |start: usize| u32::from_le_bytes(prefix[start..start + 4].try_into().unwrap()) as usize;
    let header_size = number(4);
    let json_size = number(12);
    if number(0) != 4
        || json_size == 0
        || header_size > MAX_ASAR_HEADER
        || json_size > header_size.checked_sub(8)?
        || number(8) > header_size
    {
        return None;
    }
    let mut header = vec![0; json_size];
    file.read_exact(&mut header).ok()?;
    let header: Value = serde_json::from_slice(&header).ok()?;
    let entry = header.get("files")?.get("package.json")?;
    if entry.get("link").is_some()
        || entry
            .get("unpacked")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    {
        return None;
    }
    let offset = entry.get("offset")?.as_str()?.parse::<u64>().ok()?;
    let size = entry.get("size")?.as_u64()?;
    if size == 0 || size > MAX_PACKAGE as u64 {
        return None;
    }
    let position = (8_u64)
        .checked_add(header_size as u64)?
        .checked_add(offset)?;
    if position.checked_add(size)? > file_size {
        return None;
    }
    file.seek(SeekFrom::Start(position)).ok()?;
    let mut package = vec![0; size as usize];
    file.read_exact(&mut package).ok()?;
    package_version(&package)
}

fn installation_asar(executable: &Path) -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let mut path = executable;
        loop {
            if path.extension().is_some_and(|ext| ext == "app") {
                return Some(path.join("Contents/Resources/app.asar"));
            }
            path = path.parent()?;
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let executable = executable.canonicalize().ok()?;
        Some(executable.parent()?.join("resources/app.asar"))
    }
}

fn status_for_installation(enabled: bool, executable: Option<&Path>) -> LocalizationStatus {
    let dictionary: Value = serde_json::from_str(DICTIONARY).expect("bundled dictionary is JSON");
    let installed_version = executable
        .and_then(installation_asar)
        .and_then(|p| read_asar_version(&p));
    LocalizationStatus {
        enabled,
        state: if executable.is_none() {
            "not_installed"
        } else {
            "needs_verification"
        },
        detail: Some(if executable.is_some() && installed_version.is_none() {
            "unknown_installation"
        } else {
            UNVERIFIED
        }),
        installed_version,
        dictionary_version: dictionary["version"]
            .as_str()
            .unwrap_or("unknown")
            .to_string(),
        dictionary_entries: dictionary["exact"].as_object().map_or(0, |d| d.len()),
        supported_versions: supported_versions().iter().map(|v| v.to_string()).collect(),
        can_apply: false,
        active: false,
    }
}

fn current_status() -> Result<LocalizationStatus, String> {
    let config = super::config::load_app_config()?;
    let executable = super::process::get_antigravity_executable_path(None);
    Ok(status_for_installation(
        config.app_localization.enabled,
        executable.as_deref(),
    ))
}

#[tauri::command]
pub async fn get_app_localization_status() -> Result<LocalizationStatus, String> {
    tokio::task::spawn_blocking(current_status)
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn set_app_localization_enabled(enabled: bool) -> Result<LocalizationStatus, String> {
    // Server-side guard, independent of disabled UI controls and saved values.
    if enabled {
        return Err(UNVERIFIED.to_string());
    }
    tokio::task::spawn_blocking(|| {
        let mut config = super::config::load_app_config()?;
        config.app_localization.enabled = false;
        super::config::save_app_config(&config)?;
        current_status()
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn apply_app_localization(launch: bool) -> Result<LocalizationStatus, String> {
    let _ = launch;
    Err(UNVERIFIED.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn official_package(version: &str) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "name":"antigravity", "productName":"Antigravity",
            "description":"Antigravity - Agentic Desktop Application", "version":version
        }))
        .unwrap()
    }

    #[test]
    fn reads_only_standalone_app_identity() {
        assert_eq!(
            package_version(&official_package("2.19.1")),
            Some("2.19.1".into())
        );
        for version in ["", "2.19", "2.19.1-extra", "2.19.1.1", "a.b.c"] {
            assert_eq!(package_version(&official_package(version)), None);
        }
        assert_eq!(
            package_version(br#"{"name":"antigravity","version":"2.19.1"}"#),
            None
        );
        assert_eq!(package_version(b"not JSON"), None);
    }

    #[test]
    fn wip_release_cannot_enable_or_apply_even_with_saved_opt_in() {
        assert!(supported_versions().is_empty());
        for enabled in [true, false] {
            let status = status_for_installation(enabled, None);
            assert!(!status.can_apply);
            assert!(!status.active);
            assert_eq!(status.state, "not_installed");
            assert!(status.dictionary_entries > 0);
        }
    }

    #[test]
    fn dictionary_is_only_reviewed_exact_plain_text() {
        let d: Value = serde_json::from_str(DICTIONARY).unwrap();
        let map = d["exact"].as_object().unwrap();
        assert_eq!(d.as_object().unwrap().len(), 2);
        for (key, value) in map {
            assert!(!key.is_empty() && key.len() < 100);
            let value = value.as_str().unwrap();
            assert!(!value.is_empty() && value.len() < 150);
            assert!(!value.contains('<') && !value.contains('>'));
            assert!(!value.contains("http") && !value.contains('\n'));
        }
    }

    #[test]
    fn malformed_asar_is_rejected_without_executing_anything() {
        let root = std::env::temp_dir().join(format!("atl-asar-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("app.asar");
        for bytes in [vec![], vec![0; 16], vec![255; 16]] {
            std::fs::write(&path, bytes).unwrap();
            assert_eq!(read_asar_version(&path), None);
        }
        let package = official_package("2.19.1");
        let header = serde_json::to_vec(&serde_json::json!({"files":{"package.json":{
            "size":package.len(), "offset":"0"
        }}}))
        .unwrap();
        let mut bytes = Vec::new();
        for n in [
            4,
            header.len() as u32 + 8,
            header.len() as u32 + 4,
            header.len() as u32,
        ] {
            bytes.extend_from_slice(&n.to_le_bytes());
        }
        bytes.extend_from_slice(&header);
        bytes.extend_from_slice(&package);
        std::fs::write(&path, &bytes).unwrap();
        assert_eq!(read_asar_version(&path), Some("2.19.1".to_string()));
        bytes.truncate(bytes.len() - 1);
        std::fs::write(&path, bytes).unwrap();
        assert_eq!(read_asar_version(&path), None);
        std::fs::remove_dir_all(root).unwrap();
    }
}
