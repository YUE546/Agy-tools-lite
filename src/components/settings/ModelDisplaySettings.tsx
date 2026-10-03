import { useMemo, useState } from 'react';
import { Bot, BrainCircuit, Check, Layers, RotateCcw, Sparkles } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useConfigStore } from '../../stores/useConfigStore';
import { useAccountStore } from '../../stores/useAccountStore';
import { DEFAULT_PINNED_MODELS, MODEL_CONFIG } from '../../config/modelConfig';
import { getModelDisplayName } from '../../utils/modelCategory';
import { showToast } from '../common/ToastContainer';

interface ModelOption {
    id: string;
    label: string;
    group: 'gemini-3' | 'claude' | 'gemini-2.5' | 'other' | 'dynamic';
    iconType: 'gemini' | 'claude' | 'bot';
    tag?: string;
}

const KNOWN_MODELS: ModelOption[] = [
    // Gemini 3 系列
    { id: 'gemini-3.1-pro-high', label: 'Gemini 3.1 Pro (High)', group: 'gemini-3', iconType: 'gemini', tag: 'PRO' },
    { id: 'gemini-3.8-flash-high', label: 'Gemini 3.8 Flash (High)', group: 'gemini-3', iconType: 'gemini', tag: 'FLASH' },
    { id: 'gemini-3.1-flash-image', label: 'Gemini 3.1 Flash Image', group: 'gemini-3', iconType: 'gemini', tag: 'IMAGE' },
    { id: 'gemini-3.1-flash-lite', label: 'Gemini 3.1 Flash Lite', group: 'gemini-3', iconType: 'gemini', tag: 'LITE' },
    { id: 'gemini-3-flash', label: 'Gemini 3 Flash', group: 'gemini-3', iconType: 'gemini', tag: 'FLASH' },

    // Claude 系列
    { id: 'claude-sonnet-4-6', label: 'Claude Sonnet 4.6 (Thinking)', group: 'claude', iconType: 'claude', tag: 'SONNET' },
    { id: 'claude-opus-4-6-thinking', label: 'Claude Opus 4.6 (Thinking)', group: 'claude', iconType: 'claude', tag: 'OPUS' },

    // Gemini 2.5 系列
    { id: 'gemini-2.5-pro', label: 'Gemini 2.5 Pro', group: 'gemini-2.5', iconType: 'gemini', tag: 'PRO' },
    { id: 'gemini-2.5-flash', label: 'Gemini 2.5 Flash', group: 'gemini-2.5', iconType: 'gemini', tag: 'FLASH' },
    { id: 'gemini-2.5-flash-lite', label: 'Gemini 2.5 Flash Lite', group: 'gemini-2.5', iconType: 'gemini', tag: 'LITE' },

    // 其他
    { id: 'gpt-oss-120b-medium', label: 'GPT-OSS 120B (Medium)', group: 'other', iconType: 'bot', tag: 'OPENAI' },
];

interface ModelDisplaySettingsProps {
    onClose?: () => void;
    embedded?: boolean;
}

