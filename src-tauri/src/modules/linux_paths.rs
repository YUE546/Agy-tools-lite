//! Linux locations shared by process discovery and IDE data readers.
use std::path::{Path, PathBuf};

pub fn config_home() -> Option<PathBuf> {
    config_home_from(
        std::env::var_os("XDG_CONFIG_HOME")
            .as_deref()
            .map(Path::new),
        dirs::home_dir().as_deref(),
    )
}

fn config_home_from(xdg: Option<&Path>, home: Option<&Path>) -> Option<PathBuf> {
    xdg.filter(|path| path.is_absolute())
        .map(Path::to_path_buf)
        .or_else(|| home.map(|path| path.join(".config")))
}

pub fn find_executable(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    find_in_paths(name, std::env::split_paths(&path)).or_else(|| {
        dirs::home_dir().and_then(|home| executable(&home.join(".local/bin").join(name)))
    })
}

fn find_in_paths(name: &str, paths: impl Iterator<Item = PathBuf>) -> Option<PathBuf> {
    // Ignore relative PATH entries so a working-directory file cannot win discovery.
    paths
        .filter(|path| path.is_absolute())
        .find_map(|path| executable(&path.join(name)))
}

pub fn executable(path: &Path) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    let metadata = path.metadata().ok()?;
    if metadata.is_file() && metadata.permissions().mode() & 0o111 != 0 {
        std::fs::canonicalize(path).ok()
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{symlink, PermissionsExt};

    #[test]
    fn xdg_config_home_requires_an_absolute_path() {
        let home = Path::new("/home/test");
        assert_eq!(
            config_home_from(Some(Path::new("/custom/config")), Some(home)),
            Some(PathBuf::from("/custom/config"))
        );
        assert_eq!(
            config_home_from(Some(Path::new("relative")), Some(home)),
            Some(home.join(".config"))
        );
        assert_eq!(
            config_home_from(Some(Path::new("")), Some(home)),
            Some(home.join(".config"))
        );
    }

    #[test]
    fn discovery_resolves_symlinks_and_rejects_non_executables() {
        let temp = tempfile::tempdir().unwrap();
        let bin = temp.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        let actual = temp.path().join("actual");
        std::fs::write(&actual, b"#!/bin/sh\nexit 0\n").unwrap();
        symlink(&actual, bin.join("antigravity")).unwrap();
        std::fs::set_permissions(&actual, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert!(find_in_paths("antigravity", [bin.clone()].into_iter()).is_none());
        std::fs::set_permissions(&actual, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(
            find_in_paths("antigravity", [PathBuf::from("."), bin].into_iter()),
            Some(actual)
        );
    }
}
