import { request } from '../utils/request';
export interface UpdateInfo { current_version: string; latest_version: string; has_update: boolean; release_url: string }
export const checkForUpdates = () => request<UpdateInfo>('check_for_updates');
export const openRelease = (url: string) => {
    if (!/^https:\/\/github\.com\/anglee0323\/antigravity-tools-lite\/releases\/tag\/v?\d+\.\d+\.\d+$/.test(url)) return Promise.reject(new Error('invalid_release'));
    return request('plugin:opener|open_url', { url });
};
