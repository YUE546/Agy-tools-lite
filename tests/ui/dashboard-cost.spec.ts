import { expect, test } from '@playwright/test';
import { setupSettingsFixture } from './settings-fixture';
for (const language of ['zh', 'en']) test(`cost breakdown uses exact prices and localized unknown states (${language})`, async ({ page }) => {
    const override = () => {
        const w = window as any, original = w.__TAURI_INTERNALS__.invoke;
        const totals = { input_tokens: 1000000, output_tokens: 0, cached_tokens: 0, total_tokens: 1000000, request_count: 1 };
        const models = ['gemini-2.5-flash', 'gemini-2.5-flash-lite', 'claude-opus-4.6'].map(model => ({ ...totals, model }));
        w.__TAURI_INTERNALS__.invoke = async (command: string, args: any) => {
            if (command === 'get_api_pricing') return { prices: [{ model: 'gemini-2.5-flash', input: .3, output: 2.5, cached: .03 }], fetched_at: 1, stale: false, source: 'fixture' };
            if (command === 'get_local_token_usage') return { today: totals, yesterday: totals, last_3_days: totals, last_7_days: totals, last_30_days: totals, by_model_today: models, by_model_yesterday: models, by_model_3_days: models, by_model_7_days: models, by_model: models, daily: [], hourly: [], unreadable_databases: 0, generated_at: 1 };
            return original(command, args);
        };
    };
    await page.addInitScript({ content: `(${setupSettingsFixture.toString()})(${JSON.stringify({ language })});(${override.toString()})();` });
    await page.goto('/');
    const table = page.getByRole('table');
    await expect(table.getByRole('row').filter({ hasText: 'gemini-2.5-flash-lite' })).toContainText(language === 'zh' ? '未计价' : 'Unpriced');
    await expect(table.getByRole('row').filter({ hasText: 'claude-opus' })).toContainText(language === 'zh' ? '未计价' : 'Unpriced');
    await expect(table.getByRole('row').filter({ hasText: 'gemini-2.5-flash' }).first()).toContainText('$0.30');
    await page.getByRole('button', { name: language === 'zh' ? '费用饼图' : 'Cost' }).click();
    await expect(page.getByText(language === 'zh' ? '已计价小计' : 'Priced subtotal', { exact: true })).toBeVisible();
    if (language === 'en') expect(await page.locator('main').innerText()).not.toMatch(/[\u3400-\u9fff]/);
});
