//! Narrow macOS metadata collection for an already-running, known App.
//! No port scan, login/keychain access, launch, or security-setting changes.
#[cfg(target_os = "macos")]
use super::localization_transport::VerifiedListener;
use super::localization_transport::{AppOrigin, BrowserEndpoint, ListenerObservation};
#[cfg(target_os = "macos")]
use std::collections::BTreeMap;
#[cfg(any(target_os = "macos", test))]
use std::collections::BTreeSet;
use std::net::{IpAddr, SocketAddr};
#[cfg(target_os = "macos")]
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub(crate) struct Installation {
    pub bundle: PathBuf,
    pub executable: PathBuf,
    pub version: String,
}

#[derive(Debug, Clone)]
pub(crate) struct Discovery {
    pub endpoint: BrowserEndpoint,
    pub origins: Vec<AppOrigin>,
}

pub(crate) fn parse_listener_fields(text: &str) -> Result<Vec<ListenerObservation>, &'static str> {
    if text.len() > 256 * 1024 {
        return Err("metadata_unavailable");
    }
    let mut owner = None;
    let mut records = Vec::new();
    for line in text.lines() {
        if let Some(raw) = line.strip_prefix('p') {
            owner = Some(raw.parse::<u32>().map_err(|_| "metadata_unavailable")?);
        } else if let Some(raw) = line.strip_prefix('n') {
            let pid = owner.filter(|p| *p > 0).ok_or("metadata_unavailable")?;
            let (host, port) = raw.rsplit_once(':').ok_or("metadata_unavailable")?;
            let port = port.parse::<u16>().map_err(|_| "metadata_unavailable")?;
            let ip: IpAddr = match host {
                "*" => "0.0.0.0".parse().unwrap(),
                other => {
                    let host = if other.starts_with('[') {
                        other
                            .strip_prefix('[')
                            .and_then(|v| v.strip_suffix(']'))
                            .ok_or("metadata_unavailable")?
                    } else {
                        other
                    };
                    host.parse().map_err(|_| "metadata_unavailable")?
                }
            };
            if port == 0 || records.len() >= 64 {
                return Err("metadata_unavailable");
            }
            records.push(ListenerObservation {
                address: SocketAddr::new(ip, port),
                owner_pid: pid,
            });
        }
    }
    if records.is_empty() {
        return Err("metadata_unavailable");
    }
    Ok(records)
}

#[cfg(target_os = "macos")]
fn command(program: &str, args: &[String]) -> Result<String, &'static str> {
    let mut command = std::process::Command::new(program);
    command.args(args).stdin(std::process::Stdio::null());
    let output =
        crate::utils::process::output_with_timeout(command, std::time::Duration::from_secs(3))
            .map_err(|_| "metadata_unavailable")?;
    if !output.status.success() || output.stdout.len() > 256 * 1024 || !output.stderr.is_empty() {
        return Err("metadata_unavailable");
    }
    String::from_utf8(output.stdout)
        .map(|v| v.trim().to_string())
        .map_err(|_| "metadata_unavailable")
}

#[cfg(target_os = "macos")]
fn pids(args: &[String]) -> Result<Vec<u32>, &'static str> {
    let mut command = std::process::Command::new("/usr/bin/pgrep");
    command.args(args).stdin(std::process::Stdio::null());
    let output =
        crate::utils::process::output_with_timeout(command, std::time::Duration::from_secs(3))
            .map_err(|_| "metadata_unavailable")?;
    if output.status.code() == Some(1) && output.stdout.is_empty() && output.stderr.is_empty() {
        return Ok(vec![]);
    }
    if !output.status.success() || output.stdout.len() > 1024 || !output.stderr.is_empty() {
        return Err("metadata_unavailable");
    }
    parse_pid_metadata(&output.stdout)
}

#[cfg(any(target_os = "macos", test))]
fn parse_pid_metadata(output: &[u8]) -> Result<Vec<u32>, &'static str> {
    if output.len() > 1024 {
        return Err("metadata_unavailable");
    }
    let parsed: Result<BTreeSet<u32>, _> = std::str::from_utf8(output)
        .map_err(|_| "metadata_unavailable")?
        .lines()
        .map(str::parse)
        .collect();
    let parsed = parsed.map_err(|_| "metadata_unavailable")?;
    if parsed.len() > 8 || parsed.contains(&0) {
        return Err("multiple_instances");
    }
    Ok(parsed.into_iter().collect())
}

