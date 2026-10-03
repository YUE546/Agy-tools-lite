/**
 * 缁熶竴鍚庣璋冪敤妗ワ紙鏃?WebView 鏈嶅姟鐗堬級
 *
 * - 娴忚鍣紙绗竴浼樺厛锛夛細鎸?COMMAND_MAPPING 鎶婂懡浠ゅ悕鏄犲皠涓?`/api/*` HTTP 璋冪敤锛? *   鍑嵁浠?sessionStorage锛堢櫥褰曞畧鍗啓鍏ワ級闄勫姞鍒?Authorization / x-api-key 澶达紱
 * - Tauri锛堝吋瀹?lite 妗岄潰澹冲唴宓岋級锛氬姩鎬?import invoke 鐩磋皟锛岃涓轰笉鍙樸€? *
 * 闃插洖褰掞細`npm run check:parity` 浜ゅ弶鏍稿 COMMAND_MAPPING 鈫?server.rs 璺敱 鈫? * 鍓嶇瀹為檯璋冪敤锛學eb 妯″紡纭け璐ワ紙鏈槧灏勫懡浠わ級鍦ㄦ瀯寤烘湡鏆撮湶銆? */

const isTauri =
  typeof window !== 'undefined' &&
  (!!(window as any).__TAURI_INTERNALS__ || !!(window as any).__TAURI__);

interface CommandRoute {
  url: string;
  method: 'GET' | 'POST';
}

/** 鍛戒护 鈫?HTTP 鏄犲皠锛泂erver.rs 鐨?api_router() 閫愪竴瀵瑰簲 */
const COMMAND_MAPPING: Record<string, CommandRoute> = {
  // 璐﹀彿
  'list_accounts': { url: '/api/list_accounts', method: 'POST' },
  'get_current_account': { url: '/api/get_current_account', method: 'POST' },
  'get_account_dashboard_snapshot': { url: '/api/get_account_dashboard_snapshot', method: 'POST' },
  'add_account': { url: '/api/add_account', method: 'POST' },
  'delete_account': { url: '/api/delete_account', method: 'POST' },
  'delete_accounts': { url: '/api/delete_accounts', method: 'POST' },
  'reorder_accounts': { url: '/api/reorder_accounts', method: 'POST' },
  'switch_account': { url: '/api/switch_account', method: 'POST' },
  'fetch_account_quota': { url: '/api/fetch_account_quota', method: 'POST' },
  'refresh_all_quotas': { url: '/api/refresh_all_quotas', method: 'POST' },
  'export_accounts': { url: '/api/export_accounts', method: 'POST' },
  'update_account_label': { url: '/api/update_account_label', method: 'POST' },
  'import_from_db': { url: '/api/import_from_db', method: 'POST' },
  'import_custom_db': { url: '/api/import_custom_db', method: 'POST' },
  'import_custom_db_upload': { url: '/api/import_custom_db_upload', method: 'POST' },
  'sync_account_from_db': { url: '/api/sync_account_from_db', method: 'POST' },
  // OAuth
  'start_oauth_login': { url: '/api/start_oauth_login', method: 'POST' },
  'complete_oauth_login': { url: '/api/complete_oauth_login', method: 'POST' },
  'prepare_oauth_url': { url: '/api/prepare_oauth_url', method: 'POST' },
  'cancel_oauth_login': { url: '/api/cancel_oauth_login', method: 'POST' },
  'submit_oauth_code': { url: '/api/submit_oauth_code', method: 'POST' },
  'list_oauth_clients': { url: '/api/list_oauth_clients', method: 'POST' },
  'get_active_oauth_client': { url: '/api/get_active_oauth_client', method: 'POST' },
  'set_active_oauth_client': { url: '/api/set_active_oauth_client', method: 'POST' },
  // 閰嶇疆
  'load_config': { url: '/api/load_config', method: 'POST' },
  'save_config': { url: '/api/save_config', method: 'POST' },
  // 浣庨厤棰濊嚜鍔ㄥ垏鎹?  get_auto_switch_config: { url: '/api/get_auto_switch_config', method: 'POST' },
  'set_auto_switch_config': { url: '/api/set_auto_switch_config', method: 'POST' },
  'get_auto_switch_status': { url: '/api/get_auto_switch_status', method: 'POST' },
  'cancel_auto_switch': { url: '/api/cancel_auto_switch', method: 'POST' },
  'check_auto_switch_now': { url: '/api/check_auto_switch_now', method: 'POST' },
  // 鏈湴鍖?  get_app_localization_status: { url: '/api/get_app_localization_status', method: 'POST' },
  'set_app_localization_enabled': { url: '/api/set_app_localization_enabled', method: 'POST' },
  'apply_app_localization': { url: '/api/apply_app_localization', method: 'POST' },
  // 鏉傞」
  'get_data_dir_path': { url: '/api/get_data_dir_path', method: 'POST' },
  'open_data_folder': { url: '/api/open_data_folder', method: 'POST' },
  'get_local_token_usage': { url: '/api/get_local_token_usage', method: 'POST' },
  'get_api_pricing': { url: '/api/get_api_pricing', method: 'POST' },
};