export default function ModelDisplaySettings({ onClose, embedded = false }: ModelDisplaySettingsProps) {
    const { t } = useTranslation();
    const { config, saveConfig, showAllQuotas, toggleShowAllQuotas } = useConfigStore();
    const accounts = useAccountStore(state => state.accounts);
    const [saving, setSaving] = useState(false);

    // Current pinned list
    const currentPinned = useMemo(() => {
        const configured = config?.pinned_quota_models?.models;
        if (configured && configured.length > 0) return configured;
        return DEFAULT_PINNED_MODELS;
    }, [config?.pinned_quota_models?.models]);

    // Discover any additional models from existing accounts
    const allModels = useMemo(() => {
        const knownIds = new Set(KNOWN_MODELS.map(m => m.id.toLowerCase()));
        const dynamicModels: ModelOption[] = [];

        for (const account of accounts) {
            for (const m of account.quota?.models || []) {
                const norm = m.name.toLowerCase();
                if (!knownIds.has(norm)) {
                    knownIds.add(norm);
                    const cfg = MODEL_CONFIG[norm];
                    const label = m.display_name || cfg?.label || getModelDisplayName(m) || m.name;
                    const isClaude = norm.includes('claude');
                    const isGemini = norm.includes('gemini');
                    dynamicModels.push({
                        id: m.name,
                        label,
                        group: 'dynamic',
                        iconType: isClaude ? 'claude' : isGemini ? 'gemini' : 'bot',
                        tag: isClaude ? 'CLAUDE' : isGemini ? 'GEMINI' : 'OTHER',
                    });
                }
            }
        }

        return [...KNOWN_MODELS, ...dynamicModels];
    }, [accounts]);

    const handleToggleModel = async (modelId: string) => {
        if (!config) return;
        const exists = currentPinned.includes(modelId);
        let next: string[];
        if (exists) {
            next = currentPinned.filter(id => id !== modelId);
            if (next.length === 0) {
                showToast(t('model_display.empty_tip', '请至少保留一个模型'), 'warning');
                return;
            }
        } else {
            next = [...currentPinned, modelId];
        }

        try {
            setSaving(true);
            await saveConfig({
                ...config,
                pinned_quota_models: { models: next }
            }, true);
            showToast(t('model_display.saved', '已更新卡片展示模型'), 'success');
        } catch (err) {
            showToast(String(err), 'error');
        } finally {
            setSaving(false);
        }
    };

    const handleApplyPreset = async (preset: string[]) => {
        if (!config) return;
        try {
            setSaving(true);
            if (showAllQuotas) {
                toggleShowAllQuotas();
            }
            await saveConfig({
                ...config,
                pinned_quota_models: { models: preset }
            }, true);
            showToast(t('model_display.saved', '已更新卡片展示模型'), 'success');
        } catch (err) {
            showToast(String(err), 'error');
        } finally {
            setSaving(false);
        }
    };

    const handleSetShowAll = (showAll: boolean) => {
        if (showAllQuotas !== showAll) {
            toggleShowAllQuotas();
        }
    };

    const renderIcon = (type: ModelOption['iconType']) => {
        switch (type) {
            case 'claude':
                return <BrainCircuit className="w-4 h-4 text-purple-600 dark:text-purple-400" />;
            case 'gemini':
                return <Sparkles className="w-4 h-4 text-amber-500 dark:text-amber-400" />;
            case 'bot':
            default:
                return <Bot className="w-4 h-4 text-blue-500 dark:text-blue-400" />;
        }
    };

    const groups: { key: ModelOption['group']; title: string }[] = [
        { key: 'gemini-3', title: t('model_display.group_gemini_3', 'Gemini 3 系列') },
        { key: 'claude', title: t('model_display.group_claude', 'Claude 系列') },
        { key: 'gemini-2.5', title: t('model_display.group_gemini_25', 'Gemini 2.5 系列') },
        { key: 'other', title: t('model_display.group_other', '其他系列') },
        { key: 'dynamic', title: t('model_display.group_dynamic', '账号中检测到的其他可用模型') },
    ];

    return (
        <div className="space-y-5">
            {/* 模式选择：精选固定 vs 展示全部 */}
            <section className="rounded-xl border border-gray-200/80 bg-white p-5 shadow-sm dark:border-slate-800 dark:bg-slate-900/60">
                <div className="mb-2 text-xs font-semibold uppercase tracking-wider text-gray-500 dark:text-gray-400">
                    {t('model_display.mode', '展示模式')}
                </div>
                <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                    <button
                        type="button"
                        onClick={() => handleSetShowAll(false)}
                        className={`flex items-start gap-3 p-3.5 rounded-xl border text-left transition-all ${!showAllQuotas
                            ? 'border-blue-500 bg-blue-50/70 dark:bg-blue-500/15 dark:border-blue-500 ring-1 ring-blue-500/20'
                            : 'border-gray-200 dark:border-slate-800 hover:bg-gray-50 dark:hover:bg-slate-800/40'
                            }`}
                    >
                        <span className={`mt-0.5 w-4 h-4 rounded-full border flex items-center justify-center shrink-0 ${!showAllQuotas ? 'border-blue-600 bg-blue-600' : 'border-gray-300 dark:border-slate-600'}`}>
                            {!showAllQuotas && <span className="w-1.5 h-1.5 rounded-full bg-white" />}
                        </span>
                        <div>
                            <div className="text-sm font-semibold text-gray-800 dark:text-gray-100">
                                {t('model_display.mode_pinned', '精选固定模型 (推荐)')}
                            </div>
                            <div className="text-xs text-gray-500 dark:text-gray-400 mt-0.5 leading-relaxed">
                                {t('model_display.mode_pinned_desc', '仅展示下方勾选的常用核心模型，保持卡片与列表清爽')}
                            </div>
                        </div>
                    </button>

                    <button
                        type="button"
                        onClick={() => handleSetShowAll(true)}
                        className={`flex items-start gap-3 p-3.5 rounded-xl border text-left transition-all ${showAllQuotas
                            ? 'border-blue-500 bg-blue-50/70 dark:bg-blue-500/15 dark:border-blue-500 ring-1 ring-blue-500/20'
                            : 'border-gray-200 dark:border-slate-800 hover:bg-gray-50 dark:hover:bg-slate-800/40'
                            }`}
                    >
                        <span className={`mt-0.5 w-4 h-4 rounded-full border flex items-center justify-center shrink-0 ${showAllQuotas ? 'border-blue-600 bg-blue-600' : 'border-gray-300 dark:border-slate-600'}`}>
                            {showAllQuotas && <span className="w-1.5 h-1.5 rounded-full bg-white" />}
                        </span>
                        <div>
                            <div className="text-sm font-semibold text-gray-800 dark:text-gray-100 flex items-center gap-1.5">
                                <Layers className="w-3.5 h-3.5 text-purple-500" />
                                {t('model_display.mode_all', '展示全部可用配额模型')}
                            </div>
                            <div className="text-xs text-gray-500 dark:text-gray-400 mt-0.5 leading-relaxed">
                                {t('model_display.mode_all_desc', '平铺展示当前账号所拥有的所有可用模型配额')}
                            </div>
                        </div>
                    </button>
                </div>
            </section>

            {/* 精选模型勾选区 (在精选模式下高亮，在全部模式下可预先配置) */}
            <section className={`rounded-xl border border-gray-200/80 bg-white p-5 shadow-sm dark:border-slate-800 dark:bg-slate-900/60 transition-opacity ${showAllQuotas ? 'opacity-60' : 'opacity-100'}`}>
                {/* 顶部工具条：已选数量 + 预设快捷按钮 */}
                <div className="flex flex-wrap items-center justify-between gap-3 mb-4 pb-3 border-b border-gray-100 dark:border-slate-800">
                    <div className="flex items-center gap-2">
                        <span className="text-sm font-semibold text-gray-800 dark:text-gray-100">
                            {t('model_display.selected_models', '自定义展示模型')}
                        </span>
                        <span className="px-2 py-0.5 rounded-full text-xs font-bold bg-blue-100 dark:bg-blue-900/40 text-blue-600 dark:text-blue-300">
                            {t('model_display.selected_count', { count: currentPinned.length })}
                        </span>
                    </div>

                    <div className="flex items-center gap-1.5 flex-wrap">
                        <button
                            type="button"
                            disabled={saving}
                            onClick={() => handleApplyPreset(DEFAULT_PINNED_MODELS)}
                            className="px-2.5 py-1 text-xs font-medium rounded-lg border border-gray-200 dark:border-slate-700 bg-gray-50 dark:bg-slate-800 text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-slate-700 transition-colors flex items-center gap-1"
                        >
                            <RotateCcw className="w-3 h-3 text-blue-500" />
                            {t('model_display.preset_recommended', '⚡ 核心推荐')}
                        </button>
                        <button
                            type="button"
                            disabled={saving}
                            onClick={() => handleApplyPreset(['gemini-3.1-pro-high', 'gemini-3.8-flash-high', 'gemini-3.1-flash-image', 'claude-sonnet-4-6'])}
                            className="px-2.5 py-1 text-xs font-medium rounded-lg border border-gray-200 dark:border-slate-700 bg-gray-50 dark:bg-slate-800 text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-slate-700 transition-colors"
                        >
                            {t('model_display.preset_all_gemini', '常用组合 (含绘图)')}
                        </button>
                    </div>
                </div>

                {/* 分组列表 */}
                <div className="space-y-4">
                    {groups.map(group => {
                        const groupModels = allModels.filter(m => m.group === group.key);
                        if (groupModels.length === 0) return null;

                        return (
                            <div key={group.key} className="space-y-2">
                                <div className="text-[11px] font-semibold text-gray-400 dark:text-gray-500 uppercase tracking-wider">
                                    {group.title}
                                </div>
                                <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
                                    {groupModels.map(model => {
                                        const isChecked = currentPinned.includes(model.id);
                                        return (
                                            <div
                                                key={model.id}
                                                onClick={() => handleToggleModel(model.id)}
                                                className={`flex items-center justify-between p-3 rounded-xl border cursor-pointer select-none transition-all ${isChecked
                                                    ? 'border-blue-500/80 bg-blue-50/50 dark:bg-blue-500/10 dark:border-blue-500/60 shadow-xs'
                                                    : 'border-gray-200/80 dark:border-slate-800/80 bg-gray-50/40 dark:bg-slate-800/30 hover:bg-gray-100/60 dark:hover:bg-slate-800/60'
                                                    }`}
                                            >
                                                <div className="flex items-center gap-2.5 min-w-0">
                                                    <span className="shrink-0">{renderIcon(model.iconType)}</span>
                                                    <div className="min-w-0">
                                                        <div className="text-xs font-semibold text-gray-800 dark:text-gray-200 truncate" title={model.label}>
                                                            {model.label}
                                                        </div>
                                                        <div className="text-[10px] font-mono text-gray-400 dark:text-gray-500 truncate" title={model.id}>
                                                            {model.id}
                                                        </div>
                                                    </div>
                                                </div>

                                                <div className="flex items-center gap-2 shrink-0 ml-2">
                                                    {model.tag && (
                                                        <span className="text-[9px] font-bold px-1.5 py-0.5 rounded bg-gray-200/70 dark:bg-slate-700 text-gray-600 dark:text-gray-300">
                                                            {model.tag}
                                                        </span>
                                                    )}
                                                    <span className={`w-5 h-5 rounded-md border flex items-center justify-center transition-colors ${isChecked
                                                        ? 'bg-blue-600 border-blue-600 text-white'
                                                        : 'border-gray-300 dark:border-slate-600 bg-white dark:bg-slate-800'
                                                        }`}>
                                                        {isChecked && <Check className="w-3.5 h-3.5 stroke-[2.5]" />}
                                                    </span>
                                                </div>
                                            </div>
                                        );
                                    })}
                                </div>
                            </div>
                        );
                    })}
                </div>

                <div className="mt-4 pt-3 border-t border-gray-100 dark:border-slate-800 text-[11px] text-gray-500 dark:text-gray-400 leading-relaxed">
                    💡 {t('model_display.hint', '勾选或取消勾选后实时保存生效。账号卡片与表格将按照你的配置展示对应的配额进度与重置倒计时。')}
                </div>
            </section>

            {/* 关闭按钮 (如果不是内嵌页面) */}
            {!embedded && onClose && (
                <div className="flex justify-end pt-2">
                    <button
                        type="button"
                        onClick={onClose}
                        className="px-5 py-2 rounded-xl bg-blue-600 text-white font-medium text-xs hover:bg-blue-700 active:scale-95 transition-all shadow-sm"
                    >
                        {t('common.confirm', '完成')}
                    </button>
                </div>
            )}
        </div>
    );
}
