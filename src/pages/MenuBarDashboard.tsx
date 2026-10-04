import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { ArrowLeft, ArrowLeftRight, Check, ChevronLeft, ChevronRight, ExternalLink, Loader2, LogOut, RefreshCw, Settings, Users, Sparkles, Brain } from 'lucide-react';
import { listen } from '@tauri-apps/api/event';
import { useTranslation } from 'react-i18next';
import { request } from '../utils/request';
import { isTauri } from '../utils/env';
import { useConfigStore } from '../stores/useConfigStore';
import { aggregateMenuBar, menuBarAccount, quotaDisplay, reportedQuotaDetails, type MenuBarSnapshot, type QuotaReason, type QuotaWindow } from '../utils/menuBarOverview';
import { MenuBarSwitchDetails, useMenuBarSwitchStatus } from '../components/menubar/LowQuotaStatus';
import '../components/menubar/MenuBarDashboard.css';
import { DEFAULT_MENU_BAR_PREFERENCES, type MenuBarPreferences, type MenuBarQuotaScope } from '../types/config';
import { quotaTone } from '../utils/menuBarOverview';

interface Appearance { native_material: boolean; reduced_transparency: boolean; high_contrast: boolean; material_kind?: string }
interface Usage { today: { total_tokens: number; request_count: number } }
const tokens = (value: number) => value >= 1e6 ? (value / 1e6).toFixed(1) + 'M' : value >= 1e3 ? (value / 1e3).toFixed(1) + 'K' : String(value);
function Meter({ value, label, preferences }: { value: number | null; label: string; preferences: MenuBarPreferences }) {
  if (value === null) return <div className="mb-meter unknown" role="img" aria-label={label + ': —'} />;
  return <div className={'mb-meter ' + quotaTone(value, preferences)} role="meter" aria-label={label} aria-valuemin={0} aria-valuemax={100} aria-valuenow={value ?? undefined} aria-valuetext={quotaDisplay(value)}>
    <span style={{ width: String(value ?? 0) + '%' }} />
  </div>;
}
export default function MenuBarDashboard() {
  const { i18n, t } = useTranslation();
  const zh = i18n.language.startsWith('zh');
  const config = useConfigStore(state => state.config);
  const lowQuota = useMenuBarSwitchStatus();
  const [snapshot, setSnapshot] = useState<MenuBarSnapshot | null>(null);
  const [usage, setUsage] = useState<Usage | null>(null);
  const [appearance, setAppearance] = useState<Appearance | null>(null);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [switching, setSwitching] = useState<string | null>(null);
  const [error, setError] = useState('');
  const [notice, setNotice] = useState('');
  const [now, setNow] = useState(Date.now());
  const [page, setPage] = useState(0);
  const [capacity, setCapacity] = useState(3);
  const [detail, setDetail] = useState<string | null>(null);
  const [detailPage, setDetailPage] = useState(0);
  const [detailCapacity, setDetailCapacity] = useState(5);
  const content = useRef<HTMLDivElement>(null);
  const generation = useRef(0);
  const operation = useRef(false);
  const mounted = useRef(true);
  const preferences = useMemo(() => ({ ...DEFAULT_MENU_BAR_PREFERENCES, ...config?.menu_bar }), [config?.menu_bar]);
  const scope = preferences.quota_scope;
  const periods: QuotaWindow[] = [...(preferences.show_session ? ['5h' as const] : []), ...(preferences.show_weekly ? ['weekly' as const] : [])];
  const families = preferences.display_scope === 'all' ? ['gemini', 'other'] as const : [preferences.display_scope];
  const setFamily = async (display_scope: MenuBarQuotaScope) => {
    try { const next = await request<MenuBarPreferences>('set_menu_bar_preferences', { patch: { display_scope } }); useConfigStore.setState(state => ({ config: state.config ? { ...state.config, menu_bar: next } : null })); }
    catch { setError(zh ? '保存显示系列失败，请重试' : 'Could not save the display family. Retry.'); }
  };
  const scopeName = scope === 'gemini' ? 'Gemini' : scope === 'other' ? 'Claude / GPT' : zh ? '全部系列' : 'All families';
  const windowName = (window: QuotaWindow) => window === '5h' ? zh ? '5 小时' : '5 hours' : zh ? '每周' : 'Weekly';
  const reasonName = (reason: QuotaReason | null) => reason ? ({
    disabled: zh ? '已禁用' : 'Disabled', blocked: zh ? '待验证' : 'Verification required', forbidden: zh ? '访问受限' : 'Access denied', unreadable: zh ? '读取失败' : 'Unreadable', stale: zh ? '待刷新' : 'Refresh needed', protected: zh ? '额度保护中' : 'Protected', unknown: zh ? '未报告' : 'Not reported', expired: zh ? '已到重置时间' : 'Reset due', conflict: zh ? '数据冲突' : 'Conflicting data',
  })[reason] : '';
  const resetLabel = (reset: string) => {
    const diff = Date.parse(reset) - now;
    if (!Number.isFinite(diff)) return '—';
    if (diff <= 0) return zh ? '请刷新' : 'Refresh needed';
    const minutes = Math.ceil(diff / 60000);
    return minutes < 60 ? minutes + (zh ? '分后重置' : 'm to reset') : minutes < 1440 ? Math.floor(minutes / 60) + 'h ' + minutes % 60 + 'm' : Math.floor(minutes / 1440) + 'd ' + Math.floor(minutes % 1440 / 60) + 'h';
  };
  const reload = useCallback(async () => {
    if (!isTauri()) { setLoading(false); return; }
    const id = ++generation.current;
    try {
      const next = await request<MenuBarSnapshot>('get_menu_bar_snapshot');
      if (mounted.current && id === generation.current) { setSnapshot(next); setNow(Date.now()); setError(''); }
    } catch {
      if (mounted.current && id === generation.current) {
        setSnapshot(null);
        setError(i18n.language.startsWith('zh') ? '账号读取失败，请重试' : 'Could not read accounts. Retry.');
      }
    } finally { if (mounted.current && id === generation.current) setLoading(false); }
  }, [i18n]);
  const readAppearance = useCallback(async () => {
    try { const next = await request<Appearance>('get_menu_bar_appearance'); if (mounted.current) setAppearance(next); }
    catch { if (mounted.current) setAppearance(null); }
  }, []);
  const readUsage = useCallback(async () => {
    try { const next = await request<Usage>('get_local_token_usage'); if (mounted.current) setUsage(next); }
    catch { if (mounted.current) setUsage(null); }
  }, []);
  useEffect(() => {
    mounted.current = true;
    document.documentElement.classList.add('panel-window'); document.body.classList.add('panel-window');
    void reload(); void readUsage();
    if (!isTauri()) return;
    void readAppearance();
    const subscriptions = [
      listen('menubar://opened', () => { void reload(); void readUsage(); void readAppearance(); }),
      ...['menubar://data-updated', 'tray://account-switched', 'accounts://refreshed'].map(event => listen(event, () => void reload())),
      listen<Appearance>('menubar://appearance', event => setAppearance(event.payload)),
    ];
    return () => { mounted.current = false; generation.current++; void Promise.all(subscriptions).then(stops => stops.forEach(stop => stop())); };
  }, [reload, readUsage, readAppearance]);
  useEffect(() => {
    const timer = window.setInterval(() => { if (document.visibilityState !== 'hidden') setNow(Date.now()); }, 30000);
    const key = (event: KeyboardEvent) => {
      if (event.key === 'Escape') { event.preventDefault(); if (detail) setDetail(null); else if (isTauri()) void request('hide_menu_bar_dashboard'); }
    };
    window.addEventListener('keydown', key);
    return () => { clearInterval(timer); window.removeEventListener('keydown', key); };
  }, [detail]);
  useLayoutEffect(() => {
    if (!content.current) return;
    const element = content.current;
    const measure = () => { setCapacity(Math.max(1, Math.floor((element.clientHeight - (Math.floor((element.clientHeight - 56) / 80) >= (snapshot?.accounts.length ?? 0) ? 56 : 84)) / 80))); setDetailCapacity(Math.max(1, Math.floor((element.clientHeight - 98) / 50))); };
    const observer = new ResizeObserver(measure); observer.observe(element); measure();
    return () => observer.disconnect();
  }, [detail, loading, snapshot?.accounts.length]);
  const accounts = useMemo(() => (snapshot?.accounts || []).map(account => menuBarAccount(account, now, config?.refresh_interval)).filter(view => !preferences.hide_unavailable || view.switchable), [snapshot, now, config?.refresh_interval, preferences.hide_unavailable]);
  const threshold = lowQuota.config?.reserve_percentage ?? config?.quota_protection.threshold_percentage ?? 10;
  const aggregate = (window: QuotaWindow) => aggregateMenuBar(accounts, scope, window, threshold);
  const pageCount = Math.max(1, Math.ceil(accounts.length / capacity));
  const visiblePage = Math.min(page, pageCount - 1);
  const visible = accounts.slice(visiblePage * capacity, (visiblePage + 1) * capacity);
  const selected = accounts.find(view => view.account.id === detail);
  const details = selected ? reportedQuotaDetails(selected.account).flatMap(pool => pool.windows.map(row => ({ ...row, name: pool.name, source: pool.source }))) : [];
  const detailPages = Math.max(1, Math.ceil(details.length / detailCapacity));
  const visibleDetailPage = Math.min(detailPage, detailPages - 1);
  const busy = refreshing || Boolean(switching) || lowQuota.busy || lowQuota.status?.phase === 'switching';
  const openPage = (page: string) => { if (isTauri()) void request('open_app_page', { page }); };
  const refresh = async () => {
    if (operation.current || busy) return;
    operation.current = true; setRefreshing(true); setError(''); setNotice('');
    try {
      const result = await request<{ failed: number }>('refresh_all_quotas');
      await reload(); void readUsage();
      if (mounted.current && result.failed) setError(zh ? String(result.failed) + ' 个账号刷新失败，已保留缓存' : String(result.failed) + ' accounts failed to refresh. Cached data retained.');
    } catch { if (mounted.current) setError(zh ? '刷新失败，请重试' : 'Refresh failed. Retry.'); }
    finally { operation.current = false; if (mounted.current) setRefreshing(false); }
  };
  const switchAccount = async (id: string) => {
    const target = accounts.find(view => view.account.id === id);
    if (operation.current || busy || !target?.switchable || id === snapshot?.current_account_id) return;
    operation.current = true; setSwitching(id); setError(''); setNotice('');
    try {
      await request('switch_account', { accountId: id }); await reload();
      if (mounted.current) setNotice((zh ? '已切换到 ' : 'Switched to ') + (target.account.custom_label || target.account.email));
    } catch { await reload(); if (mounted.current) setError(zh ? '切换失败，请在 App 中检查账号状态' : 'Switch failed. Check this account in the app.'); }
    finally { operation.current = false; if (mounted.current) setSwitching(null); }
  };
  const pager = (index: number, total: number, update: (page: number) => void) => <nav className="mb-pagination" aria-label={zh ? '分页' : 'Pagination'}>
    <button aria-label={zh ? '上一页' : 'Previous page'} disabled={index === 0} onClick={() => update(index - 1)}><ChevronLeft size={13} /></button><span>{index + 1} / {total}</span><button aria-label={zh ? '下一页' : 'Next page'} disabled={index + 1 >= total} onClick={() => update(index + 1)}><ChevronRight size={13} /></button>
  </nav>;
  return <div className={'menubar-app ' + (appearance?.native_material ? 'native-material' : 'opaque-material') + (appearance?.high_contrast ? ' high-contrast' : '')} data-material={appearance?.material_kind || (appearance?.native_material ? 'vibrancy' : 'opaque')}>
    <header className="mb-header">
      <div><span className="mb-eyebrow">AntiGravity tool lite</span>{detail && <h1>{detail === 'switch' ? zh ? '自动切号' : 'Auto-switch' : zh ? '账号详情' : 'Account details'}</h1>}</div>
      <div className="mb-header-actions">{detail && <button aria-label={zh ? '返回总览' : 'Back to overview'} onClick={() => setDetail(null)}><ArrowLeft size={16} /></button>}<button aria-label={zh ? '刷新全部额度' : 'Refresh all quotas'} disabled={busy || loading} onClick={() => void refresh()}><RefreshCw size={16} className={refreshing ? 'animate-spin' : ''} /></button><button aria-label={zh ? '偏好设置' : 'Settings'} onClick={() => openPage('settings')}><Settings size={16} /></button></div>
    </header>
    {!detail && preferences.show_aggregate && <section className="mb-overview" aria-label={zh ? '聚合额度' : 'Aggregate quotas'}>
      <div className="mb-overview-heading"><h2>{zh ? '总览' : 'Overview'}</h2><span>{scopeName} · {zh ? '平均剩余' : 'Mean remaining'}</span></div>
      {periods.map(window => { const data = aggregate(window); return <div className="mb-aggregate" key={window}>
        <div><span>{windowName(window)}</span><span className="mb-availability" title={(zh ? '有效数据 ' : 'Reported ') + data.covered + '/' + data.total}>{zh ? '可用 ' : 'Available '}{data.usable}/{data.total}</span><span>{zh ? '剩余' : 'Remaining'} <strong>{quotaDisplay(data.remaining)}</strong></span></div><Meter preferences={preferences} value={data.remaining} label={scopeName + ' ' + windowName(window)} />
      </div>; })}
    </section>}
    {(error || notice) && <div className={'mb-message ' + (error ? 'error' : '')} role={error ? 'alert' : 'status'}>{error || notice}{error && !refreshing && <button onClick={() => void reload()}>{zh ? '重试读取' : 'Retry read'}</button>}</div>}
    {!detail && lowQuota.visible && <button className="mb-switch-banner" aria-label={zh ? '查看低额度换号详情' : 'View auto-switch details'} onClick={() => setDetail('switch')}><ArrowLeftRight size={13} /><span>{lowQuota.readError ? t('auto_switch.status_failed') : t('auto_switch.reasons.' + (lowQuota.status?.reason || 'checking'), { defaultValue: t('auto_switch.reasons.state_unavailable') })}</span><ChevronRight size={12} /></button>}
    <div className="mb-content" ref={content}>
      {detail === 'switch' ? <MenuBarSwitchDetails state={lowQuota} openSettings={() => openPage('settings')} /> : selected ? <>
        <div className="mb-detail-title"><strong>{selected.account.custom_label || selected.account.email}</strong><span>{selected.account.email}</span><small>{zh ? '缓存快照 · ' : 'Cached snapshot · '}{selected.account.quota?.last_updated ? new Date(selected.account.quota.last_updated * 1000).toLocaleTimeString(i18n.language, { hour: '2-digit', minute: '2-digit' }) : '—'}</small></div>
        <div className="mb-detail-rows">{details.length ? details.slice(visibleDetailPage * detailCapacity, (visibleDetailPage + 1) * detailCapacity).map((row, index) => <div className="mb-detail-row" key={row.key + index}><div><span title={row.name}>{row.name}</span><small>{row.source === 'model' ? zh ? '模型快照' : 'Model snapshot' : row.window === 'weekly' ? windowName('weekly') : row.window === '5h' ? windowName('5h') : row.window}</small><strong>{quotaDisplay(row.remaining)}</strong></div><div><Meter preferences={preferences} value={row.remaining} label={row.name + ' ' + row.window} /><small>{resetLabel(row.resetTime)}</small></div></div>) : <div className="mb-empty">{reasonName(selected.windows.weekly.gemini.reason)}</div>}</div>
        {detailPages > 1 && pager(visibleDetailPage, detailPages, setDetailPage)}
      </> : detail ? <div className="mb-empty">{zh ? '账号已移除' : 'Account removed'}</div> : <>
        <div className="mb-account-heading"><span>{zh ? accounts.length + ' 个账号' : 'Accounts (' + accounts.length + ')'}</span><nav aria-label={zh ? '账号显示系列' : 'Account family'}>{(['gemini', 'other', 'all'] as const).map(family => <button key={family} aria-pressed={preferences.display_scope === family} onClick={() => void setFamily(family)}>{preferences.show_icons && (family === 'other' ? <Brain size={12} /> : <Sparkles size={12} />)}{family === 'gemini' ? 'Gemini' : family === 'other' ? zh ? '非 Gemini' : 'Non-Gemini' : zh ? '全部' : 'All'}</button>)}</nav></div>
        <div className="mb-accounts">{loading ? <div className="mb-empty"><Loader2 size={18} className="animate-spin" />{zh ? '正在读取' : 'Loading'}</div> : !accounts.length ? <div className="mb-empty">{error ? zh ? '暂无可读取的数据' : 'Data unavailable' : zh ? '添加账号后显示额度' : 'Add accounts to see quotas'}<button onClick={() => openPage('accounts')}>{zh ? '管理账号' : 'Manage accounts'}</button></div> : visible.map(view => {
          const account = view.account; const current = account.id === snapshot?.current_account_id;
          const label = account.custom_label || account.name || account.email.split('@')[0];
          return <article className={'mb-account-row ' + (current ? 'current' : '')} key={account.id}>
            <div className="mb-account-identity"><button title={account.email} aria-label={(zh ? '查看 ' : 'Inspect ') + label} onClick={() => { setDetail(account.id); setDetailPage(0); }}><span>{preferences.label_style === 'label_then_email' && account.custom_label ? account.custom_label : account.email}</span><small>{preferences.label_style === 'email_only' ? '' : preferences.label_style === 'label_then_email' && account.custom_label ? account.email : account.custom_label ? account.custom_label : ''}</small></button><button className="mb-account-switch" aria-label={(zh ? '切换到 ' : 'Switch to ') + label} title={current ? snapshot?.current_identity_source === 'running_app' ? zh ? '运行中的 App 已确认' : 'Verified running app' : zh ? 'Tools 保存的账号' : 'Saved Tools account' : zh ? '切换并重新打开 App' : 'Switch and reopen app'} disabled={current || busy || !view.switchable || lowQuota.readError} onClick={() => void switchAccount(account.id)}>{switching === account.id ? <Loader2 size={12} className="animate-spin" /> : current ? <Check size={12} /> : <ArrowLeftRight size={12} />}{current ? zh ? '当前' : 'Current' : zh ? '切换' : 'Switch'}</button></div>
            <div className="mb-account-quotas">{periods.map(window => <div className="mb-account-window" key={window}><span>{windowName(window)}</span>{families.map(family => { const quota = view.windows[window][family]; return <div className={'mb-mini ' + family} title={reasonName(quota.reason) || quota.resets.map(resetLabel).join(' · ')} key={family}><Meter preferences={preferences} value={quota.remaining} label={label + ' ' + (family === 'gemini' ? 'Gemini' : 'Claude / GPT') + ' ' + windowName(window)} /><strong>{quotaDisplay(quota.remaining)}</strong></div>; })}</div>)}</div>
          </article>;
        })}</div>
        {pageCount > 1 && pager(visiblePage, pageCount, setPage)}
      </>}
    </div>
    <footer className="mb-footer">
      <button className="mb-usage" onClick={() => openPage('dashboard')}><span>{zh ? '今日用量' : 'Today’s usage'}</span><strong>{usage ? tokens(usage.today.total_tokens) : '—'} <small>tokens</small></strong><ChevronRight size={12} /></button>
      <div className="mb-footer-actions"><button onClick={() => openPage('accounts')}><Users size={13} />{zh ? '管理账号' : 'Accounts'}</button><button aria-label="GitHub" onClick={() => void request('plugin:opener|open_url', { url: 'https://github.com/anglee0323/antigravity-tools-lite' })}><ExternalLink size={14} /></button><button aria-label={zh ? '退出应用' : 'Quit'} onClick={() => void request('quit_app')}><LogOut size={14} /></button></div>
    </footer>
  </div>;
}
