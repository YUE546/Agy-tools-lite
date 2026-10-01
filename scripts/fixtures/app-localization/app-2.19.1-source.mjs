// Source-derived fixture. No App is started and no network/port is used.
// Official 2.19.1-6046815158665216 main bundle SHA-256:
// 47f36abaabd34f7d54a95df40d942b609c02db9f5c04d56b124a048571b9f16d
// Settings button was separately verified on the real macOS 2.19.1 App.
// Navigation and tail controls remain candidates until real-App QA succeeds.
import { createFixtureHost } from './fake-dom.mjs';

export const candidateFlag = 'const ENABLE_SOURCE_DERIVED_NAVIGATION = false; // ENABLE_ONLY_AFTER_LIVE_NAVIGATION_QA';
export const candidateManifest = Object.freeze({
  appVersion: '2.19.1',
  registryKey: '__ANTIGRAVITY_TOOLS_LOCALIZATION__',
  sourceSha256: '47f36abaabd34f7d54a95df40d942b609c02db9f5c04d56b124a048571b9f16d',
  candidateEnable: 'Replace the single literal candidateFlag from false to true in a test-only runtime copy',
  controller: ['describe()', 'probe()', 'apply()', 'getStatus()', 'renewLease()', 'dispose()'],
  leaseMs: 15000,
  renewEveryMs: 3000,
  scopes: [
    { id: 'settings-button', verification: 'live-verified', selector: '[data-testid="settings-button"]', maxLabels: 1 },
    { id: 'settings-navigation', verification: 'pending-live-qa', selector: 'div[class="h-full w-full flex flex-col bg-sidebar"]', maxLabels: 13 },
  ],
  checks: [
    'Read-only probe: exact 2.19.1, each present scope unique; absent route scopes are normal',
    'Settings heading and global nav are the first two direct children of the literal scroll wrapper',
    'General/App→Application/Appearance/Skin/Notifications/Models/Customizations/Developer/Tab/Editor only',
    'Shortcuts/Provide Feedback only as direct scroll children after its unique flex-1 spacer',
    'Both active and inactive button/span class pairs; dark/light use the same classes',
    'Workspace/project names General, Models, Shortcuts and Provide Feedback remain unchanged',
    'Account display name/email, conversations, code, inputs and paths remain unchanged',
    'Apply twice; renew; dispose restores exact original English; reinject toggles cleanly',
    'Change settings tabs/routes; new known nodes are revalidated; absent scopes are skipped',
    'Stop renewal; at 15 seconds the controller disposes and restores still-owned labels',
  ],
});

export const sourceDerivedAdapter = {
  id: 'source-only-2.19.1-settings-button', appVersion: '2.19.1',
  root: { testId: 'settings-button', tagName: 'BUTTON', attributes: {} },
  fields: [{ path: [{ tagName: 'SPAN', attributes: { class: 'truncate text-sm' } }], kind: 'text', source: 'Settings' }],
};

export const navClasses = {
  root: 'h-full w-full flex flex-col bg-sidebar',
  scroll: 'flex-1 flex flex-col gap-1 py-3 overflow-y-auto',
  heading: 'm-0 text-xs font-medium text-muted-foreground select-none',
  group: 'flex flex-col gap-0.5',
  button: 'flex items-center gap-1.5 group mx-2 px-2 py-1 rounded-lg cursor-pointer border-none text-left transition-all outline-none',
  label: 'text-sm transition-colors select-none truncate flex-1',
};
export const navLabels = ['General', 'App', 'Appearance', 'Skin', 'Notifications', 'Models', 'Customizations', 'Developer', 'Tab', 'Editor'];

export function navButton(host, key, active = false, text = key === 'App' ? 'Application' : key) {
  const button = host.element('button', 'settings-nav-item-' + key);
  button.setAttribute('type', 'button');
  button.setAttribute('class', navClasses.button + (active ? ' bg-sidebar-secondary' : ' hover:bg-sidebar-muted'));
  const label = button.appendChild(host.element('span', null, text));
  label.setAttribute('class', navClasses.label + (active ? ' text-foreground' : ' text-secondary-foreground group-hover:text-foreground'));
  return button;
}

export function makeSourceDerivedFixture({ navigation = false, buttonPresent = true, active = 'General', dark = false, internal = false } = {}) {
  const host = createFixtureHost();
  if (dark) host.document.documentElement.setAttribute('class', 'dark');
  const mount = host.document.body.appendChild(host.element('div', 'offline-route-mount'));
  const button = host.element('button', 'settings-button');
  if (buttonPresent) mount.appendChild(button);
  const icon = button.appendChild(host.element('span'));
  icon.setAttribute('class', 'shrink-0 flex items-center');
  icon.appendChild(host.element('svg'));
  const label = button.appendChild(host.element('span', null, 'Settings'));
  label.setAttribute('class', 'truncate text-sm');
  const chat = host.document.body.appendChild(host.element('div', 'planner-response-text', 'Settings'));
  chat.setAttribute('class', 'markdown-body');
  const config = { appVersion: '2.19.1', locale: 'zh-CN', dictionary: { exact: {
    Settings: '设置', General: '通用', Application: '应用', Appearance: '外观', Skin: '界面样式',
    Notifications: '通知', Models: '模型配置', Customizations: '自定义扩展', Developer: '开发者',
    Tab: '代码补全', Editor: '编辑器', Shortcuts: '快捷键', 'Provide Feedback': '提供反馈',
  } } };
  const extra = {};
  if (navigation) {
    const el = (tag, cls, text) => { const node = host.element(tag, null, text); node.setAttribute('class', cls); return node; };
    const nav = mount.appendChild(el('div', navClasses.root));
    const scroll = nav.appendChild(el('div', navClasses.scroll));
    const headingWrap = scroll.appendChild(el('div', 'px-4'));
    const heading = headingWrap.appendChild(el('h1', navClasses.heading, 'Settings'));
    const global = scroll.appendChild(el('div', navClasses.group));
    const controls = new Map();
    for (const name of [...navLabels, ...(internal ? ['Jetski Chat', 'Regroup Google3 Chats'] : [])]) {
      controls.set(name, global.appendChild(navButton(host, name, active === name)));
    }
    const names = ['General', 'Models', 'Shortcuts', 'Provide Feedback'];
    const homonyms = [];
    for (const section of ['Workspaces', 'Projects']) {
      scroll.appendChild(el('div', 'px-4 mt-2')).appendChild(el('h2', navClasses.heading, section));
      const group = scroll.appendChild(el('div', navClasses.group));
      for (const name of names) homonyms.push(group.appendChild(navButton(host, name)));
    }
    const spacer = scroll.appendChild(el('div', 'flex-1'));
    const shortcuts = scroll.appendChild(navButton(host, 'Shortcuts', active === 'Shortcuts'));
    const feedback = scroll.appendChild(navButton(host, 'Provide Feedback', active === 'Provide Feedback'));
    const account = nav.appendChild(el('div', 'flex flex-col border-t border-solid border-sidebar-border py-2'));
    const accountName = account.appendChild(host.element('button', 'settings-nav-item-General', 'General'));
    const accountEmail = account.appendChild(host.element('span', null, 'Models@example.test'));
    Object.assign(extra, { nav, scroll, headingWrap, heading, global, controls, homonyms, spacer, shortcuts, feedback, account, accountName, accountEmail });
  }
  host.metrics.writes = 0;
  return { host, mount, button, icon, label, chat, config, ...extra };
}
