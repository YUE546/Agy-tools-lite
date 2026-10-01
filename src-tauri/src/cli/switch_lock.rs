//! Shared by GUI and CLI, held over the entire credential switch.
use std::{
    fs::{File, OpenOptions},
    path::Path,
};

pub(crate) struct SwitchLock {
    _file: File,
}
impl SwitchLock {
    pub(crate) fn acquire(root: &Path) -> Result<Self, String> {
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            // Deny concurrent opens; Windows releases the handle on process exit.
            options.share_mode(0);
        }
        let file = options
            .open(root.join("account-switch.lock"))
            .map_err(|error| {
                #[cfg(windows)]
                if error.raw_os_error() == Some(32) {
                    return "another_account_switch_in_progress".into();
                }
                format!("Cannot acquire account-switch lock: {error}")
            })?;
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            // LOCK_NB avoids blocking an async executor or waiting behind an unknown process.
            if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
                let error = std::io::Error::last_os_error();
                return Err(if error.kind() == std::io::ErrorKind::WouldBlock {
                    "another_account_switch_in_progress".into()
                } else {
                    format!("Cannot acquire account-switch lock: {error}")
                });
            }
        }
        // Never unlink the lock file: removing it lets two processes lock different inodes.
        Ok(Self { _file: file })
    }
}

#[cfg(unix)]
impl Drop for SwitchLock {
    fn drop(&mut self) {
        use std::os::fd::AsRawFd;
        // Explicit unlock avoids a concurrent fork retaining this open-file
        // description for the brief interval before its close-on-exec runs.
        unsafe {
            libc::flock(self._file.as_raw_fd(), libc::LOCK_UN);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_concurrent_switch_and_releases_on_drop() {
        let temp = tempfile::tempdir().unwrap();
        let first = SwitchLock::acquire(temp.path()).unwrap();
        assert!(
            matches!(SwitchLock::acquire(temp.path()), Err(error) if error == "another_account_switch_in_progress")
        );
        drop(first);
        assert!(SwitchLock::acquire(temp.path()).is_ok());
    }
    #[cfg(unix)]
    #[test]
    fn refuses_symlink_lock() {
        let temp = tempfile::tempdir().unwrap();
        let other = temp.path().join("other");
        std::fs::write(&other, "untouched").unwrap();
        std::os::unix::fs::symlink(&other, temp.path().join("account-switch.lock")).unwrap();
        assert!(SwitchLock::acquire(temp.path()).is_err());
        assert_eq!(std::fs::read_to_string(other).unwrap(), "untouched");
    }
    #[test]
    fn rejects_a_separate_process() {
        let temp = tempfile::tempdir().unwrap();
        let first = SwitchLock::acquire(temp.path()).unwrap();
        let probe = |expected: &str| {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "cli::switch_lock::tests::child_probe",
                    "--ignored",
                ])
                .env("AGY_LITE_LOCK_TEST_DIR", temp.path())
                .env("AGY_LITE_LOCK_TEST_EXPECT", expected)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stdout)
            );
        };
        probe("busy");
        drop(first);
        probe("free");
    }
    #[test]
    #[ignore = "helper invoked by rejects_a_separate_process"]
    fn child_probe() {
        let Some(path) = std::env::var_os("AGY_LITE_LOCK_TEST_DIR") else {
            return;
        };
        let expected = std::env::var("AGY_LITE_LOCK_TEST_EXPECT").unwrap();
        let result = SwitchLock::acquire(Path::new(&path));
        if expected == "busy" {
            assert!(matches!(result, Err(error) if error == "another_account_switch_in_progress"));
        } else {
            assert!(result.is_ok());
        }
    }
}
