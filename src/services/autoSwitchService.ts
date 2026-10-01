import { request } from '../utils/request';
import { AutoSwitchConfig, AutoSwitchStatus } from '../types/autoSwitch';
export const getAutoSwitchConfig = () => request<AutoSwitchConfig>('get_auto_switch_config');
export const setAutoSwitchConfig = (config: AutoSwitchConfig) => request<AutoSwitchConfig>('set_auto_switch_config', { config });
export const getAutoSwitchStatus = () => request<AutoSwitchStatus>('get_auto_switch_status');
export const checkAutoSwitchNow = () => request<AutoSwitchStatus>('check_auto_switch_now');
export const cancelAutoSwitch = (pendingId: string) => request<AutoSwitchStatus>('cancel_auto_switch', { pendingId });
