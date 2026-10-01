import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  Activity,
  ArrowRight,
  Check,
  ChevronRight,
  CircleAlert,
  Clock3,
  Cpu,
  ExternalLink,
  Loader2,
  LogOut,
  RefreshCw,
  Settings2,
  Users,
  X,
} from "lucide-react";
import { listen } from "@tauri-apps/api/event";
import { useTranslation } from "react-i18next";
import type { Account } from "../types/account";
import { useConfigStore } from "../stores/useConfigStore";
import { request } from "../utils/request";
import { isTauri } from "../utils/env";
import {
  compactQuotaGroups,
  isAccountSwitchable,
  isQuotaStale,
  lowestKnownQuota,
  resetTimestamp,
} from "../utils/menuBarQuota";
import { getMenuBarMessages } from "../components/menubar/messages";
import "../components/menubar/MenuBarDashboard.css";

interface LocalUsage {
  today: { total_tokens: number; request_count: number };
  unreadable_databases?: number;
}
const shortNumber = (value: number, language: string) =>
  new Intl.NumberFormat(language, {
    notation: "compact",
    maximumFractionDigits: 1,
  }).format(value);
const errorText = (error: unknown) =>
  typeof error === "string"
    ? error
    : error instanceof Error
      ? error.message
      : JSON.stringify(error);

