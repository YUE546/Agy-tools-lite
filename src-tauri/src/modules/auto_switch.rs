//! Opt-in low-quota scheduling. This module never stops, kills, or starts a client.
//! A process scan is a conservative observation, not an atomic global idle barrier.
use crate::models::{Account, QuotaData};
use crate::modules::{account, cli_credentials, integration, version};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager};

const CONFIG_FILE: &str = "auto_switch.json";
const QUOTA_MAX_AGE: i64 = 180;
const REFRESH_SECONDS: i64 = 60;
const COOLDOWN_SECONDS: i64 = 300;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    #[default]
    Wait,
    Stop,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Target {
    #[default]
    App,
}
impl Target {
    fn argument(self) -> Option<&'static str> {
        match self {
            Self::App => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub enabled: bool,
    pub mode: Mode,
    pub reserve_percentage: u8,
    pub candidate_min_percentage: u8,
    pub monitored_model: String,
    pub candidate_account_ids: Vec<String>,
    pub target: Target,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: Mode::Wait,
            reserve_percentage: 10,
            candidate_min_percentage: 30,
            monitored_model: String::new(),
            candidate_account_ids: vec![],
            target: Target::App,
        }
    }
}
impl Config {
    fn validate(&self) -> Result<(), String> {
        if !(1..=98).contains(&self.reserve_percentage)
            || self.candidate_min_percentage <= self.reserve_percentage
            || self.candidate_min_percentage > 100
        {
            return Err("Choose a reserve from 1–98% and a candidate minimum above the reserve, up to 100%.".into());
        }
        if self.candidate_account_ids.len() > 10 || self.monitored_model.len() > 200 {
            return Err("Select no more than 10 candidate accounts.".into());
        }
        let mut ids = HashSet::new();
        if self
            .candidate_account_ids
            .iter()
            .any(|id| id.is_empty() || !ids.insert(id))
        {
            return Err("Candidate accounts must be unique.".into());
        }
        if self.enabled
            && (self.monitored_model.trim().is_empty() || self.candidate_account_ids.is_empty())
        {
            return Err(
                "Choose a model and at least one allowed backup account before enabling.".into(),
            );
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessState {
    Closed,
    Running,
    #[default]
    Unknown,
}
#[derive(Clone, Debug, Serialize)]
pub struct Status {
    pub phase: String,
    pub reason: Option<String>,
    pub source_account_id: Option<String>,
    pub source_email: Option<String>,
    pub target_account_id: Option<String>,
    pub target_email: Option<String>,
    pub remaining_percentage: Option<f64>,
    pub pending_id: Option<String>,
    pub mode: Mode,
    pub process_state: ProcessState,
    pub last_checked: Option<i64>,
}
impl Default for Status {
    fn default() -> Self {
        Self {
            phase: "disabled".into(),
            reason: None,
            source_account_id: None,
            source_email: None,
            target_account_id: None,
            target_email: None,
            remaining_percentage: None,
            pending_id: None,
            mode: Mode::Wait,
            process_state: ProcessState::Unknown,
            last_checked: None,
        }
    }
}
impl Status {
    fn set(&mut self, phase: &str, reason: &str) {
        self.phase = phase.into();
        self.reason = Some(reason.into());
    }
}
#[derive(Clone, Debug)]
struct Pending {
    id: String,
    source_id: String,
    target_id: String,
    revision: u64,
    closed_since: Option<i64>,
}
#[derive(Default)]
struct RuntimeData {
    config: Config,
    status: Status,
    revision: u64,
    pending: Option<Pending>,
    canceled_source: Option<String>,
    cooldown_until: i64,
    refresh_after: HashMap<String, i64>,
    refresh_failed: HashSet<String>,
    failed: bool,
    commit_started: bool,
}
#[derive(Default)]
pub struct Runtime {
    data: Arc<Mutex<RuntimeData>>,
    tick: tokio::sync::Mutex<()>,
}

#[derive(Default, Serialize, Deserialize)]
struct PauseRecord {
    source_id: Option<String>,
    failed: bool,
}
fn write_pause(source_id: Option<String>, failed: bool) -> Result<(), String> {
    let bytes = serde_json::to_vec(&PauseRecord { source_id, failed })
        .map_err(|_| "Cannot encode switch state.")?;
    crate::utils::fs::write_atomic(
        &account::get_data_dir()?.join("auto_switch_state.json"),
        &bytes,
    )
    .map_err(|_| "Cannot save switch state.".into())
}
fn read_pause() -> Result<PauseRecord, String> {
    match std::fs::read(account::get_data_dir()?.join("auto_switch_state.json")) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|_| "Cannot read switch state.".into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(PauseRecord::default()),
        Err(_) => Err("Cannot read switch state.".into()),
    }
}

fn config_path() -> Result<std::path::PathBuf, String> {
    Ok(account::get_data_dir()?.join(CONFIG_FILE))
}
fn read_config() -> Result<Config, String> {
    match std::fs::read(config_path()?) {
        Ok(bytes) => {
            let c: Config = serde_json::from_slice(&bytes)
                .map_err(|_| "Cannot read auto-switch settings.".to_string())?;
            c.validate()?;
            Ok(c)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
        Err(_) => Err("Cannot read auto-switch settings.".into()),
    }
}

/// Known bucket IDs, rather than translated display labels, identify provider pools.
/// Missing/ambiguous windows are unknown. The UI's single percentage is insufficient.
fn remaining(q: &QuotaData, model: &str, now: i64) -> Result<f64, &'static str> {
    if q.is_forbidden {
        return Err("account_unavailable");
    }
    if q.last_updated > now + 30 || now - q.last_updated > QUOTA_MAX_AGE {
        return Err("stale_quota");
    }
    let mut resolved = model;
    for _ in 0..8 {
        if let Some(next) = q.model_forwarding_rules.get(resolved) {
            resolved = next;
        } else {
            break;
        }
    }
    let m = q
        .models
        .iter()
        .find(|m| m.name == resolved)
        .ok_or("unknown_pool")?;
    if !(0..=100).contains(&m.percentage) {
        return Err("unknown_pool");
    }
    let lower = resolved.to_ascii_lowercase();
    let pool = if lower.starts_with("gemini") {
        "gemini"
    } else if lower.starts_with("claude") || lower.starts_with("gpt") {
        "3p"
    } else {
        return Err("unknown_pool");
    };
    let groups = q.quota_groups.as_ref().ok_or("unknown_pool")?;
    let mut weekly = None::<f64>;
    let mut short = None::<f64>;
    for bucket in groups.iter().flat_map(|g| &g.buckets) {
        let id = bucket.bucket_id.to_ascii_lowercase();
        if !id.starts_with(&format!("{pool}-")) {
            continue;
        }
        if !bucket.remaining_fraction.is_finite()
            || !(0.0..=1.0).contains(&bucket.remaining_fraction)
        {
            return Err("unknown_pool");
        }
        let reset = chrono::DateTime::parse_from_rfc3339(&bucket.reset_time)
            .map_err(|_| "unknown_pool")?
            .timestamp();
        // A reset deadline is not evidence that quota has actually recovered.
        if reset <= now {
            return Err("stale_quota");
        }
        let window = bucket.window.to_ascii_lowercase();
        let value = bucket.remaining_fraction * 100.0;
        if id.ends_with("weekly") || window == "weekly" {
            weekly = Some(weekly.map_or(value, |old| old.min(value)));
        } else if id.ends_with("5h") || window == "5h" {
            short = Some(short.map_or(value, |old| old.min(value)));
        } else {
            return Err("unknown_pool");
        }
    }
    let weekly = weekly.ok_or("unknown_pool")?;
    let free = q
        .subscription_tier
        .as_deref()
        .unwrap_or("")
        .to_ascii_lowercase()
        .contains("free");
    let short = match short {
        Some(v) => v,
        None if free => 100.0,
        None => return Err("unknown_pool"),
    };
    Ok(weekly.min(short).min(m.percentage as f64))
}
fn usable(a: &Account) -> bool {
    !a.disabled && !a.validation_blocked && !a.quota.as_ref().is_some_and(|q| q.is_forbidden)
}
fn account_remaining(a: &Account, model: &str, now: i64) -> Result<f64, &'static str> {
    if !usable(a) {
        return Err("account_unavailable");
    }
    remaining(a.quota.as_ref().ok_or("no_quota")?, model, now)
}

/// Scan all known native APP/IDE/agy processes even for CLI-only changes. An open
/// client may cache credentials or later write them back. A running client is NOT
/// evidence of a running task; it simply prevents this closed-client-only mode.
fn clients() -> ProcessState {
    let mut system = sysinfo::System::new();
    system.refresh_processes(sysinfo::ProcessesToUpdate::All);
    if system.processes().is_empty() {
        return ProcessState::Unknown;
    }
    let own = std::process::id();
    let mut paths = vec![];
    if let Ok(config) = crate::modules::config::load_app_config() {
        for p in [
            config.antigravity_executable,
            config.antigravity_ide_executable,
        ]
        .into_iter()
        .flatten()
        {
            match std::fs::canonicalize(p) {
                Ok(p) => paths.push(p),
                Err(_) => return ProcessState::Unknown,
            }
        }
    } else {
        return ProcessState::Unknown;
    }
    for (pid, proc) in system.processes() {
        if pid.as_u32() == own {
            continue;
        }
        let name = proc.name().to_string_lossy().to_ascii_lowercase();
        if name.starts_with("antigravity-tools") || name.starts_with("antigravity_tools") {
            continue;
        }
        let named = name.contains("antigravity") || name == "agy" || name == "agy.exe";
        let configured = proc.exe().is_some_and(|p| {
            paths
                .iter()
                .any(|configured| p == configured || p.starts_with(configured))
        });
        if named || configured {
            return ProcessState::Running;
        }
    }
    ProcessState::Closed
}

/// Compare credentials privately. No token or raw authentication error is exposed
/// through the status DTO. Any external sign-in mismatch invalidates the request.
fn verify_source(source: &Account, target: Target) -> Result<(), &'static str> {
    let actual = match target {
        Target::App => {
            let v = installed_app_version()?;
            if version::compare_version(&v.short_version, "2.0.0") == std::cmp::Ordering::Less {
                return Err("unsupported_client");
            }
            integration::read_from_system_keyring()
        }
    }
    .map_err(|_| "credentials_changed")?;
    if actual.refresh_token.is_empty() || actual.refresh_token != source.token.refresh_token {
        return Err("credentials_changed");
    }
    if target == Target::App {
        let home = dirs::home_dir().ok_or("credentials_changed")?;
        if let Some(path) =
            cli_credentials::session_path(&home).map_err(|_| "credentials_changed")?
        {
            if !native_session_exists(&path)? {
                return Ok(());
            }
            let cli = integration::read_cli_credentials().map_err(|_| "credentials_changed")?;
            if cli.refresh_token != source.token.refresh_token {
                return Err("credentials_changed");
            }
        }
    }
    Ok(())
}

