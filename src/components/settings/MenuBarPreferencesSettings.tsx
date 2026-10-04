import { useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useConfigStore } from '../../stores/useConfigStore';
import { request } from '../../utils/request';
import { isTauri } from '../../utils/env';
import { DEFAULT_MENU_BAR_PREFERENCES, type MenuBarPreferences, type MenuBarQuotaScope } from '../../types/config';
export default function MenuBarPreferencesSettings() {
  const { i18n } = useTranslation();
  const zh = i18n.language.startsWith('zh');
  const { config, loadConfig } = useConfigStore();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [pending, setPending] = useState<Partial<MenuBarPreferences>>({});
  const lock = useRef(false);
  const preferences = { ...DEFAULT_MENU_BAR_PREFERENCES, ...config?.menu_bar, ...pending };
  const disabled = busy || !config || !isTauri();
  const update = async (patch: Partial<MenuBarPreferences>) => {
    if (lock.current) return;
    lock.current = true; setBusy(true); setPending(patch); setError('');
    try {
      const next = await request<MenuBarPreferences>('set_menu_bar_preferences', { patch });
      useConfigStore.setState(state => ({ config: state.config ? { ...state.config, menu_bar: next } : null }));
      await loadConfig();
    } catch { setError(zh ? '保存失败，请检查选项后重试' : 'Could not save. Check these options and retry.'); }
    finally { lock.current = false; setPending({}); setBusy(false); }
  };
  const selectStyle = 'rounded-lg border border-gray-200 bg-gray-50 px-2 py-1.5 text-xs dark:border-slate-700 dark:bg-slate-800';
  const toggle = (key: 'hide_unavailable' | 'show_aggregate' | 'show_session' | 'show_weekly' | 'show_icons', text: string) => <label className="flex items-center gap-2 text-xs text-gray-700 dark:text-gray-300"><input type="checkbox" checked={preferences[key]} disabled={disabled || (key === 'show_session' && !preferences.show_weekly) || (key === 'show_weekly' && !preferences.show_session)} onChange={event => void update({ [key]: event.target.checked })} />{text}</label>;
  return <div className="mt-4 space-y-3 border-t border-gray-100 pt-4 dark:border-slate-800">
    <h3 className="text-xs font-semibold text-gray-800 dark:text-gray-200">{zh ? '菜单栏显示' : 'Menu bar display'}</h3>
    <div className="flex items-center justify-between gap-4"><label htmlFor="menu-bar-scope" className="text-xs">{zh ? '菜单栏聚合额度' : 'Menu bar aggregate quotas'}</label>
      <select id="menu-bar-scope" value={preferences.quota_scope} disabled={disabled} onChange={event => void update({ quota_scope: event.target.value as MenuBarQuotaScope })} className={selectStyle}><option value="all">{zh ? 'Gemini 与 Claude/GPT' : 'Gemini / Claude & GPT'}</option><option value="gemini">{zh ? 'Gemini 系列' : 'Gemini'}</option><option value="other">{zh ? 'Claude 和 GPT 系列' : 'Claude & GPT'}</option></select></div>
    <div className="flex items-center justify-between gap-4"><label htmlFor="menu-bar-display" className="text-xs">{zh ? '账号区显示系列' : 'Account quota family'}</label><select id="menu-bar-display" className={selectStyle} value={preferences.display_scope} disabled={disabled} onChange={event => void update({ display_scope: event.target.value as MenuBarQuotaScope })}><option value="gemini">{zh ? 'Gemini 系列' : 'Gemini'}</option><option value="other">{zh ? 'Claude 和 GPT 系列' : 'Claude & GPT'}</option><option value="all">{zh ? 'Gemini 与 Claude/GPT' : 'Gemini / Claude & GPT'}</option></select></div>
    <div className="flex items-center justify-between gap-4"><label htmlFor="menu-bar-label" className="text-xs">{zh ? '账号名称格式' : 'Account name format'}</label><select id="menu-bar-label" className={selectStyle} value={preferences.label_style} disabled={disabled} onChange={event => void update({ label_style: event.target.value as MenuBarPreferences['label_style'] })}><option value="email_then_label">{zh ? '邮箱优先' : 'Email first'}</option><option value="label_then_email">{zh ? '备注优先' : 'Note first'}</option><option value="email_only">{zh ? '仅邮箱' : 'Email only'}</option></select></div>
    <div className="grid grid-cols-2 gap-3">{toggle('show_aggregate', zh ? '显示整体额度' : 'Show aggregate quotas')}{toggle('hide_unavailable', zh ? '隐藏失效和禁用账号' : 'Hide invalid and disabled accounts')}{toggle('show_session', zh ? '显示 5 小时额度' : 'Show 5-hour quota')}{toggle('show_weekly', zh ? '显示周额度' : 'Show weekly quota')}{toggle('show_icons', zh ? '显示应用和模型图标' : 'Show app and model icons')}</div>
    <div className="flex flex-wrap items-center gap-3 text-xs"><label>{zh ? '绿色：高于' : 'Green: above'} <input aria-label={zh ? '绿色额度阈值' : 'Green quota threshold'} type="number" min={preferences.red_below + 1} max={100} defaultValue={preferences.green_above} key={'green' + preferences.green_above} disabled={disabled} className={selectStyle + ' w-16'} onBlur={event => { const value = Number(event.target.value); if (value !== preferences.green_above && Number.isInteger(value) && value > preferences.red_below && value <= 100) void update({ green_above: value }); else event.target.value = String(preferences.green_above); }} /> %</label><label>{zh ? '红色：低于' : 'Red: below'} <input aria-label={zh ? '红色额度阈值' : 'Red quota threshold'} type="number" min={0} max={preferences.green_above - 1} defaultValue={preferences.red_below} key={'red' + preferences.red_below} disabled={disabled} className={selectStyle + ' w-16'} onBlur={event => { const value = Number(event.target.value); if (value !== preferences.red_below && Number.isInteger(value) && value >= 0 && value < preferences.green_above) void update({ red_below: value }); else event.target.value = String(preferences.red_below); }} /> %</label><span className="text-gray-500">{zh ? '中间区间为黄色（含边界）' : 'Yellow between thresholds, including boundaries'}</span></div>
    <p className="text-[11px] leading-relaxed text-gray-400">{zh ? '整体额度分别统计 5 小时和周窗口，右侧显示可用账号及平均剩余比例。未报告或过期数据不计入平均；“可用”表示超过换号保留阈值，登录状态在切换时验证。至少保留一个额度窗口。选项自动保存，重新打开菜单即可生效。' : 'Each window shows available accounts and mean remaining on the right. Missing or stale data is excluded. Availability means above the switch reserve; sign-in is verified on switching. Keep at least one window. Changes save automatically and apply when reopening the menu.'}</p>{error && <p role="alert" className="mt-2 text-xs text-red-600">{error}</p>}
  </div>;
}
