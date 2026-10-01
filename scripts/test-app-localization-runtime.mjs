#!/usr/bin/env node
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { runInNewContext } from 'node:vm';
import { makeFixture, syntheticAdapter } from './fixtures/app-localization/fake-dom.mjs';
import { makeSourceDerivedFixture, sourceDerivedAdapter } from './fixtures/app-localization/app-2.19.1-source.mjs';

const path = new URL('../src-tauri/resources/app-localization/runtime.js', import.meta.url);
const source = await readFile(path, 'utf8');
const registry = 'const VERIFIED_ADAPTERS = Object.freeze([]); // OFFLINE_TEST_ADAPTERS_ONLY';
assert.equal(source.split(registry).length, 2, 'The production adapter registry must remain explicit');
// Deliberately transform a COPY, never the shipped runtime or runtime config.
const syntheticSource = source.replace(registry,
  `const VERIFIED_ADAPTERS = Object.freeze(${JSON.stringify([syntheticAdapter])});`);
const createProduction = runInNewContext(source, {});
const createRuntime = runInNewContext(syntheticSource, {});
const registryKey = '__ANTIGRAVITY_TOOLS_LOCALIZATION__';
let passed = 0;
function test(name, execute) {
  execute();
  passed += 1;
  console.log(`ok ${passed} - ${name}`);
}

function assertUntouched(fixture) {
  assert.equal(fixture.settings.firstChild.data, 'Settings');
  assert.equal(fixture.settings.getAttribute('title'), 'Settings');
  assert.equal(fixture.help.getAttribute('aria-label'), 'Help');
  assert.equal(fixture.host.metrics.writes, 0);
  assert.equal(fixture.host.activeObservers, 0);
}

test('production fails closed even when config supplies an alleged adapter', () => {
  const fixture = makeFixture();
  for (const version of ['2.19.1', '999.0.0', syntheticAdapter.appVersion]) {
    const api = createProduction(fixture.host, { ...fixture.config, appVersion: version, adapters: [syntheticAdapter] });
    assert.equal(api.probe().status, 'unsupported_version');
    assert.equal(api.apply().status, 'unsupported_version');
    assertUntouched(fixture);
  }
  assert.equal(fixture.host.metrics.queries.length, 0);
});

test('probe is read-only; apply is exact and repeated apply is idempotent', () => {
  const f = makeFixture();
  const api = createRuntime(f.host, f.config);
  assert.equal(api.probe().status, 'supported');
  assertUntouched(f);
  assert.equal(api.apply().status, 'applied');
  assert.equal(f.settings.firstChild.data, '设置');
  assert.equal(f.settings.getAttribute('title'), '设置');
  assert.equal(f.help.getAttribute('aria-label'), '帮助');
  const writes = f.host.metrics.writes;
  assert.equal(api.apply().translated, 3);
  f.host.flush();
  assert.equal(f.host.metrics.writes, writes);
  assert.equal(f.host.metrics.observerInstances, 1);
  assert.equal(f.host.activeObservers, 1);
  assert.equal(f.host.pendingTimers, 1, 'Only the lease timer remains');
  assert.equal(f.settings.hasAttribute('disabled'), true, 'No permission or interaction attributes change');
  assert.deepEqual([...new Set(f.host.metrics.queries)], ['[data-testid="offline-fixture-shell"]']);
});

test('untrusted body, chat, paths, input values and dictionary lookalikes remain unchanged', () => {
  const f = makeFixture();
  const nodes = [];
  for (const tag of ['div', 'pre', 'code', 'textarea', 'input', 'script']) {
    const element = f.host.element(tag, 'offline-settings', 'Settings');
    element.setAttribute('title', 'Settings');
    element.value = 'Settings';
    f.chat.appendChild(element);
    nodes.push(element);
  }
  const path = f.chat.appendChild(f.host.element('span', null, '/home/Settings/Help.txt'));
  const outside = f.host.document.body.appendChild(f.host.element('p', null, 'Help'));
  f.config.dictionary.exact['/home/Settings/Help.txt'] = 'NEVER';
  f.config.dictionary.exact.constructor = 'NEVER';
  f.config.dictionary.exact.toString = 'NEVER';
  const api = createRuntime(f.host, f.config);
  api.apply();
  for (const element of nodes) {
    assert.equal(element.firstChild.data, 'Settings');
    assert.equal(element.getAttribute('title'), 'Settings');
    assert.equal(element.value, 'Settings');
  }
  assert.equal(path.firstChild.data, '/home/Settings/Help.txt');
  assert.equal(outside.firstChild.data, 'Help');
  const writes = f.host.metrics.writes;
  nodes[0].firstChild.data = 'New user content';
  f.host.flushObservers();
  assert.equal(f.host.pendingMutationTimers, 0, 'Chat streaming must not schedule localization');
  assert.equal(f.host.metrics.writes, writes + 1);
});

