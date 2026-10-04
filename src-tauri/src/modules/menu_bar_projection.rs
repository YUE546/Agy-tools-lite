//! Credential-free native menu projection. Percentages are relative headroom,
//! never token capacity; keep independent families/windows and fail closed.
use super::account_dashboard::DashboardEntry;
use crate::models::config::MenuBarQuotaScope;
use std::collections::HashMap;
use crate::models::config::{MenuBarPreferences, MenuBarLabelStyle};

#[derive(Debug, PartialEq, Eq)]
pub enum QuotaTone { Healthy, Warning, Critical, Unknown }
pub fn quota_tone(value: Option<f64>, preferences: &MenuBarPreferences) -> QuotaTone {
    match value { Some(value) if value.is_finite() && (0.0..=100.0).contains(&value) => {
        if value > f64::from(preferences.green_above) { QuotaTone::Healthy }
        else if value < f64::from(preferences.red_below) { QuotaTone::Critical } else { QuotaTone::Warning }
    }, _ => QuotaTone::Unknown }
}
pub fn visible_account(account: &DashboardEntry, preferences: &MenuBarPreferences, now: i64) -> bool {
    !preferences.hide_unavailable || (account.read_status == "loaded" && !account.disabled
        && !account.quota.as_ref().is_some_and(|quota| quota.is_forbidden)
        && !(account.validation_blocked && account.validation_blocked_until.is_none_or(|until| until > now)))
}
pub fn identity_parts(account: &DashboardEntry, preferences: &MenuBarPreferences) -> (String, String) {
    let note = account.custom_label.as_deref().unwrap_or("").trim();
    match preferences.label_style {
        MenuBarLabelStyle::LabelThenEmail if !note.is_empty() => (note.into(), account.email.clone()),
        MenuBarLabelStyle::EmailThenLabel if !note.is_empty() && note != account.email => (account.email.clone(), note.into()),
        _ => (account.email.clone(), String::new()),
    }
}

pub fn account_windows(account: &DashboardEntry, now: i64, freshness_minutes: i32) -> [[Option<f64>; 2]; 2] {
    let unavailable = account.read_status != "loaded" || account.disabled
        || (account.validation_blocked && account.validation_blocked_until.is_none_or(|until| until > now))
        || !account.protected_models.is_empty();
    let Some(quota) = &account.quota else { return [[None; 2]; 2]; };
    let freshness = i64::from(freshness_minutes.max(1)) * 60;
    if unavailable || quota.is_forbidden || quota.last_updated <= 0
        || quota.last_updated > now + 300 || now - quota.last_updated > freshness {
        return [[None; 2]; 2];
    }
    let mut result = [[None; 2]; 2];
    for (period, window) in ["5h", "weekly"].iter().enumerate() {
        for family in 0..2 {
            let mut rows: HashMap<&str, (f64, &str)> = HashMap::new();
            let mut owners: HashMap<&str, usize> = HashMap::new();
            let mut invalid = false;
            for group in quota.groups.iter().flatten() {
                let name = group.display_name.to_lowercase();
                let owner = if name.contains("gemini") { 0 } else if name.contains("claude") || name.contains("gpt") { 1 } else { continue };
                for bucket in &group.buckets {
                    if bucket.window.trim().to_lowercase() != *window { continue; }
                    if bucket.bucket_id.is_empty() { if owner == family { invalid = true; } continue; }
                    if let Some(previous) = owners.insert(&bucket.bucket_id, owner) {
                        if previous != owner { invalid = true; }
                    }
                    if owner != family { continue; }
                    let valid_reset = chrono::DateTime::parse_from_rfc3339(&bucket.reset_time)
                        .is_ok_and(|time| time.timestamp() > now);
                    let Some(value) = bucket.remaining_fraction.filter(|value| value.is_finite() && (0.0..=1.0).contains(value)) else {
                        invalid = true; continue;
                    };
                    if !valid_reset { invalid = true; continue; }
                    if let Some((previous, reset)) = rows.insert(&bucket.bucket_id, (value, &bucket.reset_time)) {
                        if previous != value || reset != bucket.reset_time { invalid = true; }
                    }
                }
            }
            if !invalid && !rows.is_empty() {
                result[period][family] = Some(rows.values().map(|(value, _)| value * 100.0).sum::<f64>() / rows.len() as f64);
            }
        }
    }
    result
}

