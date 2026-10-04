export interface DesktopPreferences {
    launch_at_login: boolean;
    hide_dock_icon: boolean;
    start_minimized: boolean;
}

export type MenuBarQuotaScope = 'all' | 'gemini' | 'other';
export type MenuBarResetTimeDisplay = 'hidden' | 'hover' | 'always';
export interface MenuBarPreferences {
    quota_scope: MenuBarQuotaScope;
    display_scope?: MenuBarQuotaScope;
    hide_unavailable?: boolean;
    label_style?: 'email_then_label' | 'label_then_email' | 'email_only';
    show_aggregate?: boolean;
    show_session?: boolean;
    show_weekly?: boolean;
    show_icons?: boolean;
    show_reset_on_hover?: boolean;
    reset_time_display?: MenuBarResetTimeDisplay | null;
    green_above?: number;
    red_below?: number;
}
export const DEFAULT_MENU_BAR_PREFERENCES = { quota_scope: 'all', display_scope: 'all', hide_unavailable: true,
    label_style: 'email_then_label', show_aggregate: true, show_session: true, show_weekly: true,
    show_icons: true, show_reset_on_hover: true, green_above: 60, red_below: 20 } as const satisfies MenuBarPreferences;
export const menuBarResetTimeDisplay = (preferences: MenuBarPreferences): MenuBarResetTimeDisplay =>
    preferences.reset_time_display ?? (preferences.show_reset_on_hover === false ? 'hidden' : 'hover');

export interface QuotaProtectionConfig {
    enabled: boolean;
    threshold_percentage: number;
    monitored_models: string[];
}

export interface PinnedQuotaModelsConfig {
    models: string[];
}

export interface AppConfig {
    desktop?: DesktopPreferences;
    menu_bar?: MenuBarPreferences;
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