test('no trimming, substrings, regexes or partial labels are translated', () => {
  for (const value of [' Settings ', 'Open Settings', 'Settings/Help', 'settings', 'Settings\nHelp']) {
    const f = makeFixture();
    f.settings.firstChild.data = value;
    f.host.metrics.writes = 0;
    const api = createRuntime(f.host, f.config);
    assert.equal(api.apply().status, 'unsupported_dom');
    assert.equal(f.settings.firstChild.data, value);
    assert.equal(f.help.getAttribute('aria-label'), 'Help');
    assert.equal(f.host.metrics.writes, 0);
  }
});

test('all ancestor levels, inputs, user regions and editors are excluded', () => {
  const guards = [
    ['contenteditable', 'true'], ['contenteditable', 'false'], ['data-user-content', ''],
    ['data-message-id', '1'], ['data-chat-message', ''], ['data-file-path', ''],
    ['data-code', ''], ['class', 'monaco-editor'], ['class', 'xterm'], ['class', 'markdown-body'],
    ['class', 'chat-message'], ['class', 'file-path'], ['role', 'textbox'], ['role', 'log'], ['translate', 'no'],
  ];
  for (const [name, value] of guards) {
    const f = makeFixture();
    const wrapper = f.host.element('div');
    f.host.document.body.appendChild(wrapper);
    let last = wrapper;
    for (let i = 0; i < 10; i += 1) last = last.appendChild(f.host.element('div'));
    last.appendChild(f.root);
    wrapper.setAttribute(name, value);
    f.host.metrics.writes = 0;
    assert.equal(createRuntime(f.host, f.config).apply().status, 'unsupported_dom');
    assertUntouched(f);
  }
  for (const tag of ['pre', 'code', 'input', 'textarea', 'select', 'iframe', 'script']) {
    const f = makeFixture();
    const wrapper = f.host.document.body.appendChild(f.host.element(tag));
    wrapper.appendChild(f.root);
    f.host.metrics.writes = 0;
    assert.equal(createRuntime(f.host, f.config).apply().status, 'unsupported_dom');
    assertUntouched(f);
  }
});

test('duplicate root, duplicate direct anchor, wrong tag and nested replacement fail closed', () => {
  const mutations = [
    (f) => f.chat.appendChild(f.host.element('div', 'offline-fixture-shell')),
    (f) => f.toolbar.appendChild(f.host.element('button', 'offline-settings', 'Settings')),
    (f) => { f.settings.tagName = 'INPUT'; },
    (f) => { const wrap = f.toolbar.appendChild(f.host.element('div')); wrap.appendChild(f.settings); },
    (f) => f.settings.appendChild(f.host.element('span', null, 'Settings')),
    (f) => f.root.setAttribute('data-build-id', 'unknown-build'),
  ];
  for (const mutate of mutations) {
    const f = makeFixture();
    mutate(f);
    f.host.metrics.writes = 0;
    assert.equal(createRuntime(f.host, f.config).apply().status, 'unsupported_dom');
    assert.equal(f.host.metrics.writes, 0);
  }
});

test('dispose restores owned text and attributes, removes registry and cancels all work', () => {
  const f = makeFixture();
  const api = createRuntime(f.host, f.config);
  api.apply();
  f.toolbar.setAttribute('class', 'toolbar');
  f.host.flushObservers();
  assert.equal(f.host.pendingMutationTimers, 1);
  const result = api.dispose();
  assert.equal(result.restored, 3);
  assert.equal(f.settings.firstChild.data, 'Settings');
  assert.equal(f.settings.getAttribute('title'), 'Settings');
  assert.equal(f.help.getAttribute('aria-label'), 'Help');
  assert.equal(f.host[registryKey], undefined);
  assert.equal(f.host.pendingTimers, 0);
  assert.equal(f.host.activeObservers, 0);
  const writes = f.host.metrics.writes;
  f.host.flush();
  assert.equal(api.dispose().restored, 0);
  assert.equal(api.apply().status, 'disposed');
  assert.equal(f.host.metrics.writes, writes);
});