/// Never execute the APP just to discover its version on Linux. Some bundles
/// interpret --version as a normal launch, violating closed-client-only behavior.
fn installed_app_version() -> Result<version::AntigravityVersion, &'static str> {
    #[cfg(target_os = "linux")]
    {
        let path = crate::modules::process::get_antigravity_executable_path(None)
            .ok_or("unsupported_client")?;
        let path = std::fs::canonicalize(path).map_err(|_| "unsupported_client")?;
        let package = path
            .parent()
            .ok_or("unsupported_client")?
            .join("resources/app/package.json");
        let content = std::fs::read(package).map_err(|_| "unsupported_client")?;
        let json: serde_json::Value =
            serde_json::from_slice(&content).map_err(|_| "unsupported_client")?;
        let v = json
            .get("version")
            .and_then(|v| v.as_str())
            .filter(|v| !v.is_empty())
            .ok_or("unsupported_client")?;
        Ok(version::AntigravityVersion {
            short_version: v.into(),
            bundle_version: v.into(),
        })
    }
    #[cfg(not(target_os = "linux"))]
    {
        version::get_antigravity_version(None).map_err(|_| "unsupported_client")
    }
}

fn native_session_exists(path: &std::path::Path) -> Result<bool, &'static str> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() => Ok(true),
        Ok(_) => Err("credentials_changed"),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err("credentials_changed"),
    }
}

