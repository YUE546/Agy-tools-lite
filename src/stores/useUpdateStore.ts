import { create } from 'zustand';
import { checkForUpdates, UpdateInfo } from '../services/updaterService';
interface UpdateState {
    updateInfo: UpdateInfo | null; isChecking: boolean; error: boolean; isDialogOpen: boolean;
    checkForUpdates: (silent?: boolean) => Promise<void>;
    setDialogOpen: (open: boolean) => void;
}
export const useUpdateStore = create<UpdateState>((set, get) => ({
    updateInfo: null, isChecking: false, error: false, isDialogOpen: false,
    checkForUpdates: async (silent = false) => {
        if (get().isChecking) return;
        set({ isChecking: true, error: false });
        try {
            const info = await checkForUpdates();
            if (!info) throw new Error('invalid_release');
            set({ updateInfo: info, isChecking: false, isDialogOpen: info.has_update && (!silent || localStorage.getItem('dismissed_release') !== info.latest_version) });
        } catch { set({ isChecking: false, error: !silent }); }
    },
    setDialogOpen: open => {
        if (!open && get().updateInfo) localStorage.setItem('dismissed_release', get().updateInfo!.latest_version);
        set({ isDialogOpen: open });
    },
}));