test('dispose preserves legitimate external updates, including same-value writes before callbacks', () => {
  const f = makeFixture();
  const api = createRuntime(f.host, f.config);
  api.apply();
  f.settings.firstChild.data = 'New official label';
  f.settings.setAttribute('title', '设置');
  f.help.setAttribute('aria-label', 'New help label');
  assert.equal(api.dispose().restored, 0);
  assert.equal(f.settings.firstChild.data, 'New official label');
  assert.equal(f.settings.getAttribute('title'), '设置');
  assert.equal(f.help.getAttribute('aria-label'), 'New help label');
});

test('label drift stops the engine and rolls back unaffected owned labels', () => {
  const f = makeFixture();
  const api = createRuntime(f.host, f.config);
  api.apply();
  f.settings.firstChild.data = 'Official replacement';
  f.host.flush();
  assert.equal(api.getStatus().status, 'unsupported_dom');
  assert.equal(api.getStatus().active, false);
  assert.equal(f.settings.firstChild.data, 'Official replacement');
  assert.equal(f.settings.getAttribute('title'), 'Settings');
  assert.equal(f.help.getAttribute('aria-label'), 'Help');
  assert.equal(f.host.activeObservers, 0);
  assert.equal(f.host.pendingTimers, 0);
});

test('observer reapplies exact original labels once and batches repeated mutations', () => {
  const f = makeFixture();
  const api = createRuntime(f.host, f.config);
  api.apply();
  f.settings.firstChild.data = 'Settings';
  f.settings.setAttribute('title', 'Settings');
  f.help.setAttribute('aria-label', 'Help');
  f.host.flushObservers();
  assert.equal(f.host.pendingMutationTimers, 1);
  f.host.flush();
  assert.equal(f.settings.firstChild.data, '设置');
  assert.equal(f.host.pendingMutationTimers, 0);
  assert.equal(f.host.pendingTimers, 1);
  assert.equal(f.host.metrics.observerInstances, 1);
  assert.equal(api.dispose().restored, 3);
});

test('repeated injection disposes the previous engine without duplicate observers', () => {
  const f = makeFixture();
  const first = createRuntime(f.host, f.config);
  first.apply();
  const second = createRuntime(f.host, f.config);
  assert.equal(first.getStatus().status, 'disposed');
  assert.equal(f.settings.firstChild.data, 'Settings');
  assert.equal(f.host.activeObservers, 0);
  assert.equal(second.apply().status, 'applied');
  assert.equal(f.host.activeObservers, 1);
  first.dispose();
  assert.equal(f.host[registryKey], second, 'A stale controller cannot remove its replacement');
  second.dispose();
});

test('moved labels are never restored into user content and detached nodes are released', () => {
  const f = makeFixture();
  const api = createRuntime(f.host, f.config);
  api.apply();
  f.chat.appendChild(f.settings);
  assert.equal(api.dispose().restored, 1);
  assert.equal(f.settings.firstChild.data, '设置');
  assert.equal(f.settings.getAttribute('title'), '设置');
  assert.equal(f.host.activeObservers, 0);
  assert.equal(api.getStatus().translated, 0);
});

test('replacement nodes must be revalidated and never inherit old ownership', () => {
  const f = makeFixture();
  const api = createRuntime(f.host, f.config);
  api.apply();
  const oldText = f.settings.firstChild;
  f.settings.replaceChildren(f.host.document.createTextNode('Settings'));
  f.host.flush();
  assert.equal(oldText.data, '设置');
  assert.equal(f.settings.firstChild.data, '设置');
  assert.equal(api.dispose().restored, 3);
  assert.equal(oldText.data, '设置', 'Detached nodes are not touched');
  assert.equal(f.settings.firstChild.data, 'Settings');
});