pub fn aggregate(windows: &[[[Option<f64>; 2]; 2]], scope: MenuBarQuotaScope, period: usize, reserve: u8) -> (Option<f64>, usize, usize) {
    let families: &[usize] = match scope { MenuBarQuotaScope::All => &[0, 1], MenuBarQuotaScope::Gemini => &[0], MenuBarQuotaScope::Other => &[1] };
    let covered: Vec<_> = windows.iter().filter(|row| families.iter().all(|&family| row[period][family].is_some())).collect();
    let usable = covered.iter().filter(|row| families.iter().all(|&family| row[period][family].unwrap() > f64::from(reserve))).count();
    let values: Vec<_> = covered.iter().flat_map(|row| families.iter().map(|&family| row[period][family].unwrap())).collect();
    ((!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64), usable, covered.len())
}

pub fn percent(value: Option<f64>) -> String {
    match value { None => "—".into(), Some(value) => format!("{value:.0}%") }
}
pub fn reset_summary(value: &str, now: i64, zh: bool) -> String {
    let Ok(time) = chrono::DateTime::parse_from_rfc3339(value) else { return if zh { "重置时间未报告" } else { "Reset not reported" }.into(); };
    let seconds = time.timestamp() - now;
    if seconds <= 0 { return if zh { "已到重置时间，请刷新" } else { "Reset due; refresh" }.into(); }
    let minutes = (seconds + 59) / 60;
    let remaining = if minutes >= 1440 { format!("{}d {}h", minutes / 1440, minutes % 1440 / 60) }
        else if minutes >= 60 { format!("{}h {}m", minutes / 60, minutes % 60) }
        else { format!("{minutes}m") };
    if zh { format!("{remaining} 后重置") } else { format!("Resets in {remaining}") }
}
/// Compact inline reset labels retain the row geometry during hover.
pub fn account_reset_labels(account: &DashboardEntry, now: i64, zh: bool) -> [[String; 2]; 2] {
    std::array::from_fn(|period| std::array::from_fn(|family| {
        if account.disabled { return if zh { "禁用" } else { "Disabled" }.into(); }
        let window = if period == 0 { "5h" } else { "weekly" };
        let resets: std::collections::BTreeSet<_> = account.quota.iter().flat_map(|quota| quota.groups.iter().flatten())
            .filter(|group| { let name = group.display_name.to_lowercase(); if family == 0 { name.contains("gemini") } else { name.contains("claude") || name.contains("gpt") } })
            .flat_map(|group| &group.buckets).filter(|bucket| bucket.window.trim().eq_ignore_ascii_case(window)).map(|bucket| bucket.reset_time.as_str()).collect();
        if resets.len() > 1 { return if zh { "多组" } else { "Multiple" }.into(); }
        let summary = reset_summary(resets.first().copied().unwrap_or(""), now, false);
        if let Some(countdown) = summary.strip_prefix("Resets in ") { countdown.into() }
        else if summary.starts_with("Reset due") { if zh { "请刷新" } else { "Refresh" }.into() }
        else { if zh { "未报告" } else { "Unknown" }.into() }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::account_dashboard::{ReadOnlyQuota, ReadOnlyGroup, ReadOnlyBucket};
    fn account() -> DashboardEntry {
        let groups = ["Gemini", "Claude / GPT"].into_iter().enumerate().map(|(family, name)| ReadOnlyGroup {
            display_name: name.into(), buckets: ["5h", "weekly"].into_iter().map(|window| ReadOnlyBucket {
                bucket_id: format!("pool-{family}"), window: window.into(), remaining_fraction: Some(0.5), reset_time: "2030-01-01T00:00:00Z".into(),
            }).collect(),
        }).collect();
        DashboardEntry { id: "synthetic".into(), email: "test@example.invalid".into(), name: None, custom_label: None,
            read_status: "loaded", read_error: None, disabled: false, validation_blocked: false, validation_blocked_until: None, protected_models: vec![],
            quota: Some(ReadOnlyQuota { last_updated: 1_790_000_000, is_forbidden: false, subscription_tier: None, provenance: "observed", models: vec![], groups: Some(groups) }) }
    }
    #[test] fn quota_colors_include_both_boundaries_in_yellow() {
        let preferences = MenuBarPreferences::default();
        for (value, tone) in [(0.0, QuotaTone::Critical), (19.9, QuotaTone::Critical), (20.0, QuotaTone::Warning), (60.0, QuotaTone::Warning), (60.1, QuotaTone::Healthy), (100.0, QuotaTone::Healthy)] {
            assert_eq!(quota_tone(Some(value), &preferences), tone);
        }
        for value in [None, Some(f64::NAN), Some(-1.0), Some(101.0)] { assert_eq!(quota_tone(value, &preferences), QuotaTone::Unknown); }
        let custom = MenuBarPreferences { green_above: 80, red_below: 30, ..preferences };
        assert_eq!(quota_tone(Some(70.0), &custom), QuotaTone::Warning);
    }
    #[test] fn reset_labels_are_short_and_never_expose_raw_timestamps() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-10-05T00:00:00Z").unwrap().timestamp();
        assert_eq!(reset_summary("2026-10-07T03:00:00Z", now, true), "2d 3h 后重置");
        assert_eq!(reset_summary("2026-10-05T00:00:01Z", now, false), "Resets in 1m");
        assert_eq!(reset_summary("bad", now, true), "重置时间未报告");
        assert_eq!(reset_summary("2026-10-05T00:00:00Z", now, true), "已到重置时间，请刷新");
    }
    #[test] fn hover_resets_keep_independent_windows_and_disabled_state() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-10-05T00:00:00Z").unwrap().timestamp();
        let mut account = account();
        let groups = account.quota.as_mut().unwrap().groups.as_mut().unwrap();
        groups[0].buckets[0].reset_time = "2026-10-05T04:00:00Z".into();
        groups[0].buckets[1].reset_time = "2026-10-07T03:00:00Z".into();
        assert_eq!(account_reset_labels(&account, now, false)[0][0], "4h 0m");
        assert_eq!(account_reset_labels(&account, now, true)[1][0], "2d 3h");
        account.disabled = true;
        assert!(account_reset_labels(&account, now, true).iter().flatten().all(|label| label == "禁用"));
        account.disabled = false; account.quota = None;
        assert!(account_reset_labels(&account, now, false).iter().flatten().all(|label| label == "Unknown"));
    }
    #[test] fn visibility_hides_invalid_accounts_without_hiding_missing_quota() {
        let preferences = MenuBarPreferences::default(); let mut account = account();
        account.quota = None;
        assert!(visible_account(&account, &preferences, 100));
        account.disabled = true; assert!(!visible_account(&account, &preferences, 100));
        assert!(visible_account(&account, &MenuBarPreferences { hide_unavailable: false, ..preferences.clone() }, 100));
        account.disabled = false; account.validation_blocked = true;
        account.validation_blocked_until = Some(101); assert!(!visible_account(&account, &preferences, 100));
        account.validation_blocked_until = Some(100); assert!(visible_account(&account, &preferences, 100));
        account.read_status = "failed"; assert!(!visible_account(&account, &preferences, 100));
        account = self::account(); account.quota.as_mut().unwrap().is_forbidden = true;
        assert!(!visible_account(&account, &preferences, 100));
    }
    #[test] fn email_precedes_note_by_default_and_formats_are_selectable() {
        let mut account = account(); account.custom_label = Some("  Account one  ".into());
        let mut preferences = MenuBarPreferences::default();
        assert_eq!(identity_parts(&account, &preferences), (account.email.clone(), "Account one".into()));
        preferences.label_style = MenuBarLabelStyle::LabelThenEmail;
        assert_eq!(identity_parts(&account, &preferences), ("Account one".into(), account.email.clone()));
        preferences.label_style = MenuBarLabelStyle::EmailOnly;
        assert_eq!(identity_parts(&account, &preferences), (account.email.clone(), String::new()));
    }
    #[test] fn families_and_windows_never_borrow_missing_data() {
        let mut account = account();
        account.quota.as_mut().unwrap().groups.as_mut().unwrap()[0].buckets.remove(0);
        assert_eq!(account_windows(&account, 1_790_000_000, 15), [[None, Some(50.0)], [Some(50.0), Some(50.0)]]);
    }
    #[test] fn duplicate_pools_do_not_multiply_weight_and_conflicts_fail_closed() {
        let mut account = account();
        let groups = account.quota.as_mut().unwrap().groups.as_mut().unwrap();
        let duplicate = ReadOnlyGroup { display_name: "Gemini duplicate".into(), buckets: vec![ReadOnlyBucket {
            bucket_id: "pool-0".into(), window: "5h".into(), remaining_fraction: Some(0.5), reset_time: "2030-01-01T00:00:00Z".into(),
        }] };
        groups.push(duplicate);
        assert_eq!(account_windows(&account, 1_790_000_000, 15)[0][0], Some(50.0));
        account.quota.as_mut().unwrap().groups.as_mut().unwrap()[2].buckets[0].remaining_fraction = Some(0.2);
        assert_eq!(account_windows(&account, 1_790_000_000, 15)[0][0], None);
    }
    #[test] fn ambiguous_ownership_unknown_and_expired_observations_are_unavailable() {
        let mut account = account();
        account.quota.as_mut().unwrap().groups.as_mut().unwrap()[1].buckets[0].bucket_id = "pool-0".into();
        assert_eq!(account_windows(&account, 1_790_000_000, 15)[0], [None, None]);
        let mut account = self::account();
        account.quota.as_mut().unwrap().groups.as_mut().unwrap()[0].buckets[0].remaining_fraction = None;
        account.quota.as_mut().unwrap().groups.as_mut().unwrap()[1].buckets[1].reset_time = "2020-01-01T00:00:00Z".into();
        let result = account_windows(&account, 1_790_000_000, 15);
        assert_eq!(result[0][0], None); assert_eq!(result[1][1], None);
    }
    #[test] fn unavailable_accounts_and_stale_caches_are_excluded() {
        let mut account = account(); account.disabled = true;
        assert_eq!(account_windows(&account, 1_790_000_000, 15), [[None; 2]; 2]); account.disabled = false;
        assert_eq!(account_windows(&account, 1_790_001_000, 15), [[None; 2]; 2]);
        account.quota.as_mut().unwrap().last_updated += 301;
        assert_eq!(account_windows(&account, 1_790_000_000, 15), [[None; 2]; 2]);
    }
    #[test] fn aggregation_keeps_known_zero_and_counts_availability_separately() {
        let windows = [[[Some(100.0), Some(50.0)], [Some(0.0), Some(50.0)]], [[None, Some(50.0)], [Some(100.0), Some(100.0)]]];
        assert_eq!(aggregate(&windows, MenuBarQuotaScope::All, 0, 10), (Some(75.0), 1, 1));
        assert_eq!(aggregate(&windows, MenuBarQuotaScope::Gemini, 1, 10), (Some(50.0), 1, 2));
        assert_eq!(aggregate(&windows, MenuBarQuotaScope::Other, 0, 10), (Some(50.0), 2, 2));
        assert_eq!(percent(Some(0.02)), "0%"); assert_eq!(percent(Some(99.6)), "100%"); assert_eq!(percent(None), "—");
    }
}
