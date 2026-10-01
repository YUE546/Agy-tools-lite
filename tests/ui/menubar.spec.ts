import { expect, test, type Page } from "@playwright/test";

// Browser-only synthetic IPC acceptance. These screenshots do not verify native
// macOS vibrancy, Windows glass, login items, account credentials or OS processes.
test.beforeEach(async ({ page }) => {
  await page.setViewportSize({ width: 380, height: 480 });
  await page.addInitScript(() => {
    const w = window as any;
    const callbacks: Record<number, Function> = {};
    const listeners: Record<number, any> = {};
    let language = "zh";
    let theme = "dark";
    let callback = 1;
    let current = "A";
    let holdSwitch = false;
    let releaseSwitch: (() => void) | null = null;
    const now = Math.floor(Date.now() / 1000);
    const reset = (window: string) =>
      new Date(Date.now() + (window === "5h" ? 2 * 3600000 : 3 * 86400000)).toISOString();
    const account = (id: string, label: string, gemini = 80, claude = 70) => ({
      id,
      email: `${id.toLowerCase()}@example.invalid`,
      custom_label: label,
      created_at: now,
      last_used: now,
      token: {
        access_token: "synthetic-only",
        refresh_token: "synthetic-only",
        expires_in: 3600,
        expiry_timestamp: now + 3600,
        token_type: "Bearer",
      },
      quota: {
        last_updated: now,
        subscription_tier: "PRO",
        models: [],
        quota_groups: [
          {
            display_name: "Gemini Models",
            buckets: ["5h", "weekly"].map((window) => ({
              bucket_id: `g-${window}`,
              window,
              remaining_fraction: gemini / 100,
              reset_time: reset(window),
            })),
          },
          {
            display_name: "Claude / GPT",
            buckets: ["5h", "weekly"].map((window) => ({
              bucket_id: `c-${window}`,
              window,
              remaining_fraction: claude / 100,
              reset_time: reset(window),
            })),
          },
        ],
      },
    });
    let accounts: any[] = [
      account("A", "工作账号", 80, 6),
      account("B", "个人账号"),
      { ...account("C", "已失效账号"), disabled: true },
      account("D", "研究账号", 8, 60),
      { id: "E", email: "missing@example.invalid", custom_label: "未刷新账号" },
      account("F", "过期数据"),
      { ...account("G", "待验证账号"), validation_blocked: true },
    ];
    accounts[5].quota.last_updated -= 3600;
    let status = {
      phase: "disabled",
      reason: null,
      source_account_id: "A",
      source_email: "a@example.invalid",
      target_account_id: "B",
      target_email: "b@example.invalid",
      remaining_percentage: 6,
      pending_id: null,
      mode: "wait",
      process_state: "running",
      last_checked: now,
    } as any;
    const calls: { cmd: string; args: any }[] = [];
    let failRefresh = false;
    let failStatus = false;
    const emit = (event: string, payload: any = {}) =>
      Object.entries(listeners).forEach(([id, listener]) => {
        if (listener.event === event)
          callbacks[listener.handler]?.({ id: Number(id), event, payload });
      });
    w.__menuFixture = {
      calls,
      emit,
      setPreferences: (next: any) => {
        language = next.language || language;
        theme = next.theme || theme;
        emit("config://updated");
      },
      current: () => current,
      setStatus: (patch: any) => {
        status = { ...status, ...patch };
        emit("menubar://opened");
      },
      holdSwitch: () => {
        holdSwitch = true;
      },
      releaseSwitch: () => releaseSwitch?.(),
      empty: () => {
        accounts = [];
        emit("menubar://data-updated");
      },
      failRefresh: () => {
        failRefresh = true;
      },
      failStatus: () => {
        failStatus = true;
        emit("menubar://opened");
      },
    };
    w.__TAURI_INTERNALS__ = {
      transformCallback: (fn: Function) => {
        const id = callback++;
        callbacks[id] = fn;
        return id;
      },
      unregisterCallback: (id: number) => {
        delete callbacks[id];
      },
      convertFileSrc: (s: string) => s,
      invoke: async (cmd: string, args: any = {}) => {
        calls.push({ cmd, args });
        if (cmd === "load_config")
          return {
            language,
            theme,
            auto_refresh: false,
            auto_sync: false,
            refresh_interval: 15,
            sync_interval: 5,
            pinned_quota_models: { models: [] },
            quota_protection: {
              enabled: false,
              threshold_percentage: 10,
              monitored_models: [],
            },
          };
        if (cmd === "list_accounts") return accounts;
        if (cmd === "get_current_account")
          return accounts.find((a) => a.id === current) || null;
        if (cmd === "get_local_token_usage")
          return { today: { total_tokens: 148200, request_count: 86 } };
        if (cmd === "get_menu_bar_appearance")
          return {
            platform: "linux",
            native_material: false,
            reduced_transparency: false,
            high_contrast: false,
          };
        if (cmd === "get_auto_switch_config")
          return {
            enabled: false,
            mode: "wait",
            reserve_percentage: 10,
            candidate_min_percentage: 30,
            monitored_model: "gemini-test",
            candidate_account_ids: ["B"],
            target: "app",
          };
        if (cmd === "get_auto_switch_status") {
          if (failStatus) throw new Error("Status unavailable");
          return status;
        }
        if (cmd === "cancel_auto_switch") {
          if (args.pendingId !== status.pending_id)
            throw new Error("Stale cancellation");
          status = {
            ...status,
            phase: "canceled",
            reason: "canceled_until_recovery",
            pending_id: null,
          };
          return status;
        }
        if (cmd === "check_auto_switch_now") return status;
        if (cmd === "switch_account") {
          if (holdSwitch)
            await new Promise<void>((resolve) => {
              releaseSwitch = resolve;
            });
          current = args.accountId;
          emit("tray://account-switched");
          return;
        }
        if (cmd === "refresh_all_quotas")
          return {
            total: accounts.length,
            success: failRefresh ? 0 : accounts.length,
            failed: failRefresh ? accounts.length : 0,
            details: [],
          };
        if (cmd === "fetch_account_quota") {
          if (failRefresh) throw new Error("Quota unavailable");
          return accounts.find((a) => a.id === args.accountId)?.quota;
        }
        if (cmd === "plugin:event|listen") {
          const id = callback++;
          listeners[id] = args;
          return id;
        }
        if (cmd === "plugin:event|unlisten") {
          delete listeners[args.eventId];
          return;
        }
        return null;
      },
      metadata: {
        currentWindow: { label: "menubar" },
        currentWebview: { label: "menubar" },
      },
    };
    w.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    localStorage.setItem("i18nextLng", "zh");
  });
  await page.goto("/menubar");
  await expect(
    page.getByText("总览 · 7 个账号", { exact: true }),
  ).toBeVisible();
});

