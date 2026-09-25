import { useEffect, useState } from 'react';
import { Check, Database, FolderOpen, Globe2, HardDrive, Monitor, Moon, ShieldCheck, Sun } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useConfigStore } from '../stores/useConfigStore';
import { AppConfig } from '../types/config';
import { request as invoke } from '../utils/request';
import { showToast } from '../components/common/ToastContainer';

const LANGUAGES = [
    { code: 'zh', label: '简体中文' },
    { code: 'en', label: 'English' },
];

function Settings() {
    const { t } = useTranslation();
    const { config, loadConfig, saveConfig } = useConfigStore();
    const [dataDirPath, setDataDirPath] = useState('~/.antigravity_tools');

    useEffect(() => {
        loadConfig();
        invoke<string>('get_data_dir_path')
            .then(setDataDirPath)
            .catch(() => {});
    }, [loadConfig]);

    const updateConfig = async (patch: Partial<AppConfig>) => {
        if (!config) return;
        try {
            await saveConfig({ ...config, ...patch }, true);
            showToast(t('local_settings.saved'), 'success');
        } catch (error) {
            showToast(t('local_settings.save_failed', { error: String(error) }), 'error');
        }
    };

    const openDataFolder = async () => {
        try {
            await invoke('open_data_folder');
        } catch (error) {
            showToast(t('local_settings.open_failed', { error: String(error) }), 'error');
        }
    };

    const themeOptions = [
        { value: 'system', label: t('local_settings.system'), icon: Monitor },
        { value: 'light', label: t('local_settings.light'), icon: Sun },
        { value: 'dark', label: t('local_settings.dark'), icon: Moon },
    ];
    const selectedLanguage = config?.language?.toLowerCase().startsWith('en') ? 'en' : 'zh';

    return (
        <div className="h-full w-full overflow-y-auto">
            <div className="mx-auto max-w-5xl space-y-5 px-4 py-6 md:px-6">
                <header>
                    <h1 className="text-2xl font-bold text-gray-900 dark:text-base-content">{t('local_settings.title')}</h1>
                    <p className="mt-1 text-sm text-gray-500 dark:text-gray-400">{t('local_settings.subtitle')}</p>
                </header>

                <div className="grid gap-4 lg:grid-cols-2">
                    <section className="rounded-2xl border border-gray-200/80 bg-white p-5 shadow-sm dark:border-slate-700 dark:bg-slate-900">
                        <div className="flex items-center gap-3">
                            <span className="flex h-10 w-10 items-center justify-center rounded-xl bg-blue-50 text-blue-600 dark:bg-blue-400/10 dark:text-blue-300">
                                <Monitor className="h-5 w-5" />
                            </span>
                            <div>
                                <h2 className="font-semibold text-gray-900 dark:text-gray-100">{t('local_settings.appearance')}</h2>
                                <p className="mt-0.5 text-xs text-gray-500 dark:text-gray-400">{t('local_settings.appearance_desc')}</p>
                            </div>
                        </div>

                        <div className="mt-5">
                            <div className="mb-2 text-xs font-semibold uppercase tracking-wide text-gray-500 dark:text-gray-400">{t('local_settings.theme')}</div>
                            <div className="grid grid-cols-3 gap-2">
                                {themeOptions.map(({ value, label, icon: Icon }) => {
                                    const selected = (config?.theme || 'system') === value;
                                    return (
                                        <button
                                            key={value}
                                            type="button"
                                            onClick={() => updateConfig({ theme: value })}
                                            disabled={!config}
                                            aria-pressed={selected}
                                            className={`flex min-h-[76px] flex-col items-center justify-center gap-2 rounded-xl border px-2 py-3 text-sm transition-colors disabled:cursor-not-allowed disabled:opacity-60 ${selected
                                                ? 'border-blue-300 bg-blue-50 text-blue-700 shadow-sm dark:border-blue-500/50 dark:bg-blue-500/10 dark:text-blue-300'
                                                : 'border-gray-200 bg-gray-50/70 text-gray-600 hover:border-gray-300 hover:bg-gray-100 dark:border-slate-700 dark:bg-slate-800 dark:text-gray-300 dark:hover:border-slate-600 dark:hover:bg-slate-800/70'
                                                }`}
                                        >
                                            <Icon className="h-5 w-5" />
                                            <span>{label}</span>
                                        </button>
                                    );
                                })}
                            </div>
                        </div>

                        <div className="mt-5">
                            <div className="mb-2 flex items-center gap-2 text-xs font-semibold uppercase tracking-wide text-gray-500 dark:text-gray-400">
                                <Globe2 className="h-3.5 w-3.5" />
                                {t('local_settings.language')}
                            </div>
                            <div className="grid grid-cols-2 gap-2 rounded-xl bg-gray-100 p-1 dark:bg-slate-800">
                                {LANGUAGES.map((language) => {
                                    const selected = selectedLanguage === language.code;
                                    return (
                                        <button
                                            key={language.code}
                                            type="button"
                                            onClick={() => updateConfig({ language: language.code })}
                                            disabled={!config}
                                            aria-pressed={selected}
                                            className={`flex items-center justify-center gap-2 rounded-lg px-3 py-2.5 text-sm font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-60 ${selected
                                                ? 'bg-white text-blue-700 shadow-sm dark:bg-slate-700 dark:text-blue-300'
                                                : 'text-gray-500 hover:text-gray-800 dark:text-gray-400 dark:hover:text-gray-100'
                                                }`}
                                        >
                                            {selected && <Check className="h-4 w-4" />}
                                            {language.label}
                                        </button>
                                    );
                                })}
                            </div>
                        </div>
                    </section>

                    <section className="rounded-2xl border border-gray-200/80 bg-white p-5 shadow-sm dark:border-slate-700 dark:bg-slate-900">
                        <div className="flex items-center gap-3">
                            <span className="flex h-10 w-10 items-center justify-center rounded-xl bg-emerald-50 text-emerald-600 dark:bg-emerald-400/10 dark:text-emerald-300">
                                <Database className="h-5 w-5" />
                            </span>
                            <div>
                                <h2 className="font-semibold text-gray-900 dark:text-gray-100">{t('local_settings.local_data')}</h2>
                                <p className="mt-0.5 text-xs text-gray-500 dark:text-gray-400">{t('local_settings.local_data_desc')}</p>
                            </div>
                        </div>

                        <div className="mt-5 rounded-xl border border-gray-200 bg-gray-50 p-4 dark:border-slate-700 dark:bg-slate-800">
                            <div className="flex flex-wrap items-center justify-between gap-3">
                                <div className="flex min-w-0 items-start gap-3">
                                    <HardDrive className="mt-0.5 h-4 w-4 shrink-0 text-gray-400 dark:text-gray-500" />
                                    <div className="min-w-0">
                                        <div className="text-sm font-medium text-gray-700 dark:text-gray-200">{t('local_settings.data_directory')}</div>
                                        <div className="mt-1 break-all font-mono text-xs leading-relaxed text-gray-500 dark:text-gray-400">{dataDirPath}</div>
                                    </div>
                                </div>
                                <button
                                    type="button"
                                    onClick={openDataFolder}
                                    className="flex shrink-0 items-center gap-2 rounded-lg border border-gray-200 bg-white px-3 py-2 text-xs font-medium text-gray-600 transition-colors hover:border-blue-300 hover:text-blue-600 dark:border-slate-600 dark:bg-slate-900 dark:text-gray-300 dark:hover:border-blue-500 dark:hover:text-blue-300"
                                >
                                    <FolderOpen className="h-4 w-4" />
                                    {t('local_settings.open_directory')}
                                </button>
                            </div>
                        </div>

                        <div className="mt-3 grid gap-3 sm:grid-cols-2">
                            <div className="rounded-xl border border-gray-100 bg-white p-3 dark:border-slate-700 dark:bg-slate-800/60">
                                <ShieldCheck className="mb-2 h-4 w-4 text-emerald-500" />
                                <div className="text-xs font-semibold text-gray-700 dark:text-gray-200">{t('local_settings.account_data_title')}</div>
                                <p className="mt-1 text-xs leading-relaxed text-gray-500 dark:text-gray-400">{t('local_settings.account_data_desc')}</p>
                            </div>
                            <div className="rounded-xl border border-gray-100 bg-white p-3 dark:border-slate-700 dark:bg-slate-800/60">
                                <Database className="mb-2 h-4 w-4 text-blue-500" />
                                <div className="text-xs font-semibold text-gray-700 dark:text-gray-200">{t('local_settings.token_data_title')}</div>
                                <p className="mt-1 text-xs leading-relaxed text-gray-500 dark:text-gray-400">{t('local_settings.token_data_desc')}</p>
                            </div>
                        </div>
                    </section>
                </div>
            </div>
        </div>
    );
}

export default Settings;