fn advance_pending(
    d: &mut RuntimeData,
    source_id: &str,
    target_id: &str,
    now: i64,
    process_state: ProcessState,
) -> Pending {
    if !d
        .pending
        .as_ref()
        .is_some_and(|p| p.source_id == source_id && p.target_id == target_id)
    {
        d.pending = Some(Pending {
            id: uuid::Uuid::new_v4().to_string(),
            source_id: source_id.into(),
            target_id: target_id.into(),
            revision: d.revision,
            closed_since: None,
        });
    }
    let p = d.pending.as_mut().unwrap();
    match process_state {
        ProcessState::Closed => {
            if p.closed_since.is_none() {
                p.closed_since = Some(now);
            }
        }
        _ => p.closed_since = None,
    }
    p.clone()
}

fn commit_guard(
    d: &RuntimeData,
    pending: &Pending,
    config: &Config,
    current: Option<&str>,
    source: &Account,
    target: &Account,
    now: i64,
    process: ProcessState,
) -> Result<(), &'static str> {
    if d.revision != pending.revision
        || &d.config != config
        || !config.enabled
        || !d.pending.as_ref().is_some_and(|p| p.id == pending.id)
    {
        return Err("request_changed");
    }
    if current != Some(pending.source_id.as_str()) || source.id != pending.source_id {
        return Err("source_changed");
    }
    if target.id != pending.target_id || !config.candidate_account_ids.contains(&target.id) {
        return Err("no_candidate");
    }
    if !account_remaining(source, &config.monitored_model, now)
        .is_ok_and(|v| v <= config.reserve_percentage as f64)
    {
        return Err("no_quota");
    }
    if !account_remaining(target, &config.monitored_model, now)
        .is_ok_and(|v| v >= config.candidate_min_percentage as f64)
    {
        return Err("no_candidate");
    }
    match process {
        ProcessState::Running => Err("clients_running"),
        ProcessState::Unknown => Err("process_unknown"),
        ProcessState::Closed => Ok(()),
    }
}