async function choose(page: Page, label: string) {
  await page.getByRole("button", { name: "选择查看范围" }).click();
  await page.locator(".mb-native-option").filter({ hasText: label }).click();
  await expect(page.locator(".mb-native-account")).toContainText(label);
}
async function bounded(page: Page) {
  const overflow = await page.evaluate(() =>
    [
      document.documentElement,
      document.body,
      document.getElementById("root"),
      ...document.querySelectorAll(
        ".menubar-app,.mb-native-main,.mb-native-picker,.mb-native-quotas,.mb-switch-details",
      ),
    ]
      .filter(Boolean)
      .filter(
        (el: any) =>
          el.scrollHeight > el.clientHeight + 1 ||
          el.scrollWidth > el.clientWidth + 1,
      )
      .map((el: any) => ({
        tag: el.tagName,
        class: el.className,
        height: [el.clientHeight, el.scrollHeight],
        width: [el.clientWidth, el.scrollWidth],
      })),
  );
  expect(overflow).toEqual([]);
  expect(
    await page
      .locator(".mb-native-footer")
      .evaluate((el) => el.getBoundingClientRect().bottom <= innerHeight + 1),
  ).toBe(true);
}

test("overview and account inspection stay on one screen and selection never activates an account", async ({
  page,
}, info) => {
  await expect(page.locator(".mb-overview-counts strong")).toHaveText([
    "1",
    "2",
    "2",
    "2",
  ]);
  await expect(page.getByText("未按账号拆分", { exact: true })).toBeVisible();
  await bounded(page);
  await page.screenshot({
    path: info.outputPath("menubar-overview-linux-browser-dark.png"),
  });
  await choose(page, "个人账号");
  await expect(page.getByText(/仅查看，未切换/)).toBeVisible();
  await expect(page.getByText("未按账号拆分", { exact: true })).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: "切换为此账号", exact: true }),
  ).toBeEnabled();
  expect(
    await page.evaluate(() =>
      (window as any).__menuFixture.calls.filter(
        (c: any) => c.cmd === "switch_account",
      ),
    ),
  ).toEqual([]);
  await bounded(page);
  await page.screenshot({
    path: info.outputPath("menubar-account-linux-browser-dark.png"),
  });
  await page.getByRole("button", { name: "切换为此账号", exact: true }).click();
  await expect(
    page.getByText("已切换到 个人账号", { exact: true }),
  ).toBeVisible();
  expect(
    await page.evaluate(() =>
      (window as any).__menuFixture.calls
        .filter((c: any) => c.cmd === "switch_account")
        .map((c: any) => c.args.accountId),
    ),
  ).toEqual(["B"]);
});

