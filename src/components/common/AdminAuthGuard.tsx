import React, { useState, useEffect } from 'react';
import { Lock, Key, Globe, AlertCircle, Loader2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { isTauri } from '../../utils/env';

/**
 * AdminAuthGuard
 * 针对 Web 服务模式的强制鉴权保护层。
 * 如果检测到没有存储的管理密码或后端返回 401，将拦截 UI 并要求输入密码。
 * 密码来源见 release/README.md（WEB_PASSWORD 环境变量或首启日志随机密码）。
 */
export const AdminAuthGuard: React.FC<{ children: React.ReactNode }> = ({ children }) => {
    const { t, i18n } = useTranslation();
    const [isAuthenticated, setIsAuthenticated] = useState(isTauri());
    const [password, setPassword] = useState('');
    const [showLangMenu, setShowLangMenu] = useState(false);
    const [isLoading, setIsLoading] = useState(false);
    const [error, setError] = useState('');

    useEffect(() => {
        if (isTauri()) return;

        // 检查 Session 存储 (优先)
        const sessionKey = sessionStorage.getItem('abv_admin_api_key');
        if (sessionKey) {
            setIsAuthenticated(true);
            setPassword(sessionKey);
            return;
        }

        // 检查本地存储 (迁移逻辑)
        const savedKey = localStorage.getItem('abv_admin_api_key');
        if (savedKey) {
            // 迁移到 sessionStorage 并清理 localStorage
            sessionStorage.setItem('abv_admin_api_key', savedKey);
            localStorage.removeItem('abv_admin_api_key');
            setIsAuthenticated(true);
            setPassword(savedKey);
        }

        // 监听全局 401 事件
        const handleUnauthorized = () => {
            sessionStorage.removeItem('abv_admin_api_key');
            localStorage.removeItem('abv_admin_api_key'); // 双重清理确保万一
            setIsAuthenticated(false);
        };

        window.addEventListener('abv-unauthorized', handleUnauthorized);
        return () => window.removeEventListener('abv-unauthorized', handleUnauthorized);
    }, []);

    const handleLogin = async (e: React.FormEvent) => {
        e.preventDefault();
        const trimmed = password.trim();
        if (!trimmed) return;

        setIsLoading(true);
        setError('');

        try {
            // 先临时存储密码，用于验证请求
            sessionStorage.setItem('abv_admin_api_key', trimmed);

            // 调用一个需要认证的 API 来验证密码是否正确
            const response = await fetch('/api/list_accounts', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                    'Authorization': `Bearer ${trimmed}`,
                    'x-api-key': trimmed
                },
                body: '{}'
            });

            if (response.ok || response.status === 204) {
                // 验证成功
                localStorage.removeItem('abv_admin_api_key');
                setIsAuthenticated(true);
                window.location.reload();
            } else if (response.status === 401) {
                // 密码错误
                sessionStorage.removeItem('abv_admin_api_key');
                setError(t('login.error_invalid_key'));
            } else {
                // 其他错误，但可能密码是对的
                setIsAuthenticated(true);
                window.location.reload();
            }
        } catch (err) {
            // 网络错误等
            sessionStorage.removeItem('abv_admin_api_key');
            setError(t('login.error_network'));
        } finally {
            setIsLoading(false);
        }
    };

    const changeLanguage = (lng: string) => {
        i18n.changeLanguage(lng);
        setShowLangMenu(false);
    };

    const languages = [
        { code: 'zh', name: '简体中文' },
        { code: 'en', name: 'English' },
    ];

    if (isAuthenticated) {
        return <>{children}</>;
    }

    return (
        <div className="min-h-screen bg-base-200 flex items-center justify-center p-4 relative">
            {/* 语言切换按钮 */}
            <div className="absolute top-8 right-8">
                <div className="relative">
                    <button
                        onClick={() => setShowLangMenu(!showLangMenu)}
                        className="flex items-center gap-2 px-4 py-2 bg-base-100 rounded-2xl shadow-sm border border-base-300 text-base-content/70 hover:bg-base-300/40 transition-all"
                    >
                        <Globe className="w-4 h-4" />
                        <span className="text-sm font-medium uppercase">{i18n.language.split('-')[0]}</span>
                    </button>

                    {showLangMenu && (
                        <div className="absolute right-0 mt-2 w-40 bg-base-100 rounded-2xl shadow-xl border border-base-300 py-2 z-50">
                            {languages.map((lang) => (
                                <button
                                    key={lang.code}
                                    onClick={() => changeLanguage(lang.code)}
                                    className={`w-full text-left px-4 py-2 text-sm hover:bg-base-200 transition-colors ${i18n.language === lang.code ? 'text-primary font-bold' : 'text-base-content/70'
                                        }`}
                                >
                                    {lang.name}
                                </button>
                            ))}
                        </div>
                    )}
                </div>
            </div>

            <div className="max-w-md w-full bg-base-100 rounded-3xl shadow-xl overflow-hidden border border-base-300">
                <div className="p-8">
                    <div className="w-16 h-16 bg-primary/10 rounded-2xl flex items-center justify-center mb-6 mx-auto">
                        <Lock className="w-8 h-8 text-primary" />
                    </div>
                    <h2 className="text-2xl font-bold text-center text-base-content mb-2">{t('login.title')}</h2>
                    <p className="text-center text-base-content/60 mb-8 text-sm">{t('login.desc')}</p>

                    <form onSubmit={handleLogin} className="space-y-6">
                        <div className="relative">
                            <Key className="absolute left-4 top-1/2 -translate-y-1/2 w-5 h-5 text-base-content/40" />
                            <input
                                type="password"
                                placeholder={t('login.placeholder')}
                                className={`w-full pl-12 pr-4 py-4 bg-base-200 border-2 rounded-2xl focus:ring-2 focus:ring-primary transition-all outline-none text-base-content ${error ? 'border-error' : 'border-transparent'}`}
                                value={password}
                                onChange={(e) => { setPassword(e.target.value); setError(''); }}
                                autoFocus
                                disabled={isLoading}
                            />
                        </div>
                        {error && (
                            <div className="flex items-center gap-2 text-error text-sm">
                                <AlertCircle className="w-4 h-4" />
                                <span>{error}</span>
                            </div>
                        )}
                        <button
                            type="submit"
                            disabled={isLoading || !password.trim()}
                            className="w-full py-4 bg-primary hover:bg-primary/90 disabled:bg-primary/40 disabled:cursor-not-allowed text-primary-content font-bold rounded-2xl shadow-lg transition-all active:scale-[0.98] flex items-center justify-center gap-2"
                        >
                            {isLoading ? (
                                <>
                                    <Loader2 className="w-5 h-5 animate-spin" />
                                    {t('login.btn_verifying')}
                                </>
                            ) : (
                                t('login.btn_login')
                            )}
                        </button>
                    </form>

                    <div className="mt-8 pt-6 border-t border-base-300 text-center">
                        <p className="text-[10px] text-base-content/40 leading-relaxed">
                            {t('login.note')}
                            <br />
                            {t('login.lookup_hint')}
                        </p>
                    </div>
                </div>
            </div>
        </div>
    );
};