pub fn start(app: tauri::AppHandle) {
    let runtime = app.state::<Runtime>();
    if let Ok(mut d) = runtime.data.lock() {
        match (read_config(), read_pause()) {
            (Ok(c), Ok(pause)) => {
                d.config = c;
                d.canceled_source = pause.source_id;
                d.failed = pause.failed;
                if d.failed {
                    d.status.set("blocked", "switch_failed");
                }
            }
            _ => {
                d.failed = true;
                d.status.set("blocked", "configuration_required");
            }
        }
    }
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            if evaluate(&app, false).await.is_err() {
                if let Ok(mut d) = app.state::<Runtime>().data.lock() {
                    if d.config.enabled && !d.commit_started {
                        d.status.set("blocked", "state_unavailable");
                    }
                }
            }
        }
    });
}
#[tauri::command]
pub fn get_auto_switch_config(app: tauri::AppHandle) -> Result<Config, String> {
    Ok(app
        .state::<Runtime>()
        .data
        .lock()
        .map_err(|_| "Auto-switch state is unavailable.")?
        .config
        .clone())
}
#[tauri::command]
pub fn set_auto_switch_config(app: tauri::AppHandle, mut config: Config) -> Result<Config, String> {
    config.monitored_model = config.monitored_model.trim().to_owned();
    config.validate()?;
    let runtime = app.state::<Runtime>();
    let mut d = runtime
        .data
        .lock()
        .map_err(|_| "Auto-switch state is unavailable.")?;
    if d.commit_started {
        return Err(
            "Account credentials are being updated. Wait for the result before changing settings."
                .into(),
        );
    }
    if config.enabled {
        let accounts = account::list_accounts()?;
        if config
            .candidate_account_ids
            .iter()
            .any(|id| !accounts.iter().any(|a| &a.id == id))
        {
            return Err("A selected account no longer exists. Refresh the account list.".into());
        }
    }
    let bytes = serde_json::to_vec_pretty(&config).map_err(|_| "Cannot encode settings.")?;
    crate::utils::fs::write_atomic(&config_path()?, &bytes)
        .map_err(|_| "Cannot save auto-switch settings.")?;
    write_pause(None, false)?;
    d.config = config.clone();
    d.revision += 1;
    d.pending = None;
    d.canceled_source = None;
    d.refresh_after.clear();
    d.refresh_failed.clear();
    d.failed = false;
    d.status = Status {
        mode: config.mode,
        phase: if config.enabled {
            "monitoring"
        } else {
            "disabled"
        }
        .into(),
        ..Status::default()
    };
    Ok(config)
}
#[tauri::command]
pub fn get_auto_switch_status(app: tauri::AppHandle) -> Result<Status, String> {
    Ok(app
        .state::<Runtime>()
        .data
        .lock()
        .map_err(|_| "Auto-switch state is unavailable.")?
        .status
        .clone())
}
#[tauri::command]
pub fn cancel_auto_switch(app: tauri::AppHandle, pending_id: String) -> Result<Status, String> {
    let runtime = app.state::<Runtime>();
    let mut d = runtime
        .data
        .lock()
        .map_err(|_| "Auto-switch state is unavailable.")?;
    if d.commit_started {
        return Err("Account credentials are being updated. Wait for the result.".into());
    }
    let pending = d
        .pending
        .as_ref()
        .filter(|p| p.id == pending_id)
        .ok_or("This switch request is no longer pending.")?
        .clone();
    d.canceled_source = Some(pending.source_id.clone());
    d.revision += 1;
    d.pending = None;
    d.status.pending_id = None;
    d.status.set("canceled", "canceled_until_recovery");
    // Cancellation takes effect immediately even if its durable record fails.
    write_pause(Some(pending.source_id), false)?;
    Ok(d.status.clone())
}
#[tauri::command]
pub async fn check_auto_switch_now(app: tauri::AppHandle) -> Result<Status, String> {
    evaluate(&app, true).await?;
    get_auto_switch_status(app)
}

async fn refresh(app: &tauri::AppHandle, id: &str, force: bool) -> Result<Account, &'static str> {
    let now = chrono::Utc::now().timestamp();
    let runtime = app.state::<Runtime>();
    let should_refresh = {
        let mut d = runtime.data.lock().map_err(|_| "no_quota")?;
        let after = d.refresh_after.get(id).copied().unwrap_or(0);
        // Even manual checks have a 10-second floor; a failed API is not hammered.
        if now < after && (!force || after - now > REFRESH_SECONDS - 10) {
            false
        } else {
            d.refresh_after.insert(id.into(), now + REFRESH_SECONDS);
            true
        }
    };
    let mut a = account::load_account(id).map_err(|_| "account_unavailable")?;
    if should_refresh {
        let quota = match account::fetch_quota_with_retry(&mut a).await {
            Ok(q) => {
                runtime
                    .data
                    .lock()
                    .map_err(|_| "no_quota")?
                    .refresh_failed
                    .remove(id);
                q
            }
            Err(_) => {
                runtime
                    .data
                    .lock()
                    .map_err(|_| "no_quota")?
                    .refresh_failed
                    .insert(id.into());
                return Err("no_quota");
            }
        };
        account::update_account_quota(id, quota).map_err(|_| "no_quota")?;
        a = account::load_account(id).map_err(|_| "account_unavailable")?;
    }
    if runtime
        .data
        .lock()
        .map_err(|_| "no_quota")?
        .refresh_failed
        .contains(id)
    {
        return Err("no_quota");
    }
    Ok(a)
}
fn update_status(app: &tauri::AppHandle, revision: u64, status: Status) {
    if let Ok(mut d) = app.state::<Runtime>().data.lock() {
        if d.revision == revision {
            d.status = status;
        }
    }
}