#[cfg(target_os = "macos")]
fn verify_executable(pid: u32, expected: &Path) -> Result<(), &'static str> {
    let actual = command(
        "/bin/ps",
        &[
            "-ww".into(),
            "-p".into(),
            pid.to_string(),
            "-o".into(),
            "comm=".into(),
        ],
    )?;
    let actual = std::fs::canonicalize(actual).map_err(|_| "metadata_unavailable")?;
    let expected = std::fs::canonicalize(expected).map_err(|_| "metadata_unavailable")?;
    if actual != expected {
        return Err("identity_mismatch");
    }
    Ok(())
}

/// Select only an exact executable match. Metadata collection failures must
/// never be mistaken for an unrelated process that can safely be ignored.
#[cfg(any(target_os = "macos", test))]
fn verified_app_pid(
    candidates: &[u32],
    mut verify: impl FnMut(u32) -> Result<(), &'static str>,
) -> Result<u32, &'static str> {
    if candidates.is_empty() {
        return Err("not_running");
    }
    if candidates.len() > 8 || candidates.contains(&0) {
        return Err("multiple_instances");
    }
    let mut matches = Vec::new();
    for &pid in candidates {
        match verify(pid) {
            Ok(()) => matches.push(pid),
            Err("identity_mismatch") => {}
            Err(error) => return Err(error),
        }
    }
    match matches.as_slice() {
        [pid] => Ok(*pid),
        [] => Err("identity_mismatch"),
        _ => Err("multiple_instances"),
    }
}

/// This is exclusively for a previously verified session's PID. A PID reuse
/// is deliberately treated as still running, so it can never clear restoration
/// state on the strength of an unrelated process's identity or a permission error.
#[cfg(target_os = "macos")]
pub(crate) fn known_process_exited(pid: u32) -> Result<bool, &'static str> {
    if pid == 0 || pid > i32::MAX as u32 {
        return Err("metadata_unavailable");
    }
    // Signal 0 inspects only this PID and sends no signal to the process.
    let result = unsafe { libc::kill(pid as libc::pid_t, 0) };
    let error = if result == -1 {
        std::io::Error::last_os_error().raw_os_error()
    } else {
        None
    };
    known_process_exit_result(result, error)
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn known_process_exited(_: u32) -> Result<bool, &'static str> {
    Err("unsupported_platform")
}

#[cfg(any(target_os = "macos", test))]
fn known_process_exit_result(result: i32, error: Option<i32>) -> Result<bool, &'static str> {
    match (result, error) {
        (0, None) => Ok(false),
        (-1, Some(libc::ESRCH)) => Ok(true),
        _ => Err("metadata_unavailable"),
    }
}

#[cfg(target_os = "macos")]
fn listener(port: u16, pid: u32) -> Result<VerifiedListener, &'static str> {
    let text = command(
        "/usr/sbin/lsof",
        &[
            "-nP".into(),
            format!("-iTCP:{port}"),
            "-sTCP:LISTEN".into(),
            "-Fpn".into(),
        ],
    )?;
    let observations = parse_listener_fields(&text)?;
    VerifiedListener::from_complete_observation(port, pid, &observations)
        .map_err(|_| "non_loopback_or_wrong_owner")
}

