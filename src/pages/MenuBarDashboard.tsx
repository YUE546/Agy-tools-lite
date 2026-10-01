import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  Check,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  CircleAlert,
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
  quotaPages,
  pageSlice,
  pageSizeForHeight,
} from "../utils/menuBarQuota";
import { getMenuBarMessages } from "../components/menubar/messages";
import "../components/menubar/MenuBarDashboard.css";

interface MenuBarAppearance {
  native_material: boolean;
  reduced_transparency: boolean;
  high_contrast: boolean;
  platform: string;
}

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
  const [quotaPage, setQuotaPage] = useState(0);
  const [panelHeight, setPanelHeight] = useState(window.innerHeight);
  const [accountPage, setAccountPage] = useState(0);
  const [usage, setUsage] = useState<LocalUsage | null>(null);
  const [loading, setLoading] = useState(true);
  const [appearance, setAppearance] = useState<MenuBarAppearance | null>(null);
  const [operation, setOperation] = useState<string | null>(null);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    const resize = () => setPanelHeight(window.innerHeight);
    window.addEventListener("resize", resize);
    return () => window.removeEventListener("resize", resize);
  }, []);
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
  const loadAppearance = useCallback(() => {
    if (isTauri())
      void request<MenuBarAppearance>("get_menu_bar_appearance")
        .then(setAppearance)
        .catch(() => setAppearance(null));
  }, []);
  useEffect(() => {
    void reload();
    void loadUsage();
    loadAppearance();
    if (!isTauri()) return;
    const listeners = [
      listen<MenuBarAppearance>("menubar://appearance", (event) =>
        setAppearance(event.payload),
      ),
      listen("menubar://opened", () => {
        setNotice("");
        setNow(Date.now());
        loadAppearance();
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
  }, [reload, loadUsage, loadAppearance]);
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
      setAccountPickerOpen(false);
      setQuotaPage(0);
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
        : i18n.language.startsWith("zh")
          ? `${relativeDuration(timestamp - now)}后重置`
          : `Resets in ${relativeDuration(timestamp - now)}`;
  };
  const classForQuota = (value: number | null) =>
    value === null
      ? "unknown"
      : value <= 10
        ? "critical"
        : value <= 25
          ? "warning"
          : "healthy";

  const pageSize = pageSizeForHeight(panelHeight);
  const pages = quotaPages(groups, pageSize);
  const selectedQuotaPage = Math.min(quotaPage, Math.max(pages.length - 1, 0));
  const visibleRows = pages[selectedQuotaPage] || [];
  const selectedAccountPage = Math.min(
    accountPage,
    Math.max(Math.ceil(accounts.length / pageSize) - 1, 0),
  );
  const visibleAccounts = pageSlice(accounts, selectedAccountPage, pageSize);
  const chinese = i18n.language.startsWith("zh");
  const viewLabel = accountPickerOpen
    ? chinese
      ? "返回额度"
      : "Back to quota"
    : chinese
      ? "切换账号"
      : "Switch account";
  const heading = chinese ? "剩余额度" : "Remaining quota";
  const toggleAccounts = () => {
    setAccountPickerOpen((value) => !value);
    setAccountPage(0);
  };
  const pager = (
    page: number,
    total: number,
    setPage: (value: number) => void,
    label: string,
  ) =>
    total > 1 ? (
      <div className="mb-page-controls" aria-label={label}>
        <button
          type="button"
          aria-label={chinese ? "上一页" : "Previous page"}
          disabled={page === 0}
          onClick={() => setPage(page - 1)}
        >
          <ChevronLeft size={13} />
        </button>
        <span>
          {page + 1} / {total}
        </span>
        <button
          type="button"
          aria-label={chinese ? "下一页" : "Next page"}
          disabled={page + 1 >= total}
          onClick={() => setPage(page + 1)}
        >
          <ChevronRight size={13} />
        </button>
      </div>
    ) : null;

  return (
    <div
      className={`menubar-app mb-native ${appearance?.native_material ? "is-material" : "is-solid"} ${appearance?.high_contrast ? "is-high-contrast" : ""}`}
      data-view={accountPickerOpen ? "accounts" : "quota"}
    >
      <header className="mb-native-header">
        <div className="mb-native-identity">
          <div className="mb-native-product">
            Antigravity{" "}
            <span>{current?.quota?.subscription_tier || "Lite"}</span>
          </div>
          <button
            type="button"
            className="mb-native-account"
            aria-label={t.accounts}
            aria-expanded={accountPickerOpen}
            disabled={!accounts.length}
            onClick={toggleAccounts}
            title={current?.email}
          >
            <strong>
              {current?.custom_label || current?.email || t.noAccount}
            </strong>
            <ChevronDown size={13} />
          </button>
        </div>
        <button
          type="button"
          className="mb-native-icon"
          aria-label={t.refresh}
          title={t.refresh}
          disabled={Boolean(operation) || !current || !isTauri()}
          onClick={() => void refresh()}
        >
          <RefreshCw
            size={15}
            className={operation === "refresh" ? "mb-spin" : ""}
          />
        </button>
        <button
          type="button"
          className="mb-native-icon"
          aria-label={t.close}
          title={`${t.close} · Esc`}
          onClick={hide}
        >
          <X size={15} />
        </button>
      </header>
      <div
        className={`mb-native-status ${error ? "has-error" : notice ? "has-notice" : ""}`}
        role={error ? "alert" : "status"}
        title={error || undefined}
      >
        {error ? (
          <>
            <CircleAlert size={11} />
            <span>{error}</span>
            <button
              onClick={() => {
                setError("");
                void reload();
              }}
            >
              {t.retry}
            </button>
          </>
        ) : notice ? (
          <>
            <Check size={11} />
            <span>{notice}</span>
          </>
        ) : (
          <>
            <span className={`mb-status-dot ${stale ? "is-stale" : ""}`} />
            <span>
              {operation && operation !== "refresh"
                ? t.switching
                : stale && current
                  ? `${t.stale} · ${updated}`
                  : updated}
            </span>
          </>
        )}
      </div>
      <main className="mb-native-main">
        {!isTauri() ? (
          <div className="mb-native-empty">
            <CircleAlert size={23} />
            <strong>{t.offline}</strong>
            <p>{t.nativeOnly}</p>
          </div>
        ) : loading ? (
          <div className="mb-native-empty">
            <Loader2 size={21} className="mb-spin" />
            <span>{t.loading}</span>
          </div>
        ) : accountPickerOpen ? (
          <section className="mb-native-picker" aria-label={t.accounts}>
            <div className="mb-native-section-label">
              <h2>
                {chinese ? "选择账号" : "Choose account"}{" "}
                <span>{accounts.length}</span>
              </h2>
              {pager(
                selectedAccountPage,
                Math.ceil(accounts.length / pageSize),
                setAccountPage,
                t.accounts,
              )}
            </div>
            <div
              className="mb-native-account-list"
              style={{ "--account-page-size": pageSize } as React.CSSProperties}
            >
              {visibleAccounts.map((account) => {
                const active = account.id === current?.id;
                const allowed = isAccountSwitchable(account, now);
                const remaining = lowestKnownQuota(account);
                return (
                  <button
                    type="button"
                    key={account.id}
                    className={`mb-native-option ${active ? "is-active" : ""}`}
                    disabled={active || !allowed || Boolean(operation)}
                    onClick={() => void switchAccount(account)}
                    title={
                      !allowed
                        ? account.disabled_reason ||
                          account.validation_blocked_reason ||
                          t.disabled
                        : account.email
                    }
                  >
                    <span className="mb-native-option-check">
                      {operation === account.id ? (
                        <Loader2 size={13} className="mb-spin" />
                      ) : active ? (
                        <Check size={13} />
                      ) : null}
                    </span>
                    <span className="mb-native-option-text">
                      <strong>
                        {account.custom_label || account.name || account.email}
                      </strong>
                      <span>
                        {allowed
                          ? account.email
                          : account.disabled
                            ? t.disabled
                            : t.verification}
                      </span>
                    </span>
                    <span
                      className={`mb-native-balance ${classForQuota(remaining)}`}
                      title={t.quotaLower}
                    >
                      {remaining === null ? "—" : `${remaining}%`}
                    </span>
                  </button>
                );
              })}
            </div>
            <div className="mb-native-picker-bottom">
              <p>{t.switchHint}</p>
              <button type="button" onClick={() => open("accounts")}>
                {t.addAccount}
                <ExternalLink size={12} />
              </button>
            </div>
          </section>
        ) : (
          <>
            <section className="mb-native-quotas" aria-label={heading}>
              <div className="mb-native-section-label">
                <h2>{heading}</h2>
                {pager(selectedQuotaPage, pages.length, setQuotaPage, heading)}
              </div>
              {!current ? (
                <div className="mb-native-empty">
                  <Users size={23} />
                  <strong>{t.noAccount}</strong>
                  <p>{t.noAccountHint}</p>
                  <button type="button" onClick={() => open("accounts")}>
                    {t.addAccount}
                    <ChevronRight size={12} />
                  </button>
                </div>
              ) : current.quota?.is_forbidden ? (
                <div className="mb-native-empty">
                  <CircleAlert size={21} />
                  <p>{t.forbidden}</p>
                </div>
              ) : visibleRows.length ? (
                <div
                  className="mb-native-quota-list"
                  style={
                    {
                      "--quota-rows": visibleRows.length,
                    } as React.CSSProperties
                  }
                >
                  {visibleRows.map((row) => (
                    <div className="mb-native-quota" key={row.id}>
                      <div className="mb-native-quota-title">
                        <div>
                          <span className="mb-native-model" title={row.group}>
                            {row.group.replace(/ models?$/i, "")}
                          </span>
                          {row.window && (
                            <span className="mb-native-window">
                              {row.window === "5h"
                                ? t.session
                                : row.window === "weekly"
                                  ? t.weekly
                                  : row.label}
                            </span>
                          )}
                        </div>
                        <strong>
                          {row.remaining === null ? "—" : `${row.remaining}%`}
                        </strong>
                      </div>
                      <div
                        className={`mb-native-progress ${classForQuota(row.remaining)}`}
                        role={
                          row.remaining === null ? undefined : "progressbar"
                        }
                        aria-label={`${row.group} ${row.label} ${t.remaining}`}
                        aria-valuenow={row.remaining ?? undefined}
                        aria-valuemin={0}
                        aria-valuemax={100}
                      >
                        <span style={{ width: `${row.remaining ?? 0}%` }} />
                      </div>
                      <div
                        className="mb-native-reset"
                        title={
                          row.resetTime &&
                          resetTimestamp(row.resetTime) !== null
                            ? new Date(row.resetTime).toLocaleString()
                            : undefined
                        }
                      >
                        {row.remaining === null
                          ? t.unknown
                          : resetLabel(row.resetTime)}
                      </div>
                    </div>
                  ))}
                </div>
              ) : (
                <div className="mb-native-empty">
                  <p>{t.noQuota}</p>
                </div>
              )}
            </section>
            <section className="mb-native-usage" aria-label={t.today}>
              <div>
                <h2>{chinese ? "今日用量" : "Today"}</h2>
                <span>{chinese ? "本机全部账号" : "All local accounts"}</span>
              </div>
              {usage ? (
                <div className="mb-native-usage-values">
                  <div>
                    <strong>
                      {shortNumber(usage.today.total_tokens, i18n.language)}
                    </strong>
                    <span>tokens</span>
                  </div>
                  <div>
                    <strong>
                      {shortNumber(usage.today.request_count, i18n.language)}
                    </strong>
                    <span>{t.requests}</span>
                  </div>
                </div>
              ) : (
                <span className="mb-native-usage-missing">{t.noUsage}</span>
              )}
            </section>
          </>
        )}
      </main>
      <footer className="mb-native-footer">
        <button
          type="button"
          className="mb-native-switch"
          disabled={!accounts.length || !isTauri()}
          onClick={toggleAccounts}
        >
          {accountPickerOpen ? <ChevronLeft size={13} /> : <Users size={13} />}
          {viewLabel}
        </button>
        <div className="mb-native-footer-right">
          <button
            type="button"
            className="mb-native-dashboard"
            disabled={!isTauri()}
            onClick={() => open("dashboard")}
          >
            {chinese ? "仪表盘" : "Dashboard"}
            <ExternalLink size={11} />
          </button>
          <span className="mb-native-divider" />
          <button
            type="button"
            className="mb-native-icon"
            title={t.settings}
            aria-label={t.settings}
            disabled={!isTauri()}
            onClick={() => open("settings")}
          >
            <Settings2 size={14} />
          </button>
          <button
            type="button"
            className="mb-native-icon"
            title={t.quit}
            aria-label={t.quit}
            disabled={!isTauri()}
            onClick={() => void runAction(() => request("quit_app"))}
          >
            <LogOut size={14} />
          </button>
        </div>
      </footer>
    </div>
  );
}