async fn evaluate(app: &tauri::AppHandle, force: bool) -> Result<(), String> {
    let runtime = app.state::<Runtime>();
    let Ok(_tick) = runtime.tick.try_lock() else {
        return Ok(());
    };
    let (config, revision, previous, cooldown) = {
        let d = runtime.data.lock().map_err(|_| "State unavailable")?;
        (
            d.config.clone(),
            d.revision,
            d.status.clone(),
            d.cooldown_until,
        )
    };
    if !config.enabled || runtime.data.lock().map_err(|_| "State unavailable")?.failed {
        return Ok(());
    }
    let now = chrono::Utc::now().timestamp();
    let mut status = Status {
        phase: "monitoring".into(),
        mode: config.mode,
        last_checked: Some(now),
        ..Status::default()
    };
    if now < cooldown {
        status = previous;
        status.last_checked = Some(now);
        update_status(app, revision, status);
        return Ok(());
    }
    let source_id = match account::get_current_account_id()? {
        Some(id) => id,
        None => {
            status.set("blocked", "no_current_account");
            update_status(app, revision, status);
            return Ok(());
        }
    };
    status.source_account_id = Some(source_id.clone());
    let source = match refresh(app, &source_id, force).await {
        Ok(a) => a,
        Err(reason) => {
            status.set("blocked", reason);
            update_status(app, revision, status);
            return Ok(());
        }
    };
    status.source_email = Some(source.email.clone());
    let low = match account_remaining(&source, &config.monitored_model, now) {
        Ok(p) => p,
        Err(reason) => {
            status.set("blocked", reason);
            update_status(app, revision, status);
            return Ok(());
        }
    };
    status.remaining_percentage = Some(low);
    {
        let mut d = runtime.data.lock().map_err(|_| "State unavailable")?;
        if d.revision != revision {
            return Ok(());
        }
        if d.pending.as_ref().is_some_and(|p| p.source_id != source_id) {
            d.pending = None;
            d.canceled_source = None;
        }
        if low > config.reserve_percentage as f64 {
            d.pending = None;
            if low >= config.candidate_min_percentage as f64 && d.canceled_source.is_some() {
                write_pause(None, false)?;
                d.canceled_source = None;
            }
            d.status = status;
            return Ok(());
        }
        if d.canceled_source.as_deref() == Some(&source_id) {
            status.set("canceled", "canceled_until_recovery");
            d.status = status;
            return Ok(());
        }
    }
    let mut candidate = None;
    for id in &config.candidate_account_ids {
        if id == &source_id {
            continue;
        }
        if let Ok(a) = refresh(app, id, force).await {
            if account_remaining(&a, &config.monitored_model, now)
                .is_ok_and(|p| p >= config.candidate_min_percentage as f64)
            {
                candidate = Some(a);
                break;
            }
        }
        if runtime
            .data
            .lock()
            .map_err(|_| "State unavailable")?
            .revision
            != revision
        {
            return Ok(());
        }
    }
    let Some(candidate) = candidate else {
        status.set("blocked", "no_candidate");
        update_status(app, revision, status);
        return Ok(());
    };
    status.target_account_id = Some(candidate.id.clone());
    status.target_email = Some(candidate.email.clone());
    let process_state = tokio::task::spawn_blocking(clients)
        .await
        .unwrap_or(ProcessState::Unknown);
    status.process_state = process_state;
    let pending = {
        let mut d = runtime.data.lock().map_err(|_| "State unavailable")?;
        if d.revision != revision {
            return Ok(());
        }
        let result = advance_pending(&mut d, &source_id, &candidate.id, now, process_state);
        status.pending_id = Some(result.id.clone());
        match process_state {
            ProcessState::Running => status.set("pending", "clients_running"),
            ProcessState::Unknown => status.set("blocked", "process_unknown"),
            ProcessState::Closed => status.set("pending", "ready"),
        }
        d.status = status.clone();
        result
    };
    // Two observations separated by >=3s improve race detection. They cannot stop
    // an uncoordinated external client from launching during a credential write.
    if !pending.closed_since.is_some_and(|since| now - since >= 3) {
        return Ok(());
    }
    let integration = ClosedOnlyIntegration {
        data: runtime.data.clone(),
        pending: pending.clone(),
        config: config.clone(),
    };
    status.set("pending", "checking");
    update_status(app, revision, status);
    let result =
        account::switch_account(&candidate.id, config.target.argument(), &integration).await;
    let mut d = runtime.data.lock().map_err(|_| "State unavailable")?;
    d.commit_started = false;
    if d.revision != revision {
        return Ok(());
    }
    match result {
        Ok(()) => {
            d.pending = None;
            d.status.pending_id = None;
            d.status.set("completed", "credentials_updated");
            d.cooldown_until = now + COOLDOWN_SECONDS;
            if write_pause(None, false).is_err() {
                d.failed = true;
            }
            let _ = app.emit("tray://account-switched", ());
            crate::modules::tray::update_tray_menus(app);
        }
        Err(_) => {
            // Authentication writes can fail partially. Never loop over identities
            // or retry unattended after an uncertain commit. Re-enable to retry.
            if matches!(
                d.status.reason.as_deref(),
                Some("clients_running" | "process_unknown")
            ) {
                if let Some(p) = d.pending.as_mut() {
                    p.closed_since = None;
                }
            } else {
                d.pending = None;
                d.status.pending_id = None;
                d.canceled_source = Some(source_id.clone());
                d.failed = true;
                let _ = write_pause(Some(source_id), true);
                if matches!(d.status.reason.as_deref(), Some("checking") | None) {
                    d.status.set("blocked", "switch_failed");
                }
            }
        }
    }
    Ok(())
}