#[cfg(target_os = "macos")]
pub(crate) fn installed(configured: Option<&str>) -> Result<Option<Installation>, &'static str> {
    let candidates = if let Some(path) = configured {
        let mut path = PathBuf::from(path);
        while path.extension().is_none_or(|e| e != "app") {
            path = path.parent().ok_or("unknown_installation")?.to_path_buf();
        }
        vec![path]
    } else {
        let mut paths = vec![PathBuf::from("/Applications/Antigravity.app")];
        if let Some(home) = dirs::home_dir() {
            paths.push(home.join("Applications/Antigravity.app"));
        }
        paths
    };
    for bundle in candidates {
        match std::fs::symlink_metadata(&bundle) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => return Err("metadata_unavailable"),
        }
        let bundle = bundle.canonicalize().map_err(|_| "unknown_installation")?;
        let plist_path = bundle.join("Contents/Info.plist");
        if std::fs::metadata(&plist_path)
            .map_err(|_| "unknown_installation")?
            .len()
            > 1024 * 1024
        {
            return Err("unknown_installation");
        }
        let value = plist::Value::from_file(plist_path).map_err(|_| "unknown_installation")?;
        let info = value.as_dictionary().ok_or("unknown_installation")?;
        if info
            .get("CFBundleExecutable")
            .and_then(plist::Value::as_string)
            != Some("Antigravity")
        {
            return Err("unknown_installation");
        }
        let version = info
            .get("CFBundleShortVersionString")
            .and_then(plist::Value::as_string)
            .ok_or("unknown_installation")?
            .to_string();
        let asar_version =
            super::app_localization::read_asar_version(&bundle.join("Contents/Resources/app.asar"))
                .ok_or("unknown_installation")?;
        if version != asar_version {
            return Err("unknown_installation");
        }
        return Ok(Some(Installation {
            executable: bundle.join("Contents/MacOS/Antigravity"),
            bundle,
            version,
        }));
    }
    Ok(None)
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn installed(_: Option<&str>) -> Result<Option<Installation>, &'static str> {
    Err("unsupported_platform")
}

/// Independently revalidate only the existing browser endpoint. Cleanup may
/// need this after the last App window and its language server have exited.
#[cfg(target_os = "macos")]
pub(crate) fn rediscover_endpoint(
    installation: &Installation,
) -> Result<BrowserEndpoint, &'static str> {
    use std::io::Read;
    use std::os::unix::fs::OpenOptionsExt;

    // Effective user only, exact process name. No enumeration of other users.
    let app_pids = pids(&[
        "-u".into(),
        unsafe { libc::geteuid() }.to_string(),
        "-x".into(),
        "Antigravity".into(),
    ])?;
    let pid = verified_app_pid(&app_pids, |pid| {
        verify_executable(pid, &installation.executable)
    })?;
    let home = dirs::home_dir().ok_or("no_debug_port")?;
    // This exact case/path was verified in the authorized 2.19.1 Mac test.
    let port_file = home.join("Library/Application Support/Antigravity/DevToolsActivePort");
    // Open once without following symlinks, inspect that same file handle, and
    // bound the actual read even if the file changes after metadata collection.
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(port_file)
        .map_err(|_| "no_debug_port")?;
    let stat = file.metadata().map_err(|_| "no_debug_port")?;
    if !stat.is_file() || stat.len() > 256 {
        return Err("no_debug_port");
    }
    let mut contents = String::new();
    file.take(257)
        .read_to_string(&mut contents)
        .map_err(|_| "no_debug_port")?;
    if contents.len() > 256 {
        return Err("no_debug_port");
    }
    let raw_port = contents.lines().next().ok_or("no_debug_port")?;
    let port = raw_port.parse::<u16>().map_err(|_| "no_debug_port")?;
    if port == 0 {
        return Err("no_debug_port");
    }
    let endpoint = BrowserEndpoint::from_active_port_file(&contents, listener(port, pid)?)
        .map_err(|_| "no_debug_port")?;
    verify_executable(pid, &installation.executable)?;
    Ok(endpoint)
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn rediscover_endpoint(_: &Installation) -> Result<BrowserEndpoint, &'static str> {
    Err("unsupported_platform")
}

