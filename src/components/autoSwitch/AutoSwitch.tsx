import { useEffect, useRef, useState } from 'react';
import { ArrowLeftRight, RefreshCw } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Account } from '../../types/account';
import { AutoSwitchConfig, AutoSwitchStatus } from '../../types/autoSwitch';
import * as service from '../../services/autoSwitchService';
import { listAccounts, getCurrentAccount } from '../../services/accountService';
import { isTauri } from '../../utils/env';

function useStatus() {
    const [status, setStatus] = useState<AutoSwitchStatus | null>(null);
    const [error, setError] = useState('');
    useEffect(() => {
        if (!isTauri()) return;
        let live = true; let reading = false;
        const read = async () => {
            if (reading) return; reading = true;
            try { const s = await service.getAutoSwitchStatus(); if (live) { setStatus(s); setError(''); } }
            catch { if (live) setError('status_failed'); }
            finally { reading = false; }
        };
        void read(); const timer = setInterval(read, 3000);
        return () => { live = false; clearInterval(timer); };
    }, []);
    return { status, setStatus, error };
}

function StatusBody({ status, compact = false }: { status: AutoSwitchStatus; compact?: boolean }) {
    const { t } = useTranslation();
    const [busy, setBusy] = useState(false); const [error, setError] = useState('');
    const [guide, setGuide] = useState(false); const close = useRef<HTMLButtonElement>(null);
    const guideTrigger = useRef<HTMLButtonElement>(null);
    useEffect(() => { if (guide) close.current?.focus(); }, [guide]);
    const dismiss = () => { setGuide(false); requestAnimationFrame(() => guideTrigger.current?.focus()); };
    const reason = status.reason || (status.phase === 'monitoring' ? 'monitoring' : 'checking');
    const action = async (cancel: boolean) => {
        setBusy(true); setError('');
        try { if (cancel && status.pending_id) await service.cancelAutoSwitch(status.pending_id); else await service.checkAutoSwitchNow(); }
        catch { setError(t('auto_switch.action_failed')); }
        finally { setBusy(false); }
    };
    return <div className={compact ? 'px-4 py-3' : 'rounded-xl bg-slate-50 p-4 dark:bg-slate-800/60'}>
        <div className="flex flex-wrap items-start justify-between gap-3">
            <div className="min-w-0 space-y-1">
                <p className="text-sm font-medium text-slate-800 dark:text-slate-100" role="status">
                    {status.remaining_percentage !== null && ['pending', 'switching'].includes(status.phase)
                        ? t('auto_switch.remaining', { percent: Math.floor(status.remaining_percentage) }) + ' · ' : ''}
                    {t(`auto_switch.reasons.${reason}`, { defaultValue: t('auto_switch.reasons.switch_failed') })}
                </p>
                {status.target_email && <p className="break-all text-xs text-slate-500 dark:text-slate-400">{t('auto_switch.next_account', { email: status.target_email })}</p>}
                {status.reason === 'clients_running' && <p className="max-w-3xl text-xs leading-relaxed text-slate-500 dark:text-slate-400">{t(`auto_switch.${status.mode}_instructions`)}</p>}
                {status.phase === 'completed' && <p className="text-xs text-slate-500 dark:text-slate-400">{t('auto_switch.manual_continue')}</p>}
            </div>
            <div className="flex shrink-0 flex-wrap gap-2">
                {status.mode === 'stop' && status.reason === 'clients_running' && <button ref={guideTrigger} onClick={() => setGuide(true)} className="btn btn-sm btn-outline">{t('auto_switch.stop_guide')}</button>}
                {status.pending_id && <button disabled={busy || status.phase === 'switching'} onClick={() => action(true)} className="btn btn-sm btn-ghost">{t('auto_switch.cancel')}</button>}
                <button disabled={busy || status.phase === 'switching'} onClick={() => action(false)} className="btn btn-sm btn-ghost"><RefreshCw size={14} className={busy ? 'animate-spin' : ''} />{t('auto_switch.check_now')}</button>
            </div>
        </div>
        {error && <p role="alert" className="mt-2 text-xs text-red-600">{error}</p>}
        {guide && <div className="fixed inset-0 z-[10000] flex items-center justify-center bg-black/40 p-4" onClick={dismiss}>
            <section role="dialog" aria-modal="true" aria-labelledby="auto-switch-guide-title" className="w-full max-w-md rounded-2xl bg-white p-6 shadow-xl dark:bg-slate-900" onClick={e => e.stopPropagation()} onKeyDown={e => {
                if (e.key === 'Escape') { e.preventDefault(); dismiss(); }
                // This dialog has one action; keep keyboard focus inside it.
                if (e.key === 'Tab') { e.preventDefault(); close.current?.focus(); }
            }}>
                <h3 id="auto-switch-guide-title" className="font-semibold">{t('auto_switch.stop_guide')}</h3>
                <ol className="my-4 list-decimal space-y-3 pl-5 text-sm leading-relaxed text-slate-600 dark:text-slate-300">
                    <li>{t('auto_switch.stop_step_1')}</li><li>{t('auto_switch.stop_step_2')}</li><li>{t('auto_switch.stop_step_3')}</li>
                </ol>
                <p className="mb-4 text-xs text-amber-700 dark:text-amber-300">{t('auto_switch.stop_warning')}</p>
                <button ref={close} onClick={dismiss} className="btn btn-primary w-full">{t('auto_switch.understood')}</button>
            </section>
        </div>}
    </div>;
}

