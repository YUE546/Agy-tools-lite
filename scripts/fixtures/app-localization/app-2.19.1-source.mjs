// Original minimal fixture derived from static inspection, NOT a captured DOM.
// Official build 2.19.1-6046815158665216, linux-x64. See docs/app-localization.md.
// Do not put this adapter in the production registry without real-App QA.
import { createFixtureHost } from './fake-dom.mjs';

export const sourceDerivedAdapter = {
  id: 'source-only-2.19.1-settings-button',
  appVersion: '2.19.1',
  root: { testId: 'settings-button', tagName: 'BUTTON', attributes: {} },
  fields: [{
    path: [{ tagName: 'SPAN', attributes: { class: 'truncate text-sm' } }],
    kind: 'text', source: 'Settings',
  }],
};

export function makeSourceDerivedFixture() {
  const host = createFixtureHost();
  const button = host.document.body.appendChild(host.element('button', 'settings-button'));
  const icon = button.appendChild(host.element('span'));
  icon.setAttribute('class', 'shrink-0 flex items-center');
  icon.appendChild(host.element('svg'));
  const label = button.appendChild(host.element('span', null, 'Settings'));
  label.setAttribute('class', 'truncate text-sm');
  const chat = host.document.body.appendChild(host.element('div', 'planner-response-text', 'Settings'));
  chat.setAttribute('class', 'markdown-body');
  const config = { appVersion: '2.19.1', locale: 'zh-CN', dictionary: { exact: { Settings: '设置' } } };
  host.metrics.writes = 0;
  return { host, button, icon, label, chat, config };
}