test('adapter identity change fails closed and never rewrites unknown new UI', () => {
  const f = makeFixture();
  const api = createRuntime(f.host, f.config);
  api.apply();
  f.root.setAttribute('data-build-id', 'new-release');
  f.host.flush();
  assert.equal(api.getStatus().status, 'unsupported_dom');
  assert.equal(f.host.activeObservers, 0);
  assert.equal(f.settings.firstChild.data, '设置');
  assert.equal(api.getStatus().translated, 0);
});

test('invalid config, missing exact data and dictionary accessors cannot execute', () => {
  for (const mutate of [
    (f) => { f.config.locale = 'en'; },
    (f) => { f.config.appVersion = ''; },
    (f) => { f.config.dictionary.exact = {}; },
    (f) => { f.config.dictionary.exact = Object.create({ Settings: '设置', Help: '帮助' }); },
    (f) => { Object.defineProperty(f.config.dictionary.exact, 'Settings', { get() { throw new Error('Must not execute'); } }); },
  ]) {
    const f = makeFixture();
    mutate(f);
    assert.equal(createRuntime(f.host, f.config).apply().status, 'invalid_config');
    assertUntouched(f);
  }
});

test('dictionary is snapshotted and translated values are written as plain text only', () => {
  const f = makeFixture();
  f.config.dictionary.exact.Settings = '<script>neverExecute()</script>';
  const api = createRuntime(f.host, f.config);
  f.config.dictionary.exact.Settings = 'later mutation';
  api.apply();
  assert.equal(f.settings.firstChild.data, '<script>neverExecute()</script>');
  assert.equal(f.settings.childNodes.length, 1);
  assert.equal(f.settings.firstChild.nodeType, 3);
  assert.equal(api.dispose().restored, 3);
});

test('unknown registry occupants are not called, replaced or translated through', () => {
  const f = makeFixture();
  let called = false;
  const unknown = { dispose() { called = true; } };
  f.host[registryKey] = unknown;
  const api = createRuntime(f.host, f.config);
  assert.equal(api.apply().status, 'invalid_config');
  assert.equal(called, false);
  assert.equal(f.host[registryKey], unknown);
  assertUntouched(f);
});

test('a newly ambiguous root fails closed but restores the still-owned original UI', () => {
  const f = makeFixture();
  const api = createRuntime(f.host, f.config);
  api.apply();
  const duplicate = f.chat.appendChild(f.host.element('div', 'offline-fixture-shell', 'Settings'));
  // The duplicate was added under untrusted content, so no stream-triggered
  // reapply is scheduled. A manual apply still detects the global ambiguity.
  assert.equal(api.apply().status, 'unsupported_dom');
  assert.equal(f.settings.firstChild.data, 'Settings');
  assert.equal(f.settings.getAttribute('title'), 'Settings');
  assert.equal(duplicate.firstChild.data, 'Settings');
  assert.equal(f.host.activeObservers, 0);
  assert.equal(f.host.pendingTimers, 0);
});

test('partial write failure rolls back earlier owned writes and cancels all work', () => {
  const f = makeFixture();
  const nativeSet = f.settings.setAttribute.bind(f.settings);
  f.settings.setAttribute = (name, value) => {
    if (name === 'title' && value === '设置') throw new Error('Synthetic setter failure');
    nativeSet(name, value);
  };
  const api = createRuntime(f.host, f.config);
  assert.equal(api.apply().status, 'runtime_error');
  assert.equal(f.settings.firstChild.data, 'Settings');
  assert.equal(f.settings.getAttribute('title'), 'Settings');
  assert.equal(f.help.getAttribute('aria-label'), 'Help');
  assert.equal(f.host.activeObservers, 0);
  assert.equal(f.host.pendingTimers, 0);
});

test('reinjection with an unsupported version restores the prior supported engine', () => {
  const f = makeFixture();
  const first = createRuntime(f.host, f.config);
  first.apply();
  const replacement = createProduction(f.host, { ...f.config, appVersion: 'unknown-version' });
  assert.equal(first.getStatus().status, 'disposed');
  assert.equal(replacement.apply().status, 'unsupported_version');
  assert.equal(f.settings.firstChild.data, 'Settings');
  assert.equal(f.host.activeObservers, 0);
  assert.equal(f.host.pendingTimers, 0);
});