export default function MenuBarDashboard() {
  const { i18n } = useTranslation();
  const t = getMenuBarMessages(i18n.language);
  const config = useConfigStore((state) => state.config);
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [current, setCurrent] = useState<Account | null>(null);
  const [accountPickerOpen, setAccountPickerOpen] = useState(false);
  const [usage, setUsage] = useState<LocalUsage | null>(null);
  const [loading, setLoading] = useState(true);
  const [operation, setOperation] = useState<string | null>(null);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [now, setNow] = useState(Date.now());
  const operationLock = useRef(false);
  const generation = useRef(0);
  const reload = useCallback(async () => {
    const requestId = ++generation.current;
    if (!isTauri()) {
      setLoading(false);
      return;
    }
    try {
      const [saved, active] = await Promise.all([
        request<Account[]>("list_accounts"),
        request<Account | null>("get_current_account"),
      ]);
      if (requestId !== generation.current) return;
      setAccounts(saved);
      setCurrent(
        active
          ? saved.find((account) => account.id === active.id) || active
          : null,
      );
      setNow(Date.now());
    } catch (e) {
      if (requestId === generation.current) setError(errorText(e));
    } finally {
      if (requestId === generation.current) setLoading(false);
    }
  }, []);
  const loadUsage = useCallback(async () => {
    if (!isTauri()) return;
    try {
      setUsage(await request<LocalUsage>("get_local_token_usage"));
    } catch {
      setUsage(null);
    }
  }, []);
  useEffect(() => {
    void reload();
    void loadUsage();
    if (!isTauri()) return;
    const listeners = [
      listen("menubar://opened", () => {
        setNotice("");
        setNow(Date.now());
        void reload();
        void loadUsage();
      }),
      listen("menubar://data-updated", () => {
        void reload();
      }),
      listen("accounts://refreshed", () => {
        void reload();
      }),
      listen<string>("menubar://error", (event) => setError(event.payload)),
    ];
    return () => {
      generation.current++;
      void Promise.all(listeners).then((unlisteners) =>
        unlisteners.forEach((unlisten) => unlisten()),
      );
    };
  }, [reload, loadUsage]);
  useEffect(() => {
    const timer = window.setInterval(() => {
      if (document.visibilityState !== "hidden") setNow(Date.now());
    }, 60_000);
    return () => window.clearInterval(timer);
  }, []);
  const runAction = useCallback(async (action: () => Promise<unknown>) => {
    try {
      await action();
    } catch (e) {
      setError(errorText(e));
    }
  }, []);
  const hide = useCallback(() => {
    if (isTauri()) void runAction(() => request("hide_menu_bar_dashboard"));
  }, [runAction]);
  useEffect(() => {
    const key = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        hide();
      }
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, [hide]);
  const open = (page: "dashboard" | "accounts" | "settings") =>
    void runAction(() => request("open_app_page", { page }));
  const refresh = async () => {
    if (operationLock.current || !current) return;
    operationLock.current = true;
    generation.current++;
    setOperation("refresh");
    setError("");
    setNotice("");
    try {
      await request("fetch_account_quota", { accountId: current.id });
      await reload();
      void loadUsage();
    } catch (e) {
      setError(`${t.lastError} ${errorText(e)}`);
    } finally {
      operationLock.current = false;
      setOperation(null);
    }
  };
  const switchAccount = async (account: Account) => {
    if (
      operationLock.current ||
      account.id === current?.id ||
      !isAccountSwitchable(account)
    )
      return;
    operationLock.current = true;
    generation.current++;
    setOperation(account.id);
    setError("");
    setNotice("");
    try {
      await request("switch_account", { accountId: account.id });
      await reload();
      setNotice(t.switched);
    } catch (e) {
      setError(errorText(e));
      await reload();
    } finally {
      operationLock.current = false;
      setOperation(null);
    }
  };
  const groups = useMemo(
    () =>
      compactQuotaGroups(current?.quota, config?.pinned_quota_models?.models),
    [current?.quota, config?.pinned_quota_models?.models],
  );
  const stale = isQuotaStale(
    current?.quota?.last_updated,
    config?.refresh_interval,
    now,
  );
  const relativeDuration = (ms: number) => {
    const minutes = Math.ceil(Math.max(ms, 0) / 60_000);
    if (minutes < 60) return `${minutes}${t.minutes}`;
    const hours = Math.floor(minutes / 60);
    if (hours < 24) return `${hours}${t.hours} ${minutes % 60}${t.minutes}`;
    return `${Math.floor(hours / 24)}${t.days} ${hours % 24}${t.hours}`;
  };
  const updated = current?.quota?.last_updated
    ? now - current.quota.last_updated * 1000 < 60_000
      ? t.justNow
      : `${relativeDuration(now - current.quota.last_updated * 1000)} ${t.ago}`
    : t.never;
  const resetLabel = (value: string) => {
    const timestamp = resetTimestamp(value);
    return timestamp === null
      ? t.unknownReset
      : timestamp <= now
        ? t.resetPending
        : `${t.resets} ${relativeDuration(timestamp - now)}`;
  };
  const classForQuota = (value: number | null) =>
    value === null
      ? "unknown"
      : value <= 10
        ? "critical"
        : value <= 25
          ? "warning"
          : "healthy";

  return (
    <div className="menubar-app">
      <header className="mb-header">
        <div className="mb-brand">
          <span className="mb-brand-mark">
            <Activity size={18} strokeWidth={2.3} />
          </span>
          <div>
            <h1>
              Antigravity <span>Lite</span>
            </h1>
            <p>{t.quick}</p>
          </div>
        </div>
        <div className="mb-header-actions">
          <button
            className="mb-icon-button"
            title={t.refresh}
            aria-label={t.refresh}
            disabled={Boolean(operation) || !current || !isTauri()}
            onClick={() => void refresh()}
          >
            <RefreshCw
              size={15}
              className={operation === "refresh" ? "mb-spin" : ""}
            />
          </button>
          <button
            className="mb-icon-button"
            title={`${t.close} · Esc`}
            aria-label={t.close}
            onClick={hide}
          >
            <X size={16} />
          </button>
        </div>
      </header>
      <main className="mb-scroll">
        {!isTauri() ? (
          <div className="mb-empty">
            <CircleAlert size={28} />
            <h2>{t.offline}</h2>
            <p>{t.nativeOnly}</p>
          </div>
        ) : loading ? (
          <div className="mb-loading">
            <Loader2 className="mb-spin" size={24} />
            <span>{t.loading}</span>
          </div>
        ) : (
          <>
            {error && (
              <div role="alert" className="mb-alert">
                <CircleAlert size={15} />
                <div>
                  <span>{error}</span>
                  <button
                    onClick={() => {
                      setError("");
                      void reload();
                    }}
                  >
                    {t.retry}
                  </button>
                </div>
              </div>
            )}
            {notice && (
              <div role="status" className="mb-notice">
                <Check size={14} />
                {notice}
              </div>
            )}
            {current ? (
              <section className="mb-account-card" aria-label={t.current}>
                <span className="mb-avatar">
                  {(current.custom_label || current.name || current.email)
                    .slice(0, 1)
                    .toUpperCase()}
                </span>
                <div className="mb-account-identity">
                  <div className="mb-account-heading">
                    <strong title={current.email}>
                      {current.custom_label || current.name || current.email}
                    </strong>
                    {current.quota?.subscription_tier && (
                      <span className="mb-tier">
                        {current.quota.subscription_tier}
                      </span>
                    )}
                  </div>
                  <span className="mb-email" title={current.email}>
                    {current.email}
                  </span>
                  <span className={`mb-updated ${stale ? "is-stale" : ""}`}>
                    <i />
                    {stale ? `${t.stale} · ` : ""}
                    {updated}
                  </span>
                </div>
                <button
                  type="button"
                  className="mb-picker-toggle"
                  aria-label={t.accounts}
                  aria-expanded={accountPickerOpen}
                  title={t.accounts}
                  onClick={() => setAccountPickerOpen((value) => !value)}
                >
                  <Users size={15} />
                  <ChevronRight
                    size={12}
                    style={{
                      transform: accountPickerOpen
                        ? "rotate(90deg)"
                        : undefined,
                    }}
                  />
                </button>
              </section>
            ) : (
              <div className="mb-empty">
                <Users size={30} />
                <h2>{t.noAccount}</h2>
                <p>{t.noAccountHint}</p>
                <button className="mb-primary" onClick={() => open("accounts")}>
                  {t.addAccount}
                  <ArrowRight size={14} />
                </button>
              </div>
            )}
            {current && !accountPickerOpen && (
              <section className="mb-quota-section" aria-label={t.quota}>
                <div className="mb-section-heading">
                  <h2>{t.quota}</h2>
                  <span>{t.remaining} %</span>
                </div>
                {current.quota?.is_forbidden ? (
                  <div className="mb-subtle-message">
                    <CircleAlert size={15} />
                    {t.forbidden}
                  </div>
                ) : groups.length ? (
                  <div className="mb-quota-groups">
                    {groups.map((group, groupIndex) => (
                      <div
                        className="mb-quota-card"
                        key={`${group.name}-${groupIndex}`}
                      >
                        <h3>{group.name}</h3>
                        {group.rows.map((row) => (
                          <div className="mb-quota-row" key={row.id}>
                            <div className="mb-quota-label">
                              <span>
                                {row.window === "5h"
                                  ? t.session
                                  : row.window === "weekly"
                                    ? t.weekly
                                    : row.window
                                      ? row.label
                                      : t.remaining}
                              </span>
                              <strong className={classForQuota(row.remaining)}>
                                {row.remaining === null
                                  ? "—"
                                  : `${row.remaining}%`}
                              </strong>
                            </div>
                            <div
                              className={`mb-progress ${classForQuota(row.remaining)}`}
                              role={
                                row.remaining === null
                                  ? undefined
                                  : "progressbar"
                              }
                              aria-label={`${group.name} ${row.label} ${t.remaining}`}
                              aria-valuenow={row.remaining ?? undefined}
                              aria-valuemin={0}
                              aria-valuemax={100}
                            >
                              <span
                                style={{
                                  width:
                                    row.remaining === null
                                      ? "0%"
                                      : `${row.remaining}%`,
                                }}
                              />
                            </div>
                            <div
                              className="mb-reset"
                              title={
                                row.resetTime
                                  ? new Date(row.resetTime).toLocaleString()
                                  : ""
                              }
                            >
                              <Clock3 size={10} />
                              {row.remaining === null
                                ? t.unknown
                                : resetLabel(row.resetTime)}
                            </div>
                          </div>
                        ))}
                      </div>
                    ))}
                  </div>
                ) : (
                  <div className="mb-subtle-message">{t.noQuota}</div>
                )}
              </section>
            )}
            {!accountPickerOpen && (
              <section className="mb-usage" aria-label={t.today}>
                <div className="mb-usage-title">
                  <Cpu size={12} />
                  <h2>{t.today}</h2>
                </div>
                {usage ? (
                  <div className="mb-usage-stats">
                    <div>
                      <strong>
                        {shortNumber(usage.today.total_tokens, i18n.language)}
                      </strong>
                      <span>{t.tokens}</span>
                    </div>
                    <div>
                      <strong>
                        {shortNumber(usage.today.request_count, i18n.language)}
                      </strong>
                      <span>{t.requests}</span>
                    </div>
                  </div>
                ) : (
                  <p className="mb-no-usage">{t.noUsage}</p>
                )}
                <p className="mb-scope">{t.localScope}</p>
              </section>
            )}
            <section className="mb-switcher" aria-label={t.accounts}>
              <div className="mb-section-heading">
                <h2>
                  {t.accounts}{" "}
                  <span className="mb-count">{accounts.length}</span>
                </h2>
                <button onClick={() => open("accounts")}>
                  {t.manage}
                  <ChevronRight size={12} />
                </button>
              </div>
              <div className="mb-account-list">
                {accounts.length ? (
                  accounts.map((account) => {
                    const active = account.id === current?.id;
                    const switching = operation === account.id;
                    const allowed = isAccountSwitchable(account, now);
                    const remaining = lowestKnownQuota(account);
                    return (
                      <button
                        key={account.id}
                        className={`mb-account-option ${active ? "is-active" : ""}`}
                        disabled={active || !allowed || Boolean(operation)}
                        onClick={() => void switchAccount(account)}
                        aria-label={`${account.custom_label || account.email} · ${active ? t.active : !allowed ? (account.disabled ? t.disabled : t.verification) : t.accounts}`}
                        title={
                          !allowed
                            ? account.disabled_reason ||
                              account.validation_blocked_reason ||
                              t.disabled
                            : `${t.quotaLower}: ${remaining === null ? t.unknown : `${remaining}%`}`
                        }
                      >
                        <span className="mb-small-avatar">
                          {(account.custom_label || account.email)
                            .slice(0, 1)
                            .toUpperCase()}
                        </span>
                        <span className="mb-option-name">
                          <strong>
                            {account.custom_label ||
                              account.name ||
                              account.email}
                          </strong>
                          <span>
                            {!allowed
                              ? account.disabled
                                ? t.disabled
                                : t.verification
                              : account.email}
                          </span>
                        </span>
                        <span
                          className={`mb-option-quota ${classForQuota(remaining)}`}
                        >
                          {switching ? (
                            <Loader2 size={14} className="mb-spin" />
                          ) : active ? (
                            <Check size={15} />
                          ) : remaining === null ? (
                            "—"
                          ) : (
                            `${remaining}%`
                          )}
                        </span>
                      </button>
                    );
                  })
                ) : (
                  <p className="mb-no-usage">{t.noAccounts}</p>
                )}
              </div>
              <p className="mb-switch-hint">
                {operation && operation !== "refresh"
                  ? t.switching
                  : t.switchHint}
              </p>
            </section>
          </>
        )}
      </main>
      <footer className="mb-footer">
        <button
          className="mb-dashboard-link"
          disabled={!isTauri()}
          onClick={() => open("dashboard")}
        >
          <ExternalLink size={13} />
          {t.dashboard}
        </button>
        <div>
          <button
            className="mb-icon-button"
            title={t.settings}
            aria-label={t.settings}
            disabled={!isTauri()}
            onClick={() => open("settings")}
          >
            <Settings2 size={16} />
          </button>
          <button
            className="mb-icon-button mb-quit"
            title={t.quit}
            aria-label={t.quit}
            disabled={!isTauri()}
            onClick={() => void runAction(() => request("quit_app"))}
          >
            <LogOut size={15} />
          </button>
        </div>
      </footer>
    </div>
  );
}
