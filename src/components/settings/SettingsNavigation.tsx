import { useEffect, useRef, useState } from 'react';
import { ArrowLeftRight, Database, FlaskConical, Monitor, RefreshCw, Sun } from 'lucide-react';
import { useTranslation } from 'react-i18next';

export const SETTINGS_SECTIONS = [
    { id: 'appearance', title: 'settings_sections.appearance', description: 'settings_sections.appearance_description', icon: Sun },
    { id: 'startup', title: 'settings_sections.startup', description: 'settings_sections.startup_description', icon: Monitor },
    { id: 'sync', title: 'settings_sections.sync', description: 'settings_sections.sync_description', icon: RefreshCw },
    { id: 'lowQuota', title: 'settings_sections.low_quota', description: 'auto_switch.description', icon: ArrowLeftRight },
    { id: 'data', title: 'settings_sections.data', description: 'settings_sections.data_description', icon: Database },
    { id: 'experimental', title: 'settings_sections.experimental', description: 'settings_sections.experimental_description', icon: FlaskConical },
] as const;
export type SettingsSection = typeof SETTINGS_SECTIONS[number]['id'];

export default function SettingsNavigation({ selected, onSelect }: { selected: SettingsSection; onSelect: (section: SettingsSection) => void }) {
    const { t } = useTranslation();
    const buttons = useRef<(HTMLButtonElement | null)[]>([]);
    const [narrow, setNarrow] = useState(false);
    useEffect(() => {
        const query = window.matchMedia('(max-width: 639px)');
        const update = () => setNarrow(query.matches);
        update(); query.addEventListener('change', update);
        return () => query.removeEventListener('change', update);
    }, []);
    return <div role="tablist" aria-label={t('settings_sections.navigation')} aria-orientation={narrow ? 'horizontal' : 'vertical'} className="settings-tabs">
        {SETTINGS_SECTIONS.map(({ id, title, icon: Icon }, index) => (
            <button key={id} ref={node => { buttons.current[index] = node; }} type="button" role="tab" id={`settings-tab-${id}`} aria-controls={`settings-panel-${id}`} aria-selected={selected === id} tabIndex={selected === id ? 0 : -1}
                onClick={() => onSelect(id)} onKeyDown={event => {
                    const next = event.key === 'Home' ? 0 : event.key === 'End' ? SETTINGS_SECTIONS.length - 1
                        : (narrow ? event.key === 'ArrowRight' : event.key === 'ArrowDown') ? (index + 1) % SETTINGS_SECTIONS.length
                        : (narrow ? event.key === 'ArrowLeft' : event.key === 'ArrowUp') ? (index + SETTINGS_SECTIONS.length - 1) % SETTINGS_SECTIONS.length : null;
                    if (next === null) return;
                    event.preventDefault(); onSelect(SETTINGS_SECTIONS[next].id); buttons.current[next]?.focus();
                }}
                className={`settings-tab focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-500 ${selected === id ? 'bg-blue-50/90 text-blue-600 font-semibold dark:bg-blue-500/15 dark:text-blue-300 shadow-sm' : 'text-slate-600 hover:bg-slate-200/50 dark:text-slate-400 dark:hover:bg-slate-800/60 hover:text-slate-900 dark:hover:text-slate-200'}`}>
                <Icon className={`h-4 w-4 shrink-0 ${selected === id ? 'text-blue-600 dark:text-blue-300' : 'text-slate-400 dark:text-slate-500'}`} aria-hidden="true" /><span>{t(title)}</span>
            </button>
        ))}
    </div>;
}
