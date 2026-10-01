//! Native Antigravity CLI (agy) credentials shared with the APP keyring payload.
//! Generic Google Gemini CLI files are deliberately outside this module's scope.
use crate::models::TokenData;
use std::path::{Path, PathBuf};

/// An existing directory is evidence that agy has already been initialized.
/// Do not create a CLI installation or depend on the GUI application's PATH.
pub fn session_path(home: &Path) -> Result<Option<PathBuf>, String> {
    let directory = home.join(".gemini").join("antigravity-cli");
    match std::fs::metadata(&directory) {
        Ok(metadata) if metadata.is_dir() => Ok(Some(directory.join("antigravity-oauth-token"))),
        Ok(_) => Err("The agy session location must be a directory.".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("Cannot inspect agy session directory: {error}")),
    }
}

/// Keep the APP and agy on exactly the same native token representation.
pub fn payload(token: &TokenData) -> Result<String, String> {
    let expiry_datetime = chrono::DateTime::from_timestamp(token.expiry_timestamp, 0)
        .unwrap_or_else(|| chrono::Utc::now());

    #[derive(serde::Serialize)]
    struct TokenDetails<'a> {
        access_token: &'a str,
        token_type: &'a str,
        refresh_token: &'a str,
        expiry: String,
    }

    #[derive(serde::Serialize)]
    struct Payload<'a> {
        token: TokenDetails<'a>,
        auth_method: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        id_token: Option<&'a str>,
    }

    serde_json::to_string(&Payload {
        token: TokenDetails {
            access_token: &token.access_token,
            token_type: "Bearer",
            refresh_token: &token.refresh_token,
            expiry: expiry_datetime.to_rfc3339_opts(chrono::SecondsFormat::Micros, true),
        },
        auth_method: "consumer",
        id_token: token.id_token.as_deref(),
    })
    .map_err(|error| format!("Failed to serialize Antigravity credential JSON: {error}"))
}

pub fn write_session(path: &Path, payload: &str) -> Result<(), String> {
    // Atomic replacement cannot faithfully restore a symlink or another special
    // file, so reject those before snapshotting or changing any session bytes.
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if !metadata.file_type().is_file() => {
            return Err(
                "The agy session must be a regular file, not a symlink or directory.".into(),
            );
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("Cannot inspect agy session: {error}")),
    }
    crate::utils::fs::write_atomic_verified(path, payload.as_bytes())
}

pub fn sync_session(path: Option<&Path>, payload: &str, required: bool) -> Result<(), String> {
    match path {
        Some(path) => write_session(path, payload),
        None if required => Err(
            "No initialized agy CLI session found. Initialize agy before switching its account."
                .into(),
        ),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn fixture_token() -> TokenData {
        let mut token = TokenData::new(
            "fixture-access".into(),
            "fixture-refresh".into(),
            3600,
            None,
            None,
            None,
            true,
            Some("fixture-id".into()),
        );
        token.expiry_timestamp = 1_700_000_000;
        token
    }

    #[test]
    fn native_payload_matches_app_contract() {
        let mut token = fixture_token();
        let json: serde_json::Value = serde_json::from_str(&payload(&token).unwrap()).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "token": {
                    "access_token": "fixture-access",
                    "token_type": "Bearer",
                    "refresh_token": "fixture-refresh",
                    "expiry": "2023-11-14T22:13:20.000000Z"
                },
                "auth_method": "consumer",
                "id_token": "fixture-id"
            })
        );
        token.id_token = None;
        let json: serde_json::Value = serde_json::from_str(&payload(&token).unwrap()).unwrap();
        assert!(json.get("id_token").is_none());
    }

    #[test]
    fn uninitialized_cli_is_skipped_or_rejected_without_creating_files() {
        let home = tempfile::tempdir().unwrap();
        let path = session_path(home.path()).unwrap();
        assert!(path.is_none());
        sync_session(path.as_deref(), &payload(&fixture_token()).unwrap(), false).unwrap();
        assert!(sync_session(path.as_deref(), "fixture", true)
            .unwrap_err()
            .contains("Initialize agy"));
        assert_eq!(fs::read_dir(home.path()).unwrap().count(), 0);
    }

    #[test]
    fn initialized_session_is_created_then_replaced_without_generic_gemini_files() {
        let home = tempfile::tempdir().unwrap();
        let directory = home.path().join(".gemini/antigravity-cli");
        fs::create_dir_all(&directory).unwrap();
        let path = session_path(home.path()).unwrap().unwrap();
        assert_eq!(path, directory.join("antigravity-oauth-token"));
        let mut token = fixture_token();
        for value in ["fixture-first", "fixture-second"] {
            token.refresh_token = value.into();
            let payload = payload(&token).unwrap();
            sync_session(Some(&path), &payload, true).unwrap();
            assert_eq!(fs::read_to_string(&path).unwrap(), payload);
            let json: serde_json::Value = serde_json::from_str(&payload).unwrap();
            assert_eq!(json["token"]["refresh_token"], value);
        }
        for name in ["oauth_creds.json", "google_accounts.json"] {
            assert!(!home.path().join(".gemini").join(name).exists());
        }
        assert_eq!(fs::read_dir(directory).unwrap().count(), 1);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn generic_gemini_files_remain_byte_for_byte_unchanged() {
        let home = tempfile::tempdir().unwrap();
        fs::create_dir_all(home.path().join(".gemini/antigravity-cli")).unwrap();
        let generic = ["oauth_creds.json", "google_accounts.json"];
        for name in generic {
            fs::write(home.path().join(".gemini").join(name), name.as_bytes()).unwrap();
        }
        sync_session(
            session_path(home.path()).unwrap().as_deref(),
            &payload(&fixture_token()).unwrap(),
            false,
        )
        .unwrap();
        for name in generic {
            assert_eq!(
                fs::read(home.path().join(".gemini").join(name)).unwrap(),
                name.as_bytes()
            );
        }
    }

    #[test]
    fn invalid_session_directory_is_not_silently_skipped() {
        let home = tempfile::tempdir().unwrap();
        let gemini = home.path().join(".gemini");
        fs::create_dir(&gemini).unwrap();
        let path = gemini.join("antigravity-cli");
        fs::write(&path, b"fixture-obstruction").unwrap();
        assert!(session_path(home.path()).unwrap_err().contains("directory"));
        assert_eq!(fs::read(path).unwrap(), b"fixture-obstruction");
    }

    #[test]
    fn invalid_session_target_is_reported_without_touching_it() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("antigravity-oauth-token");
        fs::create_dir(&path).unwrap();
        assert!(write_session(&path, "fixture")
            .unwrap_err()
            .contains("regular file"));
        assert!(path.is_dir());
        assert_eq!(fs::read_dir(home.path()).unwrap().count(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_session_is_rejected_without_modifying_its_target() {
        let home = tempfile::tempdir().unwrap();
        let target = home.path().join("original");
        let link = home.path().join("antigravity-oauth-token");
        fs::write(&target, b"fixture-original").unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert!(write_session(&link, "fixture-new")
            .unwrap_err()
            .contains("symlink"));
        assert!(link.is_symlink());
        assert_eq!(fs::read(target).unwrap(), b"fixture-original");
    }
}
