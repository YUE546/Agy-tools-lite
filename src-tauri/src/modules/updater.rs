use serde::{Deserialize, Serialize};
const RELEASE_ROOT: &str = "https://github.com/anglee0323/antigravity-tools-lite/releases/tag/";
const API_URL: &str = "https://api.github.com/repos/anglee0323/antigravity-tools-lite/releases/latest";

#[derive(Debug, Serialize)]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub has_update: bool,
    pub release_url: String,
}
#[derive(Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
    draft: bool,
    prerelease: bool,
}
fn version(value: &str) -> Option<[u64; 3]> {
    let clean = value.strip_prefix('v').unwrap_or(value);
    let parts = clean.split('.').collect::<Vec<_>>();
    if parts.len() != 3 || parts.iter().any(|p| p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit())) { return None; }
    Some([parts[0].parse().ok()?, parts[1].parse().ok()?, parts[2].parse().ok()?])
}
fn evaluate(current: &str, release: Release) -> Result<UpdateInfo, String> {
    let latest = version(&release.tag_name).ok_or("invalid_release")?;
    let installed = version(current).ok_or("invalid_release")?;
    if release.draft || release.prerelease || release.html_url != format!("{RELEASE_ROOT}{}", release.tag_name) {
        return Err("invalid_release".into());
    }
    Ok(UpdateInfo { current_version: current.into(), latest_version: release.tag_name,
        has_update: latest > installed, release_url: release.html_url })
}

/// Read public release metadata only; no credentials, download or installer execution.
pub async fn check_for_updates() -> Result<UpdateInfo, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("antigravity-tools-lite/", env!("CARGO_PKG_VERSION")))
        .build().map_err(|_| "update_network_failed")?;
    let response = client.get(API_URL).header("Accept", "application/vnd.github+json")
        .send().await.map_err(|_| "update_network_failed")?;
    if !response.status().is_success() { return Err("update_network_failed".into()); }
    let body = response.text().await.map_err(|_| "update_network_failed")?;
    if body.len() > 1_000_000 { return Err("invalid_release".into()); }
    let release = serde_json::from_str(&body).map_err(|_| "invalid_release")?;
    evaluate(env!("CARGO_PKG_VERSION"), release)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn release(tag: &str) -> Release { Release { tag_name: tag.into(), html_url: format!("{RELEASE_ROOT}{tag}"), draft: false, prerelease: false } }
    #[test]
    fn only_newer_stable_versions_trigger_updates() {
        assert!(evaluate("4.7.8", release("v4.7.9")).unwrap().has_update);
        assert!(evaluate("4.7.8", release("v4.10.0")).unwrap().has_update);
        for tag in ["v4.7.8", "v4.7.7"] { assert!(!evaluate("4.7.8", release(tag)).unwrap().has_update); }
    }
    #[test]
    fn malformed_versions_and_untrusted_release_links_are_rejected() {
        for tag in ["v4.7", "v4.7.9-beta", "vv4.7.9", "v4.7.9/path", "4.7.99999999999999999999999"] { assert!(evaluate("4.7.8", release(tag)).is_err()); }
        for url in ["file:///tmp/installer", "https://evil.invalid/releases/tag/v4.7.9", "https://github.com/another/project/releases/tag/v4.7.9"] {
            let mut r = release("v4.7.9"); r.html_url = url.into(); assert!(evaluate("4.7.8", r).is_err());
        }
        for prerelease in [true, false] { let mut r = release("v4.7.9"); r.draft = !prerelease; r.prerelease = prerelease; assert!(evaluate("4.7.8", r).is_err()); }
    }
}
