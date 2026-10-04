import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useUpdateStore } from '../../stores/useUpdateStore';
import { openRelease } from '../../services/updaterService';
export default function UpdateDialog() {
    const { t } = useTranslation();
    const { updateInfo, isDialogOpen, setDialogOpen } = useUpdateStore();
    const close = useRef<HTMLButtonElement>(null), view = useRef<HTMLButtonElement>(null);
    const [error, setError] = useState(false);
    useEffect(() => { if (!isDialogOpen) return; const previous = document.activeElement as HTMLElement; setError(false); close.current?.focus(); return () => previous?.focus(); }, [isDialogOpen]);
    if (!isDialogOpen || !updateInfo) return null;
    return <div className="fixed inset-0 z-[10000] flex items-center justify-center bg-black/40 p-4" onClick={() => setDialogOpen(false)}>
        <section role="dialog" aria-modal="true" aria-labelledby="update-dialog-title" className="w-full max-w-md rounded-2xl bg-white p-6 shadow-xl dark:bg-slate-900" onClick={e => e.stopPropagation()} onKeyDown={e => {
            if (e.key === 'Escape') setDialogOpen(false);
            if (e.key === 'Tab') { e.preventDefault(); (document.activeElement === close.current ? view.current : close.current)?.focus(); }
        }}>
            <h2 id="update-dialog-title" className="text-lg font-semibold">{t('updater.dialog_title')}</h2>
            <p className="my-4 text-sm leading-relaxed text-gray-600 dark:text-gray-300">{t('updater.dialog_desc', { version: updateInfo.latest_version })}</p>
            {error && <p role="alert" className="mb-3 text-xs text-red-600">{t('updater.open_failed')}</p>}
            <div className="flex justify-end gap-3"><button ref={close} className="rounded-lg border border-gray-200 px-4 py-2 text-sm dark:border-slate-700" onClick={() => setDialogOpen(false)}>{t('updater.dismiss')}</button><button ref={view} className="rounded-lg bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700" onClick={() => void openRelease(updateInfo.release_url).then(() => setDialogOpen(false)).catch(() => setError(true))}>{t('updater.open_release')}</button></div>
        </section>
    </div>;
}