struct ClosedOnlyIntegration {
    data: Arc<Mutex<RuntimeData>>,
    pending: Pending,
    config: Config,
}
impl integration::SystemIntegration for ClosedOnlyIntegration {
    async fn on_account_switch(
        &self,
        target: &Account,
        _target_ide: Option<&str>,
    ) -> Result<(), String> {
        let data = self.data.clone();
        let p = self.pending.clone();
        let config = self.config.clone();
        let target = target.clone();
        tokio::task::spawn_blocking(move || {
            let mut d = data.lock().map_err(|_| "state_unavailable")?;
            let fail = |d: &mut RuntimeData, reason: &str| -> Result<(), String> {
                d.status.set("blocked", reason);
                Err(reason.into())
            };
            if read_config()? != config {
                return fail(&mut d, "configuration_required");
            }
            let source = account::load_account(&p.source_id)?;
            let latest_target = account::load_account(&target.id)?;
            let now = chrono::Utc::now().timestamp();
            let current = account::get_current_account_id()?;
            if let Err(reason) = commit_guard(
                &d,
                &p,
                &config,
                current.as_deref(),
                &source,
                &latest_target,
                now,
                clients(),
            ) {
                return fail(&mut d, reason);
            }
            if let Err(reason) = verify_source(&source, config.target) {
                return fail(&mut d, reason);
            }
            // Verify once more after potentially slow keyring access.
            match clients() {
                ProcessState::Running => return fail(&mut d, "clients_running"),
                ProcessState::Unknown => return fail(&mut d, "process_unknown"),
                ProcessState::Closed => {}
            }
            // A crash or partial credential commit must never auto-retry on restart.
            // Persist the fail-closed journal before any credential write.
            write_pause(Some(p.source_id.clone()), true)?;
            d.commit_started = true;
            d.status.set("switching", "checking");
            // account::switch_account holds BOTH the in-process and PR5's
            // cross-process switch locks for this callback and the index update.
            integration::write_to_system_keyring(&target, false)?;
            Ok(())
        })
        .await
        .map_err(|_| "Safe switch worker failed.".to_string())?
    }
    fn update_tray(&self) {}
    fn show_notification(&self, _title: &str, _body: &str) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    const NOW: i64 = 1_800_000_000;
    fn quota(weekly: f64, short: f64) -> QuotaData {
        serde_json::from_value(serde_json::json!({
            "models": [{"name":"gemini-test", "percentage":(short * 100.0) as i32, "reset_time":"2027-02-01T00:00:00Z"}],
            "last_updated":NOW, "subscription_tier":"PRO",
            "quota_groups": [{"display_name":"任意语言名称", "buckets":[
                {"bucket_id":"gemini-weekly","window":"weekly","remaining_fraction":weekly,"reset_time":"2027-02-01T00:00:00Z"},
                {"bucket_id":"gemini-5h","window":"5h","remaining_fraction":short,"reset_time":"2027-02-01T00:00:00Z"}
            ]}]
        })).unwrap()
    }
    fn account_fixture(id: &str, weekly: f64, short: f64) -> Account {
        let token = crate::models::TokenData::new(
            format!("fixture-{id}-access"),
            format!("fixture-{id}-refresh"),
            3600,
            None,
            None,
            None,
            true,
            None,
        );
        let mut a = Account::new(id.into(), format!("{id}@example.invalid"), token);
        a.quota = Some(quota(weekly, short));
        a
    }
    fn config(mode: Mode) -> Config {
        Config {
            enabled: true,
            mode,
            monitored_model: "gemini-test".into(),
            candidate_account_ids: vec!["B".into()],
            ..Config::default()
        }
    }
    fn runtime(mode: Mode) -> RuntimeData {
        RuntimeData {
            config: config(mode),
            ..RuntimeData::default()
        }
    }