export function AutoSwitchStatusBar() {
    const { t } = useTranslation(); const { status, error } = useStatus();
    if (!status || ['disabled', 'monitoring'].includes(status.phase)) return null;
    return <aside aria-label={t('auto_switch.title')} className="shrink-0 border-b border-amber-200 bg-amber-50/80 dark:border-amber-800/60 dark:bg-amber-950/20">
        {error ? <p className="px-4 py-3 text-sm" role="alert">{t(`auto_switch.${error}`)}</p> : <StatusBody status={status} compact />}
    </aside>;
}

export function AutoSwitchSettings() {
    const { t } = useTranslation(); const { status, error: statusError } = useStatus();
    const [draft, setDraft] = useState<AutoSwitchConfig | null>(null);
    const [accounts, setAccounts] = useState<Account[]>([]); const [currentId, setCurrentId] = useState<string | null>(null);
    const [error, setError] = useState(''); const [saved, setSaved] = useState(false); const [busy, setBusy] = useState(false);
    const reload = async () => {
        setBusy(true); setError('');
        try { const [c, a, current] = await Promise.all([service.getAutoSwitchConfig(), listAccounts(), getCurrentAccount()]); setDraft(c); setAccounts(a); setCurrentId(current?.id || null); }
        catch { setError(t('auto_switch.load_failed')); } finally { setBusy(false); }
    };
    useEffect(() => { void reload(); }, []);
    const patch = (p: Partial<AutoSwitchConfig>) => { setDraft(d => d && ({ ...d, ...p })); setSaved(false); };
    const save = async () => {
        if (!draft) return; setBusy(true); setError(''); setSaved(false);
        try { setDraft(await service.setAutoSwitchConfig(draft)); setSaved(true); }
        catch (e) { setError(t('auto_switch.save_failed', { error: String(e) })); }
        finally { setBusy(false); }
    };
    const models = [...new Map(accounts.flatMap(a => a.quota?.models || []).map(m => [m.name, m.display_name || m.name])).entries()].sort((a, b) => a[1].localeCompare(b[1]));
    const invalid = !draft || draft.reserve_percentage < 1 || draft.reserve_percentage > 98 || draft.candidate_min_percentage <= draft.reserve_percentage || draft.candidate_min_percentage > 100 || !Number.isInteger(draft.reserve_percentage) || !Number.isInteger(draft.candidate_min_percentage) || (draft.enabled && (!draft.monitored_model || !draft.candidate_account_ids.length));
    return <section className="rounded-2xl border border-gray-200/80 bg-white p-5 shadow-sm dark:border-slate-700 dark:bg-slate-900">
        <div className="flex items-start gap-3"><span className="rounded-xl bg-amber-50 p-2.5 text-amber-600 dark:bg-amber-400/10"><ArrowLeftRight size={20} /></span><div><h2 className="font-semibold">{t('auto_switch.title')}</h2><p className="mt-1 max-w-3xl text-xs leading-relaxed text-slate-500 dark:text-slate-400">{t('auto_switch.description')}</p></div></div>
        {error && <p role="alert" className="mt-3 text-sm text-red-600">{error}</p>}
        {!draft ? <button disabled={busy} onClick={reload} className="btn btn-sm mt-4">{t(busy ? 'auto_switch.loading' : 'auto_switch.retry')}</button> : <>
            <label className="my-5 flex cursor-pointer items-center gap-3 text-sm font-medium"><input type="checkbox" className="toggle toggle-primary toggle-sm" checked={draft.enabled} disabled={busy} onChange={e => patch({ enabled: e.target.checked })} />{t('auto_switch.enable')}</label>
            <div className="grid gap-3 md:grid-cols-2" role="group" aria-label={t('auto_switch.mode')}>
                {(['wait', 'stop'] as const).map(mode => <label key={mode} className={`flex cursor-pointer items-start gap-3 rounded-xl border p-4 ${draft.mode === mode ? 'border-blue-400 bg-blue-50/60 dark:bg-blue-500/10' : 'border-slate-200 dark:border-slate-700'}`}>
                    <input type="radio" name="auto-switch-mode" checked={draft.mode === mode} disabled={busy} onChange={() => patch({ mode })} className="radio radio-primary radio-sm mt-0.5" />
                    <span><span className="block text-sm font-medium">{t(`auto_switch.mode_${mode}`)}</span><span className="mt-1 block text-xs leading-relaxed text-slate-500 dark:text-slate-400">{t(`auto_switch.${mode}_instructions`)}</span></span>
                </label>)}
            </div>
            <div className="my-5 grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
                <label className="space-y-2 text-xs text-slate-600 dark:text-slate-300"><span className="block">{t('auto_switch.target')}</span><select className="select select-bordered select-sm w-full" disabled={busy} value={draft.target} onChange={e => patch({ target: e.target.value as 'app' })}><option value="app">Antigravity APP + agy</option></select></label>
                <label className="space-y-2 text-xs text-slate-600 dark:text-slate-300"><span className="block">{t('auto_switch.model')}</span><select className="select select-bordered select-sm w-full" disabled={busy} value={draft.monitored_model} onChange={e => patch({ monitored_model: e.target.value })}><option value="">{t('auto_switch.select_model')}</option>{draft.monitored_model && !models.some(([id]) => id === draft.monitored_model) && <option value={draft.monitored_model}>{draft.monitored_model}</option>}{models.map(([id, label]) => <option key={id} value={id}>{label}</option>)}</select></label>
                <label className="space-y-2 text-xs text-slate-600 dark:text-slate-300"><span className="block">{t('auto_switch.reserve')}</span><input className="input input-bordered input-sm w-full" type="number" min={1} max={98} step={1} disabled={busy} value={draft.reserve_percentage} onChange={e => patch({ reserve_percentage: Number(e.target.value) })} /></label>
                <label className="space-y-2 text-xs text-slate-600 dark:text-slate-300"><span className="block">{t('auto_switch.candidate_min')}</span><input className="input input-bordered input-sm w-full" type="number" min={draft.reserve_percentage + 1} max={100} step={1} disabled={busy} value={draft.candidate_min_percentage} onChange={e => patch({ candidate_min_percentage: Number(e.target.value) })} /></label>
            </div>
            <fieldset className="space-y-2"><legend className="mb-2 text-sm font-medium">{t('auto_switch.accounts')}</legend><p className="mb-3 text-xs text-slate-500">{t('auto_switch.accounts_hint')}</p>
                {!accounts.length && <p className="text-xs text-amber-700">{t('auto_switch.no_accounts')}</p>}
                <div className="grid max-h-52 gap-2 overflow-y-auto sm:grid-cols-2">{accounts.map(a => <label key={a.id} className="flex cursor-pointer items-start gap-2 rounded-lg border border-slate-200 p-3 text-sm dark:border-slate-700"><input type="checkbox" className="checkbox checkbox-primary checkbox-sm mt-0.5" disabled={busy} checked={draft.candidate_account_ids.includes(a.id)} onChange={e => patch({ candidate_account_ids: e.target.checked ? [...draft.candidate_account_ids, a.id] : draft.candidate_account_ids.filter(id => id !== a.id) })} /><span className="min-w-0 break-all">{a.email}{a.id === currentId && <span className="ml-2 text-xs text-blue-500">{t('auto_switch.current')}</span>}</span></label>)}</div>
            </fieldset>
            <p className="mt-4 text-xs leading-relaxed text-amber-700 dark:text-amber-300">{t('auto_switch.safety_note')}</p>
            <div className="my-4 flex items-center gap-3"><button className="btn btn-primary btn-sm" disabled={busy || invalid} onClick={save}>{t(busy ? 'auto_switch.saving' : 'auto_switch.save')}</button>{saved && <span role="status" className="text-xs text-emerald-600">{t('auto_switch.saved')}</span>}</div>
            {statusError && <p role="alert" className="text-xs text-red-600">{t(`auto_switch.${statusError}`)}</p>}
            {status && status.phase !== 'disabled' && <StatusBody status={status} />}
        </>}
    </section>;
}