test("account pagination, blocked inspection and empty state do not scroll", async ({
  page,
}, info) => {
  await page.getByRole("button", { name: "选择查看范围" }).click();
  await expect(page.locator(".mb-native-option")).toHaveCount(4);
  await page.getByRole("button", { name: "下一页", exact: true }).click();
  await expect(page.locator(".mb-native-option")).toHaveCount(3);
  await bounded(page);
  await page.screenshot({
    path: info.outputPath("menubar-account-picker-linux-browser.png"),
  });
  await page
    .locator(".mb-native-option")
    .filter({ hasText: "待验证账号" })
    .click();
  await expect(
    page.getByRole("button", { name: "切换为此账号", exact: true }),
  ).toBeDisabled();
  await bounded(page);
  await page.evaluate(() => (window as any).__menuFixture.empty());
  await expect(
    page.getByText("总览 · 0 个账号", { exact: true }),
  ).toBeVisible();
  await bounded(page);
});

test("switch completion preserves a newer inspected account and repeated activation is blocked", async ({
  page,
}) => {
  await choose(page, "个人账号");
  await page.evaluate(() => (window as any).__menuFixture.holdSwitch());
  await page.getByRole("button", { name: "切换为此账号", exact: true }).click();
  await expect(page.locator(".mb-native-switch")).toBeDisabled();
  await expect
    .poll(() =>
      page.evaluate(() =>
        (window as any).__menuFixture.calls.filter(
          (c: any) => c.cmd === "switch_account",
        ).length,
      ),
    )
    .toBe(1);
  await choose(page, "研究账号");
  await page.evaluate(() => (window as any).__menuFixture.releaseSwitch());
  await expect(
    page.getByText("已切换到 个人账号", { exact: true }),
  ).toBeVisible();
  await expect(page.locator(".mb-native-account")).toContainText("研究账号");
  expect(
    await page.evaluate(() => (window as any).__menuFixture.current()),
  ).toBe("B");
  expect(
    await page.evaluate(
      () =>
        (window as any).__menuFixture.calls.filter(
          (c: any) => c.cmd === "switch_account",
        ).length,
    ),
  ).toBe(1);
});

test("menu route shows coordinator status, truthful wait instructions and cancellation", async ({
  page,
}, info) => {
  await page.evaluate(() =>
    (window as any).__menuFixture.setStatus({
      phase: "pending",
      reason: "clients_running",
      pending_id: "fixture-request",
    }),
  );
  await page.getByRole("button", { name: "查看低额度换号详情" }).click();
  await expect(
    page.getByText("客户端仍未退出，等待安全换号", { exact: true }).last(),
  ).toBeVisible();
  await expect(
    page.getByText(/等待任务完成，再正常退出 Antigravity 和 agy/),
  ).toBeVisible();
  await bounded(page);
  await page.screenshot({
    path: info.outputPath("menubar-low-quota-wait-linux-browser.png"),
  });
  await page.getByRole("button", { name: "取消本次换号", exact: true }).click();
  await expect(page.getByText(/已取消本次换号/).last()).toBeVisible();
  expect(
    await page.evaluate(() =>
      (window as any).__menuFixture.calls
        .filter((c: any) => c.cmd === "cancel_auto_switch")
        .map((c: any) => c.args.pendingId),
    ),
  ).toEqual(["fixture-request"]);
  expect(
    await page.evaluate(() =>
      (window as any).__menuFixture.calls.some((c: any) =>
        /switch_account|stop|kill/.test(c.cmd),
      ),
    ),
  ).toBe(false);
});

