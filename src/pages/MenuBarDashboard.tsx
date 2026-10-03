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
  summarizeAccounts,
  accountReadiness,
  quotaThreshold,
} from "../utils/menuBarQuota";
import { getMenuBarMessages } from "../components/menubar/messages";
import "../components/menubar/MenuBarDashboard.css";
import {
  MenuBarSwitchDetails,
  useMenuBarSwitchStatus,
} from "../components/menubar/LowQuotaStatus";

interface MenuBarAppearance {
  native_material: boolean;
  reduced_transparency: boolean;
  high_contrast: boolean;
  platform: string;
}

interface LocalUsage {
  today: { total_tokens: number; request_count: number };
  unreadable_databases?: number;
  skipped_large_records?: number;
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
  const { i18n, t: translate } = useTranslation();
  const lowQuota = useMenuBarSwitchStatus();
  const [switchDetailsOpen, setSwitchDetailsOpen] = useState(false);
  const t = getMenuBarMessages(i18n.language);
  const config = useConfigStore((state) => state.config);
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [current, setCurrent] = useState<Account | null>(null);
  const [selectedAccountId, setSelectedAccountId] = useState<string | null>(
    null,
  );
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
  const initialMount = useRef(true);
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
      const activeAccount = active
        ? saved.find((account) => account.id === active.id) || active
        : null;
      setCurrent(activeAccount);
      if (initialMount.current && activeAccount) {
        initialMount.current = false;
        setSelectedAccountId(activeAccount.id);
      }
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
      listen("tray://account-switched", () => {
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
  const viewedAccount =
    accounts.find((account) => account.id === selectedAccountId) || null;
  const isOverview = viewedAccount === null;
  const threshold = quotaThreshold(
    lowQuota.config?.enabled
      ? lowQuota.config.reserve_percentage
      : config?.quota_protection?.threshold_percentage,
  );
  const overview = useMemo(
    () => summarizeAccounts(accounts, threshold, config?.refresh_interval, now),
    [accounts, threshold, config?.refresh_interval, now],
  );
  // Viewing is deliberately separate from credential activation.
  const viewAccount = (accountId: string | null) => {
    setSwitchDetailsOpen(false);
    setSelectedAccountId(accountId);
    setAccountPickerOpen(false);
    setQuotaPage(0);
    setNotice("");
    setError("");
  };
  const refresh = async () => {
    if (operationLock.current || !accounts.length) return;
    operationLock.current = true;
    generation.current++;
    setOperation("refresh");
    setError("");
    setNotice("");
    try {
      if (viewedAccount) {
        await request("fetch_account_quota", { accountId: viewedAccount.id });
      } else {
        const stats = await request<{ failed: number }>("refresh_all_quotas");
        if (stats.failed)
          setError(
            i18n.language.startsWith("zh")
              ? `${stats.failed} 个账号刷新失败，仍显示已保存的数据。`
              : `${stats.failed} accounts could not refresh. Saved snapshots remain visible.`,
          );
      }
      await reload();
      void loadUsage();
    } catch (e) {
      setError(
        `${viewedAccount?.custom_label || viewedAccount?.email || (i18n.language.startsWith("zh") ? "全部账号" : "All accounts")}: ${t.lastError} ${errorText(e)}`,
      );
    } finally {
      operationLock.current = false;
      setOperation(null);
    }
  };
  const switchAccount = async (account: Account) => {
    if (
      operationLock.current ||
      lowQuota.status === null ||
      lowQuota.status?.phase === "switching" ||
      lowQuota.readError ||
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
      const name = account.custom_label || account.email;
      setNotice(
        i18n.language.startsWith("zh")
          ? `已切换到 ${name}`
          : `Switched to ${name}`,
      );
      // Do not close or replace a newer account-inspection view while this
      // background operation was in flight.
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
      compactQuotaGroups(
        viewedAccount?.quota,
        config?.pinned_quota_models?.models,
      ),
    [viewedAccount?.quota, config?.pinned_quota_models?.models],
  );
  const stale = isQuotaStale(
    viewedAccount?.quota?.last_updated,
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
  const updated = viewedAccount?.quota?.last_updated
    ? now - viewedAccount.quota.last_updated * 1000 < 60_000
      ? t.justNow
      : `${relativeDuration(now - viewedAccount.quota.last_updated * 1000)} ${t.ago}`
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
  const overviewPageSize = Math.min(3, pageSize);
  const poolPage = Math.min(
    quotaPage,
    Math.max(0, Math.ceil(overview.pools.length / overviewPageSize) - 1),
  );
  const visiblePools = pageSlice(overview.pools, poolPage, overviewPageSize);
  const heading = chinese ? "剩余额度" : "Remaining quota";
  const toggleAccounts = () => {
    setSwitchDetailsOpen(false);
    setAccountPickerOpen((value) => !value);
    setAccountPage(0);
  };
  const statusLabels = chinese
    ? { healthy: "充足", low: "偏低", unavailable: "待处理", unknown: "未确认" }
    : {
        healthy: "Healthy",
        low: "Low",
        unavailable: "Attention",
        unknown: "Unverified",
      };
  const activeViewed = viewedAccount?.id === current?.id;
  const readiness = viewedAccount
    ? overview.statuses[viewedAccount.id] ||
      accountReadiness(viewedAccount, threshold, config?.refresh_interval, now)
    : null;
  const viewLabel =
    accountPickerOpen || switchDetailsOpen
      ? chinese
        ? "返回"
        : "Back"
      : isOverview
        ? chinese
          ? "查看账号"
          : "View accounts"
        : activeViewed
          ? t.active
          : chinese
            ? "切换为此账号"
            : "Use this account";
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
      data-view={
        switchDetailsOpen
          ? "switch-status"
          : accountPickerOpen
            ? "accounts"
            : isOverview
              ? "overview"
              : "quota"
      }
    >
      <header className="mb-native-header">
        <div className="mb-native-identity">
          <div className="mb-native-product">
            Antigravity{" "}
            <span>{viewedAccount?.quota?.subscription_tier || "Lite"}</span>
          </div>
          <button
            type="button"
            className="mb-native-account"
            aria-label={chinese ? "选择查看范围" : "Choose view"}
            aria-expanded={accountPickerOpen}
            disabled={!accounts.length}
            onClick={toggleAccounts}
            title={viewedAccount?.email}
          >
            <strong>
              {isOverview
                ? `${chinese ? "总览" : "Overview"} · ${accounts.length} ${chinese ? "个账号" : "accounts"}`
                : viewedAccount.custom_label || viewedAccount.email}
            </strong>
            <ChevronDown size={13} />
          </button>
        </div>
        <button
          type="button"
          className="mb-native-icon"
          aria-label={
            isOverview
              ? chinese
                ? "刷新全部账号"
                : "Refresh all accounts"
              : t.refresh
          }
          title={
            isOverview
              ? chinese
                ? "刷新全部账号"
                : "Refresh all accounts"
              : t.refresh
          }
          disabled={Boolean(operation) || !accounts.length || !isTauri()}
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
        title={error || viewedAccount?.email}
      >
        {error && lowQuota.status?.phase !== "switching" ? (
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
        ) : lowQuota.visible ? (
          <button
            type="button"
            className="mb-low-quota-indicator"
            onClick={() => {
              setSwitchDetailsOpen((value) => !value);
              setAccountPickerOpen(false);
            }}
            aria-label={
              chinese
                ? "查看低额度换号详情"
                : "View low-quota switching details"
            }
            title={
              lowQuota.readError
                ? translate("auto_switch.status_failed")
                : translate(
                    `auto_switch.reasons.${lowQuota.status?.reason || "checking"}`,
                    {
                      defaultValue: translate(
                        "auto_switch.reasons.state_unavailable",
                      ),
                    },
                  )
            }
          >
            <CircleAlert size={11} />
            <span>
              {lowQuota.readError
                ? translate("auto_switch.status_failed")
                : translate(
                    `auto_switch.reasons.${lowQuota.status?.reason || "checking"}`,
                    {
                      defaultValue: translate(
                        "auto_switch.reasons.state_unavailable",
                      ),
                    },
                  )}
            </span>
            <b>{chinese ? "详情" : "Details"}</b>
          </button>
        ) : notice ? (
          <>
            <Check size={11} />
            <span>{notice}</span>
          </>
        ) : (
          <>
            <span
              className={`mb-status-dot ${isOverview ? (current ? "" : "is-stale") : stale ? "is-stale" : ""}`}
            />
            <span>
              {operation && operation !== "refresh"
                ? t.switching
                : isOverview
                  ? `${chinese ? "当前记录" : "Recorded current"}: ${current?.custom_label || current?.email || (chinese ? "未选择" : "None")}`
                  : `${activeViewed ? t.active : chinese ? "仅查看，未切换" : "Viewing only"} · ${stale ? `${t.stale} · ` : ""}${updated}`}
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
        ) : switchDetailsOpen ? (
          <MenuBarSwitchDetails
            state={lowQuota}
            openSettings={() => open("settings")}
          />
        ) : accountPickerOpen ? (
          <section
            className="mb-native-picker"
            aria-label={chinese ? "查看账号" : "View accounts"}
          >
            <div className="mb-native-section-label">
              <h2>
                {chinese ? "选择查看的账号" : "Choose an account to inspect"}
              </h2>
              {pager(
                selectedAccountPage,
                Math.ceil(accounts.length / pageSize),
                setAccountPage,
                t.accounts,
              )}
            </div>
            <button
              type="button"
              className={`mb-overview-choice ${isOverview ? "is-selected" : ""}`}
              onClick={() => viewAccount(null)}
            >
              <span>
                {isOverview && <Check size={12} />}
                {chinese ? "全部账号总览" : "All accounts overview"}
              </span>
              <span>{accounts.length}</span>
            </button>
            <div
              className="mb-native-account-list"
              style={{ "--account-page-size": pageSize } as React.CSSProperties}
            >
              {visibleAccounts.map((account) => {
                const active = account.id === current?.id;
                const selected = account.id === viewedAccount?.id;
                const health =
                  overview.statuses[account.id] ||
                  accountReadiness(
                    account,
                    threshold,
                    config?.refresh_interval,
                    now,
                  );
                const remaining = lowestKnownQuota(account);
                return (
                  <button
                    type="button"
                    key={account.id}
                    className={`mb-native-option ${selected ? "is-active" : ""}`}
                    onClick={() => viewAccount(account.id)}
                    title={account.email}
                  >
                    <span className="mb-native-option-check">
                      {selected && <Check size={13} />}
                    </span>
                    <span className="mb-native-option-text">
                      <strong>
                        {account.custom_label || account.name || account.email}
                      </strong>
                      <span>{account.email}</span>
                    </span>
                    <span className="mb-account-view-state">
                      {active ? (
                        <b>{t.active}</b>
                      ) : (
                        <span className={health === "low" ? "warning" : ""}>
                          {statusLabels[health]}
                        </span>
                      )}
                      <small>
                        {health === "unknown" ||
                        health === "unavailable" ||
                        remaining === null
                          ? "—"
                          : `${remaining}%`}
                      </small>
                    </span>
                  </button>
                );
              })}
            </div>
            <div className="mb-native-picker-bottom">
              <p>
                {chinese
                  ? "点选只查看，切换需在详情页确认。"
                  : "Selecting a row only changes this view."}
              </p>
              <button type="button" onClick={() => open("accounts")}>
                {t.addAccount}
                <ExternalLink size={12} />
              </button>
            </div>
          </section>
        ) : isOverview ? (
          <>
            <section
              className="mb-overview-counts"
              aria-label={chinese ? "账号状态" : "Account status"}
              title={
                chinese
                  ? `充足：新鲜且全部已知窗口 > ${threshold}%；过期、缺失和受保护额度不计为可用`
                  : `Healthy: fresh data and every known window above ${threshold}%. Stale, missing or protected data is unverified.`
              }
            >
              {(["healthy", "low", "unavailable", "unknown"] as const).map(
                (key) => (
                  <div key={key}>
                    <strong>{overview[key]}</strong>
                    <span>{statusLabels[key]}</span>
                  </div>
                ),
              )}
            </section>
            <section
              className="mb-native-quotas mb-overview-pools"
              aria-label={
                chinese ? "各配额池可用账号" : "Usable accounts by pool"
              }
            >
              <div className="mb-native-section-label">
                <h2>
                  {chinese ? "可用账号 / 全部账号" : "Usable / all accounts"}
                </h2>
                {pager(
                  poolPage,
                  Math.ceil(overview.pools.length / overviewPageSize),
                  setQuotaPage,
                  heading,
                )}
              </div>
              {visiblePools.length ? (
                <div
                  className="mb-native-quota-list"
                  style={
                    {
                      "--quota-rows": visiblePools.length,
                    } as React.CSSProperties
                  }
                >
                  {visiblePools.map((pool) => (
                    <div className="mb-native-quota" key={pool.key}>
                      <div className="mb-native-quota-title">
                        <div>
                          <span className="mb-native-model" title={pool.name}>
                            {pool.name.replace(/ models?$/i, "")}
                          </span>
                        </div>
                        <strong>
                          {pool.usable}
                          <span className="mb-count-denominator">
                            {" "}
                            / {pool.total}
                          </span>
                        </strong>
                      </div>
                      <div
                        className="mb-pool-distribution"
                        role="img"
                        aria-label={`${pool.usable} ${chinese ? "可用" : "usable"}, ${pool.low} ${statusLabels.low}, ${pool.unknown} ${statusLabels.unknown}, ${pool.unavailable} ${statusLabels.unavailable}`}
                      >
                        <span
                          className="is-usable"
                          style={{
                            width: `${(pool.usable / (pool.total || 1)) * 100}%`,
                          }}
                        />
                        <span
                          className="is-low"
                          style={{
                            width: `${(pool.low / (pool.total || 1)) * 100}%`,
                          }}
                        />
                        <span
                          className="is-unverified"
                          style={{
                            width: `${(pool.unknown / (pool.total || 1)) * 100}%`,
                          }}
                        />
                      </div>
                      <div className="mb-native-reset">
                        {pool.low} {statusLabels.low} · {pool.unknown}{" "}
                        {statusLabels.unknown} · {pool.unavailable}{" "}
                        {statusLabels.unavailable}
                      </div>
                    </div>
                  ))}
                </div>
              ) : (
                <div className="mb-native-empty">
                  <Users size={21} />
                  <p>{accounts.length ? t.noQuota : t.noAccountHint}</p>
                  <button onClick={() => open("accounts")}>
                    {t.addAccount}
                    <ChevronRight size={12} />
                  </button>
                </div>
              )}
              <p className="mb-overview-rule">
                {chinese
                  ? `所有已知窗口 > ${threshold}%，且数据有效；不累加额度百分比`
                  : `Fresh, known windows above ${threshold}%; percentages are not summed`}
              </p>
            </section>
            <section
              className="mb-native-usage"
              aria-label={
                chinese
                  ? "本机今日用量，未按账号拆分"
                  : "Local usage today, not attributed to accounts"
              }
            >
              <div>
                <h2>{chinese ? "本机今日用量" : "Local usage today"}</h2>
                <span>
                  {chinese
                    ? `${usage?.unreadable_databases || usage?.skipped_large_records ? "部分记录 · " : ""}未按账号拆分`
                    : `${usage?.unreadable_databases || usage?.skipped_large_records ? "Partial · " : ""}Not attributed to accounts`}
                </span>
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
        ) : (
          <>
            <section className="mb-native-quotas" aria-label={heading}>
              <div className="mb-native-section-label">
                <h2>{heading}</h2>
                {pager(selectedQuotaPage, pages.length, setQuotaPage, heading)}
              </div>
              {viewedAccount.quota?.is_forbidden ? (
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
            <section className="mb-native-usage mb-account-detail-status">
              <div>
                <h2>{chinese ? "账号状态" : "Account status"}</h2>
                <span>{readiness ? statusLabels[readiness] : t.unknown}</span>
              </div>
              <p>
                {viewedAccount.disabled
                  ? chinese
                    ? "登录已失效，请到账号管理处理"
                    : "Login unavailable; open account management"
                  : !isAccountSwitchable(viewedAccount, now)
                    ? t.verification
                    : readiness === "unknown"
                      ? chinese
                        ? "过期、缺失或受保护额度需确认"
                        : "Stale, missing or protected quota needs verification"
                      : activeViewed
                        ? chinese
                          ? "Tools 记录的当前账号"
                          : "Current account recorded by Tools"
                        : t.switchHint}
              </p>
            </section>
          </>
        )}
      </main>
      <footer className="mb-native-footer">
        <button
          type="button"
          className="mb-native-switch"
          disabled={
            (!accounts.length && !accountPickerOpen && !switchDetailsOpen) ||
            !isTauri() ||
            (!accountPickerOpen &&
              !switchDetailsOpen &&
              !isOverview &&
              (Boolean(operation) ||
                lowQuota.status === null ||
                lowQuota.status?.phase === "switching" ||
                lowQuota.readError ||
                activeViewed ||
                !isAccountSwitchable(viewedAccount, now)))
          }
          title={!isOverview ? t.switchHint : undefined}
          onClick={() =>
            switchDetailsOpen
              ? setSwitchDetailsOpen(false)
              : accountPickerOpen
                ? setAccountPickerOpen(false)
                : isOverview
                  ? toggleAccounts()
                  : void switchAccount(viewedAccount)
          }
        >
          {operation && operation !== "refresh" ? (
            <Loader2 size={13} className="mb-spin" />
          ) : accountPickerOpen || switchDetailsOpen ? (
            <ChevronLeft size={13} />
          ) : activeViewed ? (
            <Check size={13} />
          ) : (
            <Users size={13} />
          )}
          {operation && operation !== "refresh" ? t.switching : viewLabel}
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
