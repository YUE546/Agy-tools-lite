import { useCallback, useEffect, useRef, useState } from 'react';
import { AlertTriangle, Languages, RefreshCw, RotateCcw } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useConfigStore } from '../../stores/useConfigStore';
import { request } from '../../utils/request';

interface LocalizationStatus {
    enabled: boolean;
    state: string;
    installed_version: string | null;
    dictionary_version: string;
    dictionary_entries: number;
    supported_versions: string[];
    can_apply: boolean;
    active: boolean;
    translated: number;
    supported: boolean;
    detail: string | null;
}

export default function AppLocalizationSettings() {
    const { t } = useTranslation();
    const { config, loadConfig } = useConfigStore();
    const [status, setStatus] = useState<LocalizationStatus | null>(null);
    const [busy, setBusy] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [needsDisableSave, setNeedsDisableSave] = useState(false);
    const mounted = useRef(true);
    const operation = useRef(false);
    const refreshing = useRef(false);
    const generation = useRef(0);
    const enabled = status?.enabled ?? config?.app_localization?.enabled ?? false;

    const refresh = useCallback(async () => {
        if (operation.current || refreshing.current) return;
        refreshing.current = true;
        const observedGeneration = generation.current;
        try {
            const next = await request<LocalizationStatus>('get_app_localization_status');
            if (mounted.current && !operation.current && generation.current === observedGeneration) setStatus(next);
        } catch (e) {
            if (mounted.current && !operation.current && generation.current === observedGeneration) setError(String(e));
        } finally {
            refreshing.current = false;
        }
    }, []);

    useEffect(() => {
        mounted.current = true;
        void refresh();
        const timer = window.setInterval(() => void refresh(), 5000);
        return () => { mounted.current = false; window.clearInterval(timer); };
    }, [refresh]);

    const run = async (command: string, args?: Record<string, unknown>) => {
        if (operation.current) return;
        operation.current = true;
        generation.current += 1;
        setBusy(true);
        setError(null);
        try {
            const next = await request<LocalizationStatus>(command, args);
            if (mounted.current) {
                setStatus(next);
                if (command === 'set_app_localization_enabled') setNeedsDisableSave(false);
            }
            await loadConfig();
        } catch (e) {
            if (mounted.current) {
                const code = String(e);
                setError(code);
                if (command === 'set_app_localization_enabled' && args?.enabled === false && code === 'disable_not_saved') setNeedsDisableSave(true);
            }
        } finally {
            operation.current = false;
            if (mounted.current) setBusy(false);
        }
    };

    const buttonClass = 'inline-flex items-center justify-center gap-2 rounded-lg border border-gray-200 bg-white px-3 py-2 text-xs font-medium text-gray-700 hover:border-blue-300 disabled:cursor-not-allowed disabled:opacity-50 dark:border-slate-600 dark:bg-slate-900 dark:text-gray-200';
    return (
        <section className="rounded-2xl border border-gray-200/80 bg-white p-5 sm:p-6 shadow-xs dark:border-slate-800 dark:bg-slate-900/80 lg:col-span-2" aria-labelledby="app-localization-title">
            <div className="flex items-start justify-between gap-4">
                <div className="flex items-center gap-3">
                    <span className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-sky-50 text-sky-600 dark:bg-sky-400/10 dark:text-sky-300"><Languages className="h-5 w-5" /></span>
                    <div>
                        <h3 id="app-localization-title" className="text-base font-semibold text-gray-900 dark:text-gray-100">{t('app_localization.title')}</h3>
                        <p className="mt-0.5 text-xs text-gray-500 dark:text-gray-400 leading-relaxed">{t('app_localization.subtitle')}</p>
                    </div>
                </div>
                <button type="button" role="switch" aria-label={t('app_localization.enable')} aria-checked={enabled} disabled={!config || busy || !status || (!enabled && !status.can_apply)} onClick={() => void run('set_app_localization_enabled', { enabled: !enabled })} className="mt-1 shrink-0 rounded-full focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-500 focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50">
                    <span className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${enabled ? 'bg-blue-600' : 'bg-gray-300 dark:bg-slate-600'}`}><span className={`inline-block h-5 w-5 transform rounded-full bg-white shadow-sm transition-transform ${enabled ? 'translate-x-[22px]' : 'translate-x-0.5'}`} /></span>
                </button>
            </div>
            <div className="mt-4 flex gap-2 rounded-xl bg-amber-50 p-3 text-xs leading-relaxed text-amber-800 dark:bg-amber-500/10 dark:text-amber-200"><AlertTriangle className="mt-0.5 h-4 w-4 shrink-0" /><p>{t('app_localization.notice')}</p></div>
            <div className="mt-4 flex flex-wrap items-center justify-between gap-3 rounded-xl border border-gray-200/80 bg-gray-50/70 p-4 dark:border-slate-800 dark:bg-slate-800/40">
                <div aria-live="polite" className="text-sm text-gray-800 dark:text-gray-100">
                    <div className="font-medium">{t(`app_localization.states.${status?.state ?? 'checking'}`)}</div>
                    <div className="mt-1 text-xs text-gray-500 dark:text-gray-400">{t('app_localization.version', { version: status?.installed_version ?? '—' })}</div>
                    {!!status?.translated && <div className="mt-1 text-xs text-gray-500 dark:text-gray-400">{t('app_localization.translated', { count: status.translated })}</div>}
                    {status?.detail && <div className="mt-1 max-w-2xl break-words text-xs text-gray-500 dark:text-gray-400">{t(`app_localization.details.${status.detail}`, { defaultValue: t('app_localization.details.runtime_failed') })}</div>}
                </div>
                <button type="button" className={buttonClass} disabled={busy} onClick={() => void refresh()}><RefreshCw className="h-3.5 w-3.5" />{t('app_localization.refresh')}</button>
            </div>
            <div className="mt-4 flex flex-wrap gap-2">
                <button type="button" className={buttonClass} disabled={busy || !enabled || !status?.can_apply} onClick={() => void run('apply_app_localization', { launch: false })}><RefreshCw className="h-3.5 w-3.5" />{t('app_localization.apply')}</button>
                <button type="button" className={buttonClass} disabled={busy || (!enabled && !status?.active && status?.state !== 'restore_pending' && !needsDisableSave)} onClick={() => void run('set_app_localization_enabled', { enabled: false })}><RotateCcw className="h-3.5 w-3.5" />{t('app_localization.restore')}</button>
            </div>
            {error && <p role="alert" className="mt-3 break-words text-xs text-red-600 dark:text-red-400">{t(`app_localization.details.${error}`, { defaultValue: t('app_localization.details.runtime_failed') })}</p>}
            <div className="mt-4 text-xs leading-relaxed text-gray-500 dark:text-gray-400">
                <p>{t('app_localization.coverage')}</p>
                <p className="mt-1">{t('app_localization.source', { version: status?.dictionary_version ?? 'atl-1', count: status?.dictionary_entries ?? 0 })} <a href="https://github.com/yiheng8023/antigravity-chinese/tree/573fa3c40aa6b410070a0b023730f0d1b4727bb9" target="_blank" rel="noreferrer" className="text-blue-600 underline dark:text-blue-400">antigravity-chinese</a> · <a href="https://github.com/yiheng8023/antigravity-chinese/blob/573fa3c40aa6b410070a0b023730f0d1b4727bb9/LICENSE" target="_blank" rel="noreferrer" className="text-blue-600 underline dark:text-blue-400">MIT</a></p>
                <p className="mt-1">{t('app_localization.stop_notice')}</p>
            </div>
        </section>
    );
}
