const isTauri = typeof window !== 'undefined' &&
  (!!(window as any).__TAURI_INTERNALS__ || !!(window as any).__TAURI__);

export async function request<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri) {
    throw new Error(`Command [${cmd}] is only available in the desktop app.`);
  }

  try {
    const { invoke } = await import('@tauri-apps/api/core');
    return await invoke<T>(cmd, args);
  } catch (error) {
    console.error(`Tauri Invoke Error [${cmd}]:`, error);
    throw error;
  }
}
