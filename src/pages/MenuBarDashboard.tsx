import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  Activity,
  AlertTriangle,
  Check,
  ChevronLeft,
  ChevronRight,
  ExternalLink,
  Loader2,
  LogOut,
  RefreshCw,
  Settings2,
  Users,
  Zap,
} from "lucide-react";
import { listen } from "@tauri-apps/api/event";
import { useTranslation } from "react-i18next";
import type { Account } from "../types/account";
import { request } from "../utils/request";
import { isTauri } from "../utils/env";
import {
  compactQuotaGroups,
  isAccountSwitchable,
  lowestKnownQuota,
  resetTimestamp,
} from "../utils/menuBarQuota";
import { useMenuBarSwitchStatus } from "../components/menubar/LowQuotaStatus";
import "../components/menubar/MenuBarDashboard.css";

interface LocalUsage {
  today: { total_tokens: number; request_count: number };
}

const formatTokens = (value: number) => {
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`;
  if (value >= 1_000) return `${(value / 1_000).toFixed(1)}K`;
  return `${value}`;
};

export default function MenuBarDashboard() {
  const { i18n, t: translate } = useTranslation();
  const chinese = i18n.language.startsWith("zh");
  const lowQuota = useMenuBarSwitchStatus();

  const [accounts, setAccounts] = useState<Account[]>([]);
  const [currentAccount, setCurrentAccount] = useState<Account | null>(null);
  const [selectedAccountId, setSelectedAccountId] = useState<string | null>(null);
  const [usage, setUsage] = useState<LocalUsage | null>(null);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [switchingId, setSwitchingId] = useState<string | null>(null);
  const [accountPage, setAccountPage] = useState(0);
  const [now, setNow] = useState(Date.now());

  const generation = useRef(0);
  const operationLock = useRef(false);

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
        : saved[0] || null;
      setCurrentAccount(activeAccount);
      setSelectedAccountId((prev) => prev ?? (activeAccount ? activeAccount.id : null));
      setNow(Date.now());
    } catch (e) {
      console.error("Failed to load accounts in menubar:", e);
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
        setNow(Date.now());
        void reload();
        void loadUsage();
      }),
      listen("menubar://data-updated", () => void reload()),
      listen("tray://account-switched", () => void reload()),
      listen("accounts://refreshed", () => void reload()),
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
    }, 30_000);
    return () => window.clearInterval(timer);
  }, []);

  const hide = useCallback(() => {
    if (isTauri()) void request("hide_menu_bar_dashboard");
  }, []);

  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        hide();
      }
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, [hide]);

  const openPage = (page: "dashboard" | "accounts" | "settings") => {
    if (isTauri()) void request("open_app_page", { page });
  };

  const viewAccount = (accountId: string | null) => {
    setSelectedAccountId(accountId);
  };

  const refresh = async () => {
    if (operationLock.current || !accounts.length) return;
    operationLock.current = true;
    setRefreshing(true);
    try {
      const target = accounts.find((a) => a.id === selectedAccountId) || currentAccount;
      if (target) {
        await request("fetch_account_quota", { accountId: target.id });
      } else {
        await request("refresh_all_quotas");
      }
      await reload();
      void loadUsage();
    } catch (e) {
      console.error("Refresh quota failed:", e);
    } finally {
      operationLock.current = false;
      setRefreshing(false);
    }
  };

  const switchAccount = async (account: Account) => {
    if (
      operationLock.current ||
      account.id === currentAccount?.id ||
      !isAccountSwitchable(account, now)
    )
      return;
    operationLock.current = true;
    setSwitchingId(account.id);
    const targetName = account.custom_label || account.email;
    try {
      await request("switch_account", { accountId: account.id });
      await reload();
    } catch (e) {
      console.error("Switch account failed for " + targetName, e);
      await reload();
    } finally {
      operationLock.current = false;
      setSwitchingId(null);
    }
  };

  const viewedAccount = accounts.find((a) => a.id === selectedAccountId) || currentAccount;
  const groups = useMemo(
    () => compactQuotaGroups(viewedAccount?.quota),
    [viewedAccount?.quota],
  );

  const activeQuotaModels = useMemo(() => {
    if (!viewedAccount?.quota || viewedAccount.quota.is_forbidden) return [];
    const rows: Array<{
      id: string;
      name: string;
      window?: string;
      percentage: number | null;
      resetLabel: string;
    }> = [];

    for (const group of groups) {
      if (group.name.includes("-tiered")) continue;
      for (const row of group.rows) {
        if (row.label && row.label.includes("-tiered")) continue;

        let resetLabel = "—";
        if (row.resetTime) {
          const ts = resetTimestamp(row.resetTime);
          if (ts !== null) {
            const diff = ts - now;
            if (diff <= 0) {
              resetLabel = chinese ? "已到重置时间 · 请刷新" : "Reset due · refresh";
            } else {
              const mins = Math.ceil(diff / 60_000);
              if (mins < 60) {
                resetLabel = chinese ? `${mins}分钟后重置` : `Resets in ${mins}m`;
              } else {
                const hrs = Math.floor(mins / 60);
                const remMins = mins % 60;
                if (hrs < 24) {
                  resetLabel = chinese
                    ? `${hrs}小时${remMins ? `${remMins}分` : ""}后重置`
                    : `Resets in ${hrs}h ${remMins ? `${remMins}m` : ""}`;
                } else {
                  const days = Math.floor(hrs / 24);
                  resetLabel = chinese ? `${days}天后重置` : `Resets in ${days}d`;
                }
              }
            }
          }
        }

        let cleanName = group.name;
        if (cleanName.includes("claude-sonnet-4-6") || cleanName.includes("claude-3-7")) cleanName = "Claude 3.7 Sonnet";
        else if (cleanName.includes("claude-3-5")) cleanName = "Claude 3.5 Sonnet";
        else if (cleanName.includes("flash")) cleanName = "Gemini Flash";
        else if (cleanName.includes("pro")) cleanName = "Gemini Pro";
        else cleanName = cleanName.replace(/ models?$/i, "");

        let windowLabel = row.window;
        if (windowLabel === "5h") windowLabel = chinese ? "5小时" : "5h";
        else if (windowLabel === "weekly") windowLabel = chinese ? "每周" : "Weekly";

        rows.push({
          id: row.id,
          name:
            row.label && row.label !== row.window && !row.label.includes("5h")
              ? row.label
              : cleanName,
          window: windowLabel,
          percentage: row.remaining !== null ? Math.round(row.remaining) : null,
          resetLabel,
        });
      }
    }

    return rows.slice(0, 4);
  }, [viewedAccount, groups, now, chinese]);

  const ACCOUNTS_PER_PAGE = 4;
  const totalAccountPages = Math.max(1, Math.ceil(accounts.length / ACCOUNTS_PER_PAGE));
  const currentPage = Math.min(accountPage, totalAccountPages - 1);
  const pagedAccounts = useMemo(() => {
    const start = currentPage * ACCOUNTS_PER_PAGE;
    return accounts.slice(start, start + ACCOUNTS_PER_PAGE);
  }, [accounts, currentPage]);

  const lastUpdated = useMemo(() => {
    if (!viewedAccount?.quota?.last_updated) {
      return chinese ? "尚未刷新" : "Not refreshed";
    }
    const diff = now - viewedAccount.quota.last_updated * 1000;
    if (diff < 60_000) return chinese ? "刚刚更新" : "Just now";
    const mins = Math.floor(diff / 60_000);
    if (mins < 60) return chinese ? `${mins} 分钟前` : `${mins}m ago`;
    const hrs = Math.floor(mins / 60);
    return chinese ? `${hrs} 小时前` : `${hrs}h ago`;
  }, [viewedAccount, now, chinese]);

  const getQuotaColor = (val: number | null) => {
    if (val === null) return "text-slate-400 bg-slate-400";
    if (val >= 30) return "text-emerald-500 bg-emerald-500";
    if (val >= 10) return "text-amber-500 bg-amber-500";
    return "text-rose-500 bg-rose-500";
  };

  const isViewedActive = viewedAccount?.id === currentAccount?.id;

  return (
    <div className="menubar-app">
      {/* Top Header (CodexBar Style) */}
      <header className="mb-header">
        <div className="mb-header-top">
          <div className="mb-brand-group">
            <span className="mb-brand-title">Antigravity</span>
            <span className="mb-tier-badge">
              {currentAccount?.quota?.subscription_tier || "PRO"}
            </span>
          </div>

          <div className="mb-header-actions">
            <button
              type="button"
              className="mb-action-btn"
              title={chinese ? "刷新配额" : "Refresh quotas"}
              onClick={() => void refresh()}
              disabled={refreshing || !accounts.length}
            >
              <RefreshCw
                size={13}
                className={refreshing ? "animate-spin text-blue-500" : ""}
              />
            </button>
            <button
              type="button"
              className="mb-action-btn"
              title={chinese ? "打开仪表盘" : "Open Full Dashboard"}
              onClick={() => openPage("dashboard")}
            >
              <ExternalLink size={13} />
            </button>
            <button
              type="button"
              className="mb-action-btn"
              title={chinese ? "设置" : "Settings"}
              onClick={() => openPage("settings")}
            >
              <Settings2 size={13} />
            </button>
            <button
              type="button"
              className="mb-action-btn hover-danger"
              title={chinese ? "退出应用" : "Quit"}
              onClick={() => request("quit_app")}
            >
              <LogOut size={13} />
            </button>
          </div>
        </div>

        <div className="mb-header-sub">
          <div
            className="mb-current-identity"
            title={currentAccount?.email || (chinese ? "暂无账号" : "No Account")}
          >
            <span className="mb-status-dot" />
            <span className="mb-current-email-text">
              {currentAccount?.custom_label ||
                currentAccount?.email ||
                (chinese ? "暂无生效账号" : "No active account")}
            </span>
          </div>
          <span className="mb-sync-time">{chinese ? "已同步" : "Synced"}</span>
        </div>
      </header>

      <div className="mb-divider" />

      {/* Main Body */}
      <main className="mb-body">
        {loading ? (
          <div className="mb-empty-state">
            <Loader2 size={18} className="animate-spin text-blue-500" />
            <span>{chinese ? "正在加载账号数据…" : "Loading accounts…"}</span>
          </div>
        ) : (
          <>
            {/* Auto Switch Alert (if monitoring or pending) */}
            {lowQuota.visible && (
              <div className="mb-switch-alert">
                <AlertTriangle size={12} className="shrink-0" />
                <span className="truncate">
                  {translate(
                    `auto_switch.reasons.${lowQuota.status?.reason || "monitoring"}`,
                    {
                      defaultValue: translate(
                        "auto_switch.reasons.state_unavailable",
                      ),
                    },
                  )}
                </span>
                <button
                  type="button"
                  onClick={() => openPage("settings")}
                  className="mb-alert-link"
                >
                  {chinese ? "配置" : "Setup"}
                </button>
              </div>
            )}

            {/* Section 1: Active / Viewed Account Quotas */}
            <section className="mb-section">
              <div className="mb-section-header">
                <div className="mb-section-title">
                  <Zap size={11} style={{ color: "#f59e0b" }} />
                  <span>
                    {isViewedActive
                      ? chinese
                        ? "模型配额"
                        : "Model Quotas"
                      : chinese
                        ? `配额预览 · ${viewedAccount?.custom_label || viewedAccount?.email}`
                        : `Quota · ${viewedAccount?.custom_label || viewedAccount?.email}`}
                  </span>
                </div>
                <div className="mb-section-extra">{lastUpdated}</div>
              </div>

              <div className="mb-quota-list">
                {activeQuotaModels.length > 0 ? (
                  activeQuotaModels.map((model) => (
                    <div key={model.id} className="mb-quota-row">
                      <div className="mb-quota-row-header">
                        <div className="mb-quota-name-group">
                          <span className="mb-quota-model-name">{model.name}</span>
                          {model.window && (
                            <span className="mb-window-tag">{model.window}</span>
                          )}
                        </div>
                        <div className="mb-quota-row-meta">
                          <span
                            className={`mb-quota-percent ${getQuotaColor(model.percentage).split(" ")[0]}`}
                          >
                            {model.percentage !== null
                              ? `${model.percentage}%`
                              : "—"}
                          </span>
                        </div>
                      </div>
                      <div className="mb-quota-track">
                        <div
                          className={`mb-quota-bar ${getQuotaColor(model.percentage).split(" ")[1]}`}
                          style={{
                            width: `${Math.max(0, Math.min(100, model.percentage ?? 0))}%`,
                          }}
                        />
                      </div>
                      <div className="mb-quota-reset">{model.resetLabel}</div>
                    </div>
                  ))
                ) : (
                  <div
                    style={{
                      padding: "12px 0",
                      textAlign: "center",
                      color: "rgba(0,0,0,0.4)",
                      fontSize: "11px",
                    }}
                  >
                    {viewedAccount?.quota?.is_forbidden
                      ? chinese
                        ? "额度访问受限 (403)"
                        : "Quota access denied (403)"
                      : chinese
                        ? "暂无配额数据，点击右上角刷新获取"
                        : "No quota reported, refresh to sync"}
                  </div>
                )}
              </div>

              {/* Action bar for viewed account */}
              {!isViewedActive && viewedAccount && (
                <div className="mb-preview-banner">
                  <span className="mb-preview-text">
                    {chinese ? "正在预览该账号额度" : "Previewing account quota"}
                  </span>
                  <button
                    type="button"
                    className="mb-use-account-btn"
                    disabled={!isAccountSwitchable(viewedAccount, now) || Boolean(switchingId)}
                    onClick={() => void switchAccount(viewedAccount)}
                  >
                    {switchingId === viewedAccount.id
                      ? chinese
                        ? "切换中…"
                        : "Switching…"
                      : chinese
                        ? "切换为此账号"
                        : "Use this account"}
                  </button>
                </div>
              )}

              {isViewedActive && (
                <div className="mb-synced-state">
                  <Check size={11} />
                  <span>
                    {chinese ? "当前生效账号 · 本机已同步" : "Current account recorded by Tools"}
                  </span>
                </div>
              )}
            </section>

            <div className="mb-divider" />

            {/* Section 2: Token Usage / Activity */}
            <section className="mb-section" style={{ padding: "8px 14px" }}>
              <div className="mb-section-header">
                <div className="mb-section-title">
                  <Activity size={11} style={{ color: "#007aff" }} />
                  <span>{chinese ? "今日用量" : "Today's Activity"}</span>
                </div>
              </div>
              <div className="mb-activity-row">
                <span className="mb-activity-tokens">
                  {usage
                    ? `${formatTokens(usage.today.total_tokens)} Tokens`
                    : "0 Tokens"}
                </span>
                <span className="mb-activity-reqs">
                  {usage
                    ? `${usage.today.request_count} ${chinese ? "次请求" : "requests"}`
                    : "0 requests"}
                </span>
              </div>
            </section>

            <div className="mb-divider" />

            {/* Section 3: Compact Account Switcher (CodexBar Style) */}
            <section className="mb-accounts-section">
              <div className="mb-section-header">
                <div className="mb-section-title">
                  <Users size={11} style={{ color: "#007aff" }} />
                  <span>{chinese ? "账号切换" : "Quick Switch"}</span>
                  <span className="mb-account-count-badge">{accounts.length}</span>
                </div>

                {totalAccountPages > 1 ? (
                  <div className="mb-pager">
                    <button
                      type="button"
                      className="mb-pager-btn"
                      disabled={currentPage === 0}
                      onClick={() => setAccountPage((p) => Math.max(0, p - 1))}
                    >
                      <ChevronLeft size={12} />
                    </button>
                    <span>
                      {currentPage + 1}/{totalAccountPages}
                    </span>
                    <button
                      type="button"
                      className="mb-pager-btn"
                      disabled={currentPage >= totalAccountPages - 1}
                      onClick={() =>
                        setAccountPage((p) =>
                          Math.min(totalAccountPages - 1, p + 1),
                        )
                      }
                    >
                      <ChevronRight size={12} />
                    </button>
                  </div>
                ) : (
                  <button
                    type="button"
                    className="mb-card-link"
                    onClick={() => openPage("accounts")}
                  >
                    {chinese ? "管理" : "Manage"} →
                  </button>
                )}
              </div>

              <div className="mb-account-list">
                {pagedAccounts.map((account) => {
                  const isCurrent = account.id === currentAccount?.id;
                  const isSelected = account.id === viewedAccount?.id;
                  const isSwitching = switchingId === account.id;
                  const lowestQuota = lowestKnownQuota(account);
                  const isSwitchable = isAccountSwitchable(account, now);

                  return (
                    <div
                      key={account.id}
                      className={`mb-account-row ${isSelected ? "is-selected" : ""}`}
                      onClick={() => viewAccount(account.id)}
                    >
                      <div className="mb-account-info-left">
                        <div className="mb-account-top-line">
                          <span
                            className={`mb-acc-dot ${isCurrent ? "active" : ""}`}
                          />
                          <span
                            className="mb-acc-name"
                            title={account.email}
                          >
                            {account.custom_label || account.email}
                          </span>
                          <span
                            className="mb-tier-badge"
                            style={{ fontSize: "8px", padding: "0 4px" }}
                          >
                            {account.quota?.subscription_tier || "PRO"}
                          </span>
                        </div>
                        {account.custom_label && (
                          <span
                            className="mb-acc-sub-email"
                            title={account.email}
                          >
                            {account.email}
                          </span>
                        )}
                      </div>

                      <div className="mb-account-action-right">
                        {lowestQuota !== null && !isCurrent && (
                          <div className="mb-headroom-group">
                            <div className="mb-headroom-track">
                              <div
                                className={`mb-headroom-bar ${getQuotaColor(lowestQuota).split(" ")[1]}`}
                                style={{
                                  width: `${Math.max(0, Math.min(100, lowestQuota))}%`,
                                }}
                              />
                            </div>
                            <span
                              className={`mb-headroom-percent ${getQuotaColor(lowestQuota).split(" ")[0]}`}
                            >
                              {Math.round(lowestQuota)}%
                            </span>
                          </div>
                        )}

                        {isCurrent ? (
                          <span className="mb-active-pill">
                            <Check size={11} />
                            {chinese ? "生效中" : "Active"}
                          </span>
                        ) : isSwitching ? (
                          <span className="mb-switching-pill">
                            <Loader2 size={11} className="animate-spin" />
                            {chinese ? "切换中" : "Switching"}
                          </span>
                        ) : (
                          <button
                            type="button"
                            className="mb-switch-btn"
                            disabled={!isSwitchable || Boolean(switchingId)}
                            onClick={(e) => {
                              e.stopPropagation();
                              void switchAccount(account);
                            }}
                          >
                            {chinese ? "切换" : "Switch"}
                          </button>
                        )}
                      </div>
                    </div>
                  );
                })}
              </div>
            </section>
          </>
        )}
      </main>

      <div className="mb-divider" />

      {/* Footer */}
      <footer className="mb-footer">
        <span>Antigravity Tools Lite</span>
        <span>Esc {chinese ? "关闭" : "Close"}</span>
      </footer>
    </div>
  );
}
