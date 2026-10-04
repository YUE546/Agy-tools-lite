//! Credential-free native menu projection. Percentages are relative headroom,
//! never token capacity; keep independent families/windows and fail closed.
use super::account_dashboard::DashboardEntry;
use crate::models::config::MenuBarQuotaScope;
use std::collections::HashMap;

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
    match value { None => "—".into(), Some(value) if value > 0.0 && value < 1.0 => "<1%".into(),
        Some(value) if value > 99.0 && value < 100.0 => ">99%".into(), Some(value) => format!("{value:.0}%") }
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
        assert_eq!(percent(Some(0.02)), "<1%"); assert_eq!(percent(None), "—");
    }
}
