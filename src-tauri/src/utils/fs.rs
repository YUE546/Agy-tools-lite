use std::io::Write;
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Platform-specific atomic file replacement
#[cfg(target_os = "windows")]
fn atomic_replace_file(src: &Path, dst: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;

    type Bool = i32;
    type Dword = u32;

    #[link(name = "Kernel32")]
    extern "system" {
        fn MoveFileExW(
            lp_existing_file_name: *const u16,
            lp_new_file_name: *const u16,
            dw_flags: Dword,
        ) -> Bool;
    }

    let src_wide: Vec<u16> = src
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let dst_wide: Vec<u16> = dst
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    // MOVEFILE_REPLACE_EXISTING = 0x1
    // MOVEFILE_WRITE_THROUGH = 0x8
    const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x8;
    let flags = MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH;

    let result = unsafe { MoveFileExW(src_wide.as_ptr(), dst_wide.as_ptr(), flags) };
    if result == 0 {
        let err = std::io::Error::last_os_error();
        let _ = std::fs::remove_file(src);
        return Err(format!("MoveFileExW failed: {}", err));
    }

    Ok(())
}

/// Non-Windows: use standard atomic rename
#[cfg(not(target_os = "windows"))]
fn atomic_replace_file(src: &Path, dst: &Path) -> Result<(), String> {
    std::fs::rename(src, dst).map_err(|e| format!("rename failed: {}", e))
}

/// Atomically write bytes to a file at `target_path`.
///
/// Steps:
/// 1. Create a unique temporary file in the same directory (`<file>.tmp.<uuid>`).
/// 2. Write content and call `sync_all()` to flush buffers to physical disk.
/// 3. Replace target file via atomic rename (`MoveFileExW` with `MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH` on Windows, `fs::rename` on POSIX).
/// 4. If any step fails, remove the temporary file.
pub fn write_atomic<P: AsRef<Path>>(target_path: P, content: &[u8]) -> Result<(), String> {
    let target = target_path.as_ref();
    let parent_dir = target
        .parent()
        .ok_or_else(|| "Target path has no parent directory".to_string())?;

    let file_name = target
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file");
    let temp_filename = format!("{}.tmp.{}", file_name, Uuid::new_v4());
    let temp_path: PathBuf = parent_dir.join(temp_filename);

    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&temp_path)
        .map_err(|e| format!("Failed to create temporary file {:?}: {}", temp_path, e))?;

    if let Err(e) = file.write_all(content) {
        let _ = std::fs::remove_file(&temp_path);
        return Err(format!(
            "Failed to write to temporary file {:?}: {}",
            temp_path, e
        ));
    }

    if let Err(e) = file.sync_all() {
        let _ = std::fs::remove_file(&temp_path);
        return Err(format!(
            "Failed to fsync temporary file {:?}: {}",
            temp_path, e
        ));
    }

    // Explicitly drop file handle before rename to release Windows lock
    drop(file);

    if let Err(e) = atomic_replace_file(&temp_path, target) {
        let _ = std::fs::remove_file(&temp_path);
        return Err(format!("Failed to atomically replace {:?}: {}", target, e));
    }

    Ok(())
}

pub fn write_atomic_verified(target: &Path, content: &[u8]) -> Result<(), String> {
    let old = match std::fs::read(target) {
        Ok(old) => Some(old),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(format!("Cannot snapshot CLI session: {error}")),
    };
    write_atomic(target, content)?;
    match std::fs::read(target) {
        Ok(actual) if actual == content => Ok(()),
        _ => {
            let recovered = if let Some(old) = old {
                write_atomic(target, &old)
            } else {
                std::fs::remove_file(target).map_err(|error| error.to_string())
            };
            Err(if recovered.is_ok() {
                "CLI session readback failed; the previous file was restored.".into()
            } else {
                "CLI session readback and recovery failed; check the active CLI account.".into()
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[cfg(unix)]
    #[test]
    fn private_permissions_apply_to_new_and_replaced_files() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("antigravity-oauth-token");
        write_atomic(&target, b"fixture-first").unwrap();
        assert_eq!(
            fs::metadata(&target).unwrap().permissions().mode() & 0o777,
            0o600
        );
        fs::set_permissions(&target, fs::Permissions::from_mode(0o644)).unwrap();
        write_atomic(&target, b"fixture-second").unwrap();
        assert_eq!(
            fs::metadata(&target).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(fs::read(&target).unwrap(), b"fixture-second");
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);
    }

    #[test]
    fn test_write_atomic_basic() {
        let temp_dir = std::env::temp_dir().join(format!("test_atomic_{}", Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).unwrap();

        let target_file = temp_dir.join("config.json");
        let initial_data = b"{\"key\":\"initial_value\"}";
        write_atomic(&target_file, initial_data).expect("First write should succeed");

        assert_eq!(fs::read(&target_file).unwrap(), initial_data);

        let updated_data = b"{\"key\":\"updated_value\"}";
        write_atomic(&target_file, updated_data).expect("Overwrite should succeed");

        assert_eq!(fs::read(&target_file).unwrap(), updated_data);

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_write_atomic_survives_partial_tmp() {
        let temp_dir = std::env::temp_dir().join(format!("test_atomic_partial_{}", Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).unwrap();

        let target_file = temp_dir.join("accounts.json");
        let good_data = b"{\"valid\":true}";
        write_atomic(&target_file, good_data).expect("Initial write should succeed");

        // Simulate leftover half-written temp file from a simulated crash / power loss
        let leftover_tmp = temp_dir.join("accounts.json.tmp.aborted");
        fs::write(&leftover_tmp, b"{\"valid\":false, truncated...").unwrap();

        // Target file should remain intact and valid
        assert_eq!(fs::read(&target_file).unwrap(), good_data);

        // Next write should succeed cleanly
        let next_data = b"{\"valid\":true,\"generation\":2}";
        write_atomic(&target_file, next_data).expect("Subsequent write should succeed");
        assert_eq!(fs::read(&target_file).unwrap(), next_data);

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