/** 妗岄潰澹充笓灞炲懡浠わ細Web 妯″紡闈欓粯涓虹┖鎿嶄綔锛堟墭鐩?绐楀彛璇箟鍦ㄦ祻瑙堝櫒涓笉瀛樺湪锛?*/
const DESKTOP_ONLY = new Set([
  'show_main_window',
  'set_window_theme',
  'quit_app',
  'hide_menu_bar_dashboard',
  'get_menu_bar_appearance',
  'open_app_page',
]);

function getApiKey(): string | null {
  return typeof window !== 'undefined'
    ? sessionStorage.getItem('abv_admin_api_key')
    : null;
}

function authHeaders(): Record<string, string> {
  const apiKey = getApiKey();
  return apiKey
    ? { Authorization: `Bearer ${apiKey}`, 'x-api-key': apiKey }
    : {};
}

async function request<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (isTauri) {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      return await invoke<T>(cmd, args);
    } catch (error) {
      console.error(`Tauri Invoke Error [${cmd}]:`, error);
      throw error;
    }
  }

  if (DESKTOP_ONLY.has(cmd)) {
    // 绐楀彛/鎵樼洏璇箟鍦?Web 妯″紡涓嶅瓨鍦細闈欓粯绌烘搷浣?    return undefined as T;
  }

  const route = COMMAND_MAPPING[cmd];
  if (!route) {
    throw new Error(`Command [${cmd}] is not supported in Web mode.`);
  }

  const res = await fetch(route.url, {
    method: route.method,
    headers: {
      'Content-Type': 'application/json',
      ...authHeaders(),
    },
    body: JSON.stringify(args ?? {}),
  });

  if (res.status === 401) {
    sessionStorage.removeItem('abv_admin_api_key');
    window.dispatchEvent(new CustomEvent('abv-unauthorized'));
    throw new Error('unauthorized');
  }

  if (!res.ok) {
    const text = await res.text().catch(() => '');
    let message = text;
    try {
      const parsed = JSON.parse(text);
      message = typeof parsed === 'string' ? parsed : (parsed.message ?? text);
    } catch {
      /* 闈?JSON 閿欒浣撲繚鐣欏師鏂?*/
    }
    throw new Error(message || `HTTP ${res.status}`);
  }

  const text = await res.text();
  if (!text) return undefined as T;
  return JSON.parse(text) as T;
}

/** Web 妯″紡 multipart 涓婁紶锛?vscdb 瀵煎叆鐩翠紶锛夛紝浠呮祻瑙堝櫒閫氶亾浣跨敤 */
async function uploadRequest<T>(cmd: string, file: File | Blob, fileName?: string): Promise<T> {
  const route = COMMAND_MAPPING[cmd];
  if (!route) {
    throw new Error(`Command [${cmd}] is not supported in Web mode.`);
  }

  const form = new FormData();
  form.append('file', file, fileName ?? (file as File).name ?? 'import.vscdb');

  const res = await fetch(route.url, {
    method: 'POST',
    headers: authHeaders(),
    body: form,
  });

  if (res.status === 401) {
    sessionStorage.removeItem('abv_admin_api_key');
    window.dispatchEvent(new CustomEvent('abv-unauthorized'));
    throw new Error('unauthorized');
  }
  if (!res.ok) {
    throw new Error((await res.text().catch(() => '')) || `HTTP ${res.status}`);
  }
  const text = await res.text();
  if (!text) return undefined as T;
  return JSON.parse(text) as T;
}

export { isTauri, COMMAND_MAPPING, DESKTOP_ONLY, uploadRequest, request };
export default request;