test("switching prevents manual activation; completion and read failures stay truthful", async ({
  page,
}, info) => {
  await choose(page, "个人账号");
  await page.evaluate(() =>
    (window as any).__menuFixture.setStatus({
      phase: "switching",
      reason: "checking",
      pending_id: "fixture-request",
    }),
  );
  await expect(
    page.getByRole("button", { name: "切换为此账号", exact: true }),
  ).toBeDisabled();
  await page.getByRole("button", { name: "查看低额度换号详情" }).click();
  await expect(
    page.getByRole("button", { name: "取消本次换号", exact: true }),
  ).toBeDisabled();
  await expect(
    page.getByRole("button", { name: "检查状态", exact: true }),
  ).toBeDisabled();
  await page.evaluate(() =>
    (window as any).__menuFixture.setStatus({
      phase: "completed",
      reason: "credentials_updated",
      pending_id: null,
      process_state: "closed",
    }),
  );
  await expect(
    page.getByText("账号已准备，下次打开生效", { exact: true }).last(),
  ).toBeVisible();
  await expect(
    page.getByText(/不会自动重启、恢复会话或重跑工具/),
  ).toBeVisible();
  await bounded(page);
  await page.screenshot({
    path: info.outputPath("menubar-low-quota-completed-linux-browser.png"),
  });
  await page.evaluate(() => (window as any).__menuFixture.failStatus());
  await expect(
    page
      .getByText("无法读取换号状态，请打开设置检查。", { exact: true })
      .last(),
  ).toBeVisible();
});

test("refresh failure retains known data, and Escape dismisses instead of reopening", async ({
  page,
}) => {
  await page.evaluate(() => (window as any).__menuFixture.failRefresh());
  await page.getByRole("button", { name: "刷新全部账号" }).click();
  await expect(page.getByText(/7 个账号刷新失败/)).toBeVisible();
  await expect(page.locator(".mb-overview-counts strong")).toHaveText([
    "1",
    "2",
    "2",
    "2",
  ]);
  await bounded(page);
  await page.keyboard.press("Escape");
  // The key handler awaits request()'s dynamic import before native IPC runs.
  // Wait for the observable dismissal rather than racing that microtask.
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (window as any).__menuFixture.calls.filter(
            (c: any) => c.cmd === "hide_menu_bar_dashboard",
          ).length,
      ),
    )
    .toBe(1);
  expect(
    await page.evaluate(() =>
      (window as any).__menuFixture.calls.some(
        (c: any) => c.cmd === "show_main_window",
      ),
    ),
  ).toBe(false);
});

test("English and light fallback remain bounded, including the stop-first detail view", async ({
  page,
}, info) => {
  await page.evaluate(() =>
    (window as any).__menuFixture.setPreferences({
      language: "en",
      theme: "light",
    }),
  );
  await expect(
    page.getByText("Overview · 7 accounts", { exact: true }),
  ).toBeVisible();
  await bounded(page);
  await page.screenshot({
    path: info.outputPath("menubar-overview-linux-browser-light-en.png"),
  });
  await page.evaluate(() =>
    (window as any).__menuFixture.setStatus({
      phase: "pending",
      reason: "clients_running",
      mode: "stop",
      pending_id: "fixture-request",
    }),
  );
  await page
    .getByRole("button", { name: "View low-quota switching details" })
    .click();
  await expect(
    page.getByText(/Stop the current task in Antigravity/),
  ).toBeVisible();
  await bounded(page);
  await page.screenshot({
    path: info.outputPath("menubar-stop-first-linux-browser-en.png"),
  });
  await page
    .getByRole("button", { name: "Show stop instructions", exact: true })
    .click();
  await expect
    .poll(() =>
      page.evaluate(() =>
        (window as any).__menuFixture.calls
          .filter((c: any) => c.cmd === "open_app_page")
          .map((c: any) => c.args.page),
      ),
    )
    .toEqual(["settings"]);
  expect(
    await page.evaluate(() =>
      (window as any).__menuFixture.calls.some((c: any) =>
        /stop|kill|switch_account/.test(c.cmd),
      ),
    ),
  ).toBe(false);
});