test('lease expires at 15 seconds, cancels work and restores safely when Tools stops renewing', () => {
  const f = makeFixture();
  const api = createRuntime(f.host, f.config);
  assert.equal(api.renewLease().status, 'inactive');
  assert.equal(f.host.pendingTimers, 0);
  api.apply();
  f.host.advanceTime(14999);
  assert.equal(api.getStatus().active, true);
  f.host.advanceTime(1);
  assert.equal(api.getStatus().status, 'disposed');
  assert.equal(api.getStatus().reason, 'lease_expired');
  assert.equal(f.settings.firstChild.data, 'Settings');
  assert.equal(f.host.activeObservers, 0);
  assert.equal(f.host.pendingTimers, 0);
  assert.equal(f.host[registryKey], undefined);
  assert.equal(api.renewLease().status, 'disposed');
});

test('explicit renewLease keeps a single timer and extends only the owner lease', () => {
  const f = makeFixture();
  const api = createRuntime(f.host, f.config);
  api.apply();
  for (let i = 0; i < 5; i += 1) {
    f.host.advanceTime(3000);
    assert.equal(api.renewLease().leaseMs, 15000);
    assert.equal(f.host.pendingTimers, 1);
  }
  f.host.advanceTime(14999);
  assert.equal(api.getStatus().active, true);
  f.host.advanceTime(1);
  assert.equal(api.getStatus().reason, 'lease_expired');
  assert.equal(f.host.pendingTimers, 0);
});

test('manual and mutation-driven repeated apply cannot extend an abandoned lease', () => {
  const f = makeFixture();
  const api = createRuntime(f.host, f.config);
  api.apply();
  f.host.advanceTime(12000);
  api.apply();
  f.settings.firstChild.data = 'Settings';
  f.host.flush();
  assert.equal(f.settings.firstChild.data, '设置');
  f.host.advanceTime(3000);
  assert.equal(api.getStatus().reason, 'lease_expired');
  assert.equal(f.settings.firstChild.data, 'Settings');
  assert.equal(f.host.pendingTimers, 0);
});

const sourceDerivedRuntime = runInNewContext(source.replace(registry,
  `const VERIFIED_ADAPTERS = Object.freeze(${JSON.stringify([sourceDerivedAdapter])});`), {});

test('2.19.1 source-derived Settings control translates and restores only its literal span', () => {
  const f = makeSourceDerivedFixture();
  const api = sourceDerivedRuntime(f.host, f.config);
  assert.equal(api.apply().status, 'applied');
  assert.equal(f.label.firstChild.data, '设置');
  assert.equal(f.chat.firstChild.data, 'Settings');
  assert.equal(f.icon.children[0].tagName, 'SVG');
  assert.equal(api.dispose().status, 'disposed');
  assert.equal(f.label.firstChild.data, 'Settings');
  assert.equal(f.host.pendingTimers, 0);
});

test('source-derived Settings control refuses label, structure and release drift', () => {
  for (const mutate of [
    f => { f.label.firstChild.data = 'Settings file.txt'; },
    f => { f.label.setAttribute('class', 'truncate text-sm changed'); },
    f => { f.config.appVersion = '2.19.2'; },
    f => { f.label.appendChild(f.host.element('span', null, 'Settings')); },
  ]) {
    const f = makeSourceDerivedFixture();
    mutate(f); f.host.metrics.writes = 0;
    assert.notEqual(sourceDerivedRuntime(f.host, f.config).apply().status, 'applied');
    assert.equal(f.host.metrics.writes, 0);
  }
});

test('source-derived Settings label inside user content is never translated', () => {
  const f = makeSourceDerivedFixture();
  f.chat.appendChild(f.button); f.host.metrics.writes = 0;
  assert.equal(sourceDerivedRuntime(f.host, f.config).apply().status, 'unsupported_dom');
  assert.equal(f.label.firstChild.data, 'Settings');
  assert.equal(f.host.metrics.writes, 0);
});

test('static source evidence alone never enables the production 2.19.1 adapter', () => {
  const f = makeSourceDerivedFixture();
  assert.equal(createProduction(f.host, f.config).apply().status, 'unsupported_version');
  assert.equal(f.host.metrics.writes, 0);
});

console.log(`\n${passed} offline runtime tests passed. No production App compatibility is asserted.`);