#[cfg(target_os = "macos")]
pub(crate) fn discover(installation: &Installation) -> Result<Discovery, &'static str> {
    let endpoint = rediscover_endpoint(installation)?;
    let pid = endpoint.browser_pid();
    let servers = pids(&[
        "-P".into(),
        pid.to_string(),
        "-x".into(),
        "language_server".into(),
    ])?;
    let mut origins = Vec::new();
    for server in servers {
        verify_executable(
            server,
            &installation
                .bundle
                .join("Contents/Resources/bin/language_server"),
        )?;
        let parent = command(
            "/bin/ps",
            &["-p".into(), server.to_string(), "-o".into(), "ppid=".into()],
        )?;
        if parent.parse::<u32>().ok() != Some(pid) {
            return Err("identity_mismatch");
        }
        let text = command(
            "/usr/sbin/lsof",
            &[
                "-nP".into(),
                "-a".into(),
                "-p".into(),
                server.to_string(),
                "-iTCP".into(),
                "-sTCP:LISTEN".into(),
                "-Fpn".into(),
            ],
        )?;
        let mut ports = BTreeMap::new();
        for row in parse_listener_fields(&text)? {
            ports.insert(row.address.port(), ());
        }
        if ports.len() > 8 {
            return Err("metadata_unavailable");
        }
        for port in ports.keys() {
            // Complete port observation, including a conflicting/non-loopback
            // listener, before accepting this server origin.
            match listener(*port, server) {
                Ok(_) => origins.push(
                    AppOrigin::from_verified_server_port(*port).map_err(|_| "identity_mismatch")?,
                ),
                Err("non_loopback_or_wrong_owner") => {}
                Err(error) => return Err(error),
            }
        }
    }
    origins.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    origins.dedup();
    if origins.is_empty() || origins.len() > 8 {
        return Err("identity_mismatch");
    }
    Ok(Discovery { endpoint, origins })
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn discover(_: &Installation) -> Result<Discovery, &'static str> {
    Err("unsupported_platform")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_only_bounded_numeric_listener_metadata() {
        let rows =
            parse_listener_fields("p42\nf16\nn127.0.0.1:54321\np42\nn[::1]:54321\n").unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows
            .iter()
            .all(|r| r.owner_pid == 42 && r.address.ip().is_loopback()));
        for raw in [
            "",
            "n127.0.0.1:42",
            "pno\nn127.0.0.1:42",
            "p42\nnlocalhost:42",
            "p42\nn127.0.0.1:0",
            "p42\nn[[::1]]:42",
            "p42\nn[::1:42",
            "p0\nn127.0.0.1:42",
        ] {
            assert!(parse_listener_fields(raw).is_err(), "{raw}");
        }
        let external = parse_listener_fields("p42\nn*:54321").unwrap();
        assert!(!external[0].address.ip().is_loopback());
    }

    #[test]
    fn process_metadata_requires_bounded_positive_numeric_pids() {
        assert_eq!(parse_pid_metadata(b"42\n43\n42\n"), Ok(vec![42, 43]));
        for raw in [b"0\n".as_slice(), b"no\n", b"-42\n", b"42 x\n", b"\xff\n"] {
            assert!(parse_pid_metadata(raw).is_err());
        }
        assert_eq!(
            parse_pid_metadata(b"1\n2\n3\n4\n5\n6\n7\n8\n9\n"),
            Err("multiple_instances")
        );
        assert_eq!(
            parse_pid_metadata(&vec![b'1'; 1025]),
            Err("metadata_unavailable")
        );
    }

    #[test]
    fn executable_verification_never_swallows_metadata_errors() {
        assert_eq!(verified_app_pid(&[], |_| Ok(())), Err("not_running"));
        assert_eq!(verified_app_pid(&[42], |_| Ok(())), Ok(42));
        assert_eq!(
            verified_app_pid(&[42, 43], |_| Ok(())),
            Err("multiple_instances")
        );
        assert_eq!(
            verified_app_pid(&[42], |_| Err("identity_mismatch")),
            Err("identity_mismatch")
        );
        assert_eq!(
            verified_app_pid(&[42, 43], |pid| if pid == 42 {
                Ok(())
            } else {
                Err("identity_mismatch")
            }),
            Ok(42)
        );
        // A permissions/timeout failure cannot be downgraded to an excluded PID.
        assert_eq!(
            verified_app_pid(&[42, 43], |pid| if pid == 42 {
                Ok(())
            } else {
                Err("metadata_unavailable")
            }),
            Err("metadata_unavailable")
        );
    }

    #[test]
    fn only_explicit_esrch_proves_a_known_process_has_exited() {
        assert_eq!(known_process_exit_result(0, None), Ok(false));
        assert_eq!(known_process_exit_result(-1, Some(libc::ESRCH)), Ok(true));
        for error in [
            None,
            Some(libc::EPERM),
            Some(libc::EACCES),
            Some(libc::EINVAL),
        ] {
            assert_eq!(
                known_process_exit_result(-1, error),
                Err("metadata_unavailable")
            );
        }
        assert_eq!(
            known_process_exit_result(1, None),
            Err("metadata_unavailable")
        );
    }
}
