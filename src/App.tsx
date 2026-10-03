import { createBrowserRouter, Navigate, RouterProvider } from 'react-router-dom';

import Layout from './components/layout/Layout';
import Dashboard from './pages/Dashboard';
import Accounts from './pages/Accounts';
import Settings from './pages/Settings';
import MenuBarDashboard from './pages/MenuBarDashboard';
import ThemeManager from './components/common/ThemeManager';
import { AdminAuthGuard } from './components/common/AdminAuthGuard';
import { useEffect } from 'react';
import { useConfigStore } from './stores/useConfigStore';
import { useAccountStore } from './stores/useAccountStore';
import { useTranslation } from 'react-i18next';
import { subscribe } from './utils/events';

const router = createBrowserRouter([
  { path: "/menubar", element: <MenuBarDashboard /> },
  {
    path: '/',
    element: <Layout />,
    children: [
      {
        index: true,
        element: <Dashboard />,
      },
      {
        path: 'accounts',
        element: <Accounts />,
      },
      {
        path: 'settings',
        element: <Settings />,
      },
      {
        path: '*',
        element: <Navigate to="/" replace />,
      },
    ],
  },
]);

function App() {
  const { config, loadConfig } = useConfigStore();
  const { fetchCurrentAccount, fetchAccounts } = useAccountStore();
  const { i18n } = useTranslation();

  useEffect(() => {
    loadConfig();
  }, [loadConfig]);

  // Sync language from config
  useEffect(() => {
    if (config?.language) {
      const language = config.language.toLowerCase().startsWith('en') ? 'en' : 'zh';
      i18n.changeLanguage(language);
      document.documentElement.dir = 'ltr';
    }
  }, [config?.language, i18n]);

  // Listen for backend events (Tauri listen / Web SSE 双轨，见 utils/events.ts)
  useEffect(() => {
    const unlistenPromises: Promise<() => void>[] = [];

    unlistenPromises.push(subscribe('config://updated', () => { loadConfig(); }));
    if (window.location.pathname === '/menubar') {
      return () => { Promise.all(unlistenPromises).then(listeners => listeners.forEach(unlisten => unlisten())); };
    }
    unlistenPromises.push(subscribe<string>('app://navigate', payload => {
      if (['/', '/accounts', '/settings'].includes(payload)) router.navigate(payload);
    }));

    // 监听托盘切换账号事件
    unlistenPromises.push(
      subscribe('tray://account-switched', () => {
        console.log('[App] Tray account switched, refreshing...');
        fetchCurrentAccount();
        fetchAccounts();
      })
    );

    // 监听托盘刷新事件
    unlistenPromises.push(
      subscribe('tray://refresh-current', () => {
        console.log('[App] Tray refresh triggered, refreshing...');
        fetchCurrentAccount();
        fetchAccounts();
      })
    );

    // 监听后端全量刷新事件 (Command / Scheduler)
    unlistenPromises.push(
      subscribe('accounts://refreshed', () => {
        console.log('[App] Backend triggered quota refresh, syncing UI...');
        fetchCurrentAccount();
        fetchAccounts();
      })
    );

    // Cleanup
    return () => {
      Promise.all(unlistenPromises).then(unlisteners => {
        unlisteners.forEach(unlisten => unlisten());
      });
    };
  }, [fetchCurrentAccount, fetchAccounts, loadConfig]);

  return (
    <>
      <ThemeManager />
      <AdminAuthGuard>
        <RouterProvider router={router} />
      </AdminAuthGuard>
    </>
  );
}

export default App;