    #[test]
    fn opt_in_and_config_validation() {
        assert!(!Config::default().enabled);
        assert!(!serde_json::from_str::<Config>("{}").unwrap().enabled);
        let mut c = Config::default();
        c.enabled = true;
        assert!(c.validate().is_err());
        c = config(Mode::Wait);
        assert!(c.validate().is_ok());
        c.candidate_min_percentage = 10;
        assert!(c.validate().is_err());
        c.candidate_min_percentage = 30;
        c.candidate_account_ids.push("B".into());
        assert!(c.validate().is_err());
    }
    #[test]
    fn considers_both_windows_and_provider_ids_not_labels() {
        assert_eq!(remaining(&quota(0.08, 0.8), "gemini-test", NOW), Ok(8.0));
        assert_eq!(remaining(&quota(0.8, 0.08), "gemini-test", NOW), Ok(8.0));
        let mut q = quota(0.8, 0.8);
        q.quota_groups.as_mut().unwrap()[0].buckets[0].bucket_id = "3p-weekly".into();
        assert_eq!(remaining(&q, "gemini-test", NOW), Err("unknown_pool"));
    }
    #[test]
    fn unknown_stale_reset_and_invalid_numbers_fail_closed() {
        let mut q = quota(0.8, 0.8);
        q.quota_groups = None;
        assert_eq!(remaining(&q, "gemini-test", NOW), Err("unknown_pool"));
        q = quota(0.8, 0.8);
        q.last_updated = NOW - QUOTA_MAX_AGE - 1;
        assert_eq!(remaining(&q, "gemini-test", NOW), Err("stale_quota"));
        q = quota(0.8, 0.8);
        q.quota_groups.as_mut().unwrap()[0].buckets[0].reset_time = "2020-01-01T00:00:00Z".into();
        assert_eq!(remaining(&q, "gemini-test", NOW), Err("stale_quota"));
        q = quota(0.8, 0.8);
        q.quota_groups.as_mut().unwrap()[0].buckets[0].remaining_fraction = f64::NAN;
        assert_eq!(remaining(&q, "gemini-test", NOW), Err("unknown_pool"));
        assert_eq!(
            remaining(&quota(0.8, 0.8), "missing-model", NOW),
            Err("unknown_pool")
        );
    }
    #[test]
    fn free_weekly_only_is_supported_without_inventing_paid_capacity() {
        let mut q = quota(0.08, 0.8);
        q.quota_groups.as_mut().unwrap()[0].buckets.pop();
        assert_eq!(remaining(&q, "gemini-test", NOW), Err("unknown_pool"));
        q.subscription_tier = Some("FREE".into());
        assert_eq!(remaining(&q, "gemini-test", NOW), Ok(8.0));
    }
    #[test]
    fn queue_is_stable_and_loses_closed_evidence_on_activity() {
        let mut d = runtime(Mode::Wait);
        let first = advance_pending(&mut d, "A", "B", NOW, ProcessState::Running);
        assert!(first.closed_since.is_none());
        let closed = advance_pending(&mut d, "A", "B", NOW + 1, ProcessState::Closed);
        assert_eq!(first.id, closed.id);
        assert_eq!(closed.closed_since, Some(NOW + 1));
        let unknown = advance_pending(&mut d, "A", "B", NOW + 5, ProcessState::Unknown);
        assert!(unknown.closed_since.is_none());
        let changed = advance_pending(&mut d, "A", "C", NOW + 6, ProcessState::Closed);
        assert_ne!(changed.id, first.id);
        assert_eq!(changed.closed_since, Some(NOW + 6));
    }
    #[test]
    fn commit_revalidates_cancel_source_config_pool_and_process() {
        let mut d = runtime(Mode::Wait);
        let p = advance_pending(&mut d, "A", "B", NOW, ProcessState::Closed);
        let a = account_fixture("A", 0.08, 0.5);
        let mut b = account_fixture("B", 0.8, 0.8);
        let c = config(Mode::Wait);
        let validate = |d: &RuntimeData, b: &Account, current, process| {
            commit_guard(d, &p, &c, current, &a, b, NOW, process)
        };
        assert_eq!(validate(&d, &b, Some("A"), ProcessState::Closed), Ok(()));
        assert_eq!(
            validate(&d, &b, Some("C"), ProcessState::Closed),
            Err("source_changed")
        );
        assert_eq!(
            validate(&d, &b, Some("A"), ProcessState::Running),
            Err("clients_running")
        );
        assert_eq!(
            validate(&d, &b, Some("A"), ProcessState::Unknown),
            Err("process_unknown")
        );
        b.quota = Some(quota(0.2, 0.9));
        assert_eq!(
            validate(&d, &b, Some("A"), ProcessState::Closed),
            Err("no_candidate")
        );
        b.quota = Some(quota(0.8, 0.8));
        b.disabled = true;
        assert_eq!(
            validate(&d, &b, Some("A"), ProcessState::Closed),
            Err("no_candidate")
        );
        b.disabled = false;
        d.revision += 1;
        assert_eq!(
            validate(&d, &b, Some("A"), ProcessState::Closed),
            Err("request_changed")
        );
        d.revision -= 1;
        d.pending = None;
        assert_eq!(
            validate(&d, &b, Some("A"), ProcessState::Closed),
            Err("request_changed")
        );
    }
    #[test]
    fn expired_quota_or_recovered_source_cannot_commit() {
        let mut d = runtime(Mode::Wait);
        let p = advance_pending(&mut d, "A", "B", NOW, ProcessState::Closed);
        let mut a = account_fixture("A", 0.08, 0.5);
        let b = account_fixture("B", 0.8, 0.8);
        assert_eq!(
            commit_guard(
                &d,
                &p,
                &d.config,
                Some("A"),
                &a,
                &b,
                NOW + 181,
                ProcessState::Closed
            ),
            Err("no_quota")
        );
        a.quota = Some(quota(0.9, 0.9));
        assert_eq!(
            commit_guard(
                &d,
                &p,
                &d.config,
                Some("A"),
                &a,
                &b,
                NOW,
                ProcessState::Closed
            ),
            Err("no_quota")
        );
    }
    #[test]
    fn both_modes_fake_end_to_end_only_commit_after_client_exit() {
        for mode in [Mode::Wait, Mode::Stop] {
            let home = tempfile::tempdir().unwrap();
            let session = home.path().join("native-session");
            let a = account_fixture("A", 0.08, 0.5);
            let b = account_fixture("B", 0.8, 0.8);
            cli_credentials::write_session(&session, &cli_credentials::payload(&a.token).unwrap())
                .unwrap();
            let original = std::fs::read(&session).unwrap();
            let mut d = runtime(mode);
            let mut writes = 0;
            // Native Stop alone does not exit the process. Both modes stay pending.
            for state in [
                ProcessState::Running,
                ProcessState::Unknown,
                ProcessState::Running,
            ] {
                let p = advance_pending(&mut d, "A", "B", NOW, state);
                assert!(commit_guard(&d, &p, &d.config, Some("A"), &a, &b, NOW, state).is_err());
                assert_eq!(std::fs::read(&session).unwrap(), original);
            }
            let first = advance_pending(&mut d, "A", "B", NOW + 1, ProcessState::Closed);
            assert_eq!(first.closed_since, Some(NOW + 1));
            let pending = advance_pending(&mut d, "A", "B", NOW + 5, ProcessState::Closed);
            assert!(NOW + 5 - pending.closed_since.unwrap() >= 3);
            commit_guard(
                &d,
                &pending,
                &d.config,
                Some("A"),
                &a,
                &b,
                NOW + 5,
                ProcessState::Closed,
            )
            .unwrap();
            cli_credentials::write_session(&session, &cli_credentials::payload(&b.token).unwrap())
                .unwrap();
            writes += 1;
            d.pending = None;
            assert!(commit_guard(
                &d,
                &pending,
                &d.config,
                Some("B"),
                &a,
                &b,
                NOW + 6,
                ProcessState::Closed
            )
            .is_err());
            let written: serde_json::Value =
                serde_json::from_slice(&std::fs::read(session).unwrap()).unwrap();
            assert_eq!(written["token"]["refresh_token"], "fixture-B-refresh");
            assert_eq!(writes, 1);
            assert!(!home.path().join("oauth_creds.json").exists());
        }
    }
    #[test]
    fn native_file_absence_is_distinct_from_invalid_existing_path() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("native");
        assert_eq!(native_session_exists(&path), Ok(false));
        std::fs::create_dir(&path).unwrap();
        assert_eq!(native_session_exists(&path), Err("credentials_changed"));
    }
    #[test]
    fn pause_record_roundtrip_contains_no_credentials() {
        let pause = PauseRecord {
            source_id: Some("A".into()),
            failed: true,
        };
        let encoded = serde_json::to_string(&pause).unwrap();
        let restored: PauseRecord = serde_json::from_str(&encoded).unwrap();
        assert_eq!(restored.source_id.as_deref(), Some("A"));
        assert!(restored.failed);
        assert!(!encoded.contains("token"));
        assert!(!encoded.contains("email"));
    }
}
