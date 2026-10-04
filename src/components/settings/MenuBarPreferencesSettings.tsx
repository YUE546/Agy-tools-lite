import { useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useConfigStore } from '../../stores/useConfigStore';
import { request } from '../../utils/request';
import { isTauri } from '../../utils/env';
import type { MenuBarPreferences, MenuBarQuotaScope } from '../../types/config';
export default function MenuBarPreferencesSettings() {
  const { i18n } = useTranslation();
  const zh = i18n.language.startsWith('zh');
  const { config, loadConfig } = useConfigStore();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const lock = useRef(false);
  const update = async (quotaScope: MenuBarQuotaScope) => {
    if (lock.current) return;
    lock.current = true; setBusy(true); setError('');
    try {
      const next = await request<MenuBarPreferences>('set_menu_bar_preferences', { quotaScope });
      useConfigStore.setState(state => ({ config: state.config ? { ...state.config, menu_bar: next } : null }));
      await loadConfig();
    } catch { setError(zh ? '保存失败，请重试' : 'Could not save. Retry.'); }
    finally { lock.current = false; setBusy(false); }
  };
  return <div className="mt-4 border-t border-gray-100 pt-4 dark:border-slate-800">
    <div className="flex items-start justify-between gap-4"><div><label htmlFor="menu-bar-scope" className="text-xs font-semibold text-gray-800 dark:text-gray-200">{zh ? '菜单栏聚合额度' : 'Menu bar aggregate quotas'}</label><p className="mt-1 text-xs leading-relaxed text-gray-500 dark:text-gray-400">{zh ? '分别显示 5 小时和周额度的平均剩余，并统计可用账号。' : 'Mean remaining for 5-hour and weekly windows, with available account counts.'}</p></div>
      <select id="menu-bar-scope" value={config?.menu_bar?.quota_scope ?? 'all'} disabled={busy || !config || !isTauri()} onChange={event => void update(event.target.value as MenuBarQuotaScope)} className="rounded-lg border border-gray-200 bg-gray-50 px-2 py-1.5 text-xs dark:border-slate-700 dark:bg-slate-800"><option value="all">{zh ? '全部系列' : 'All families'}</option><option value="gemini">Gemini</option><option value="other">Claude / GPT</option></select>
    </div><p className="mt-2 text-[11px] text-gray-400">{zh ? '共享池去重；未报告或过期的数据不计入平均值。“可用”表示额度高于自动切号的保留阈值，登录状态在切换时验证。' : 'Shared pools are counted once. Missing or stale data is excluded. Available means above the auto-switch reserve; sign-in is verified when switching.'}</p>{error && <p role="alert" className="mt-2 text-xs text-red-600">{error}</p>}
  </div>;
}
