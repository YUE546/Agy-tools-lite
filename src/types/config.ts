export interface QuotaProtectionConfig {
    enabled: boolean;
    threshold_percentage: number;
    monitored_models: string[];
}

export interface PinnedQuotaModelsConfig {
    models: string[];
}

export interface AppConfig {
    language: string;
    theme: string;
    auto_refresh: boolean;
    refresh_interval: number;
    auto_sync: boolean;
    sync_interval: number;
    antigravity_executable?: string;
    antigravity_ide_executable?: string;
    antigravity_args?: string[];
    quota_protection: QuotaProtectionConfig;
    pinned_quota_models: PinnedQuotaModelsConfig;
}
