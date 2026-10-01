#!/usr/bin/env node
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { runInNewContext } from 'node:vm';
import { makeFixture, syntheticAdapter } from './fixtures/app-localization/fake-dom.mjs';
import { makeSourceDerivedFixture, sourceDerivedAdapter, productionManifest, navButton, navClasses, navLabels, verifiedNavLabels, unverifiedNavLabels, verifiedNavSources } from './fixtures/app-localization/app-2.19.1-source.mjs';

const path = new URL('../src-tauri/resources/app-localization/runtime.js', import.meta.url);
const source = await readFile(path, 'utf8');
const provenance = JSON.parse(await readFile(new URL('../src-tauri/resources/app-localization/SOURCE.json', import.meta.url), 'utf8'));
const registry = source.slice(source.indexOf('  const VERIFIED_ADAPTERS ='), source.indexOf('// PRODUCTION_ADAPTERS_END') + '// PRODUCTION_ADAPTERS_END'.length).trim();
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
  for (const version of ['unknown', '2.19.2', '999.0.0', syntheticAdapter.appVersion]) {
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

const labelOf = button => button.children[0].firstChild;

function assertUserNamesUntouched(f) {
  for (const [index, node] of f.homonyms.entries()) {
    assert.equal(labelOf(node).data, ['General', 'Models', 'Shortcuts', 'Provide Feedback'][index % 4]);
  }
  assert.equal(f.accountName.firstChild.data, 'General');
  assert.equal(f.accountEmail.firstChild.data, 'Models@example.test');
  assert.equal(f.chat.firstChild.data, 'Settings');
}

test('production manifest and read-only probe expose exact scopes without external content', () => {
  const f = makeSourceDerivedFixture({ navigation: true });
  const api = createProduction(f.host, f.config);
  assert.equal(productionManifest.appVersion, '2.19.1');
  assert.equal(api.describe().navigationCandidateEnabled, false);
  assert.deepEqual([...api.describe().scopes.map(s => s.id)], ['settings-button', 'settings-navigation']);
  assert.deepEqual([...api.describe().scopes[1].sources], verifiedNavSources);
  assert.deepEqual([...api.describe().scopes[1].untranslatedSources], unverifiedNavLabels);
  assert.deepEqual(productionManifest.scopes[1].sources, verifiedNavSources);
  assert.equal(productionManifest.scopes[1].maxLabels, 8);
  assert.equal(api.probe().labelCount, 9);
  assert.equal(api.probe().scopes[1].labelCount, 8);
  assert.equal(api.probe().scopes[1].verification, 'live-verified');
  assert.equal(f.host.metrics.writes, 0);
  assert.equal(f.host.activeObservers, 0);
  assert.equal(f.host.pendingTimers, 0);
  assert.equal(JSON.stringify(api.probe()).includes('example.test'), false);
});

test('recorded acceptance and production manifests agree on the exact eight-plus-one scope', () => {
  assert.deepEqual(provenance.production_supported_app_versions, ['2.19.1']);
  assert.deepEqual(provenance.production_verified_navigation_labels, verifiedNavSources);
  assert.deepEqual(provenance.production_untranslated_navigation_labels, unverifiedNavLabels);
  assert.deepEqual(provenance.candidate_scope.labels, unverifiedNavLabels);
  assert.deepEqual(provenance.macos_navigation_acceptance.navigation_labels, verifiedNavSources);
  assert.equal(provenance.macos_navigation_acceptance.app_version, '2.19.1');
  assert.equal(provenance.macos_navigation_acceptance.independent_settings_button, true);
  assert.equal(provenance.macos_navigation_acceptance.label_count, 9);
  assert.equal(provenance.macos_navigation_acceptance.navigation_label_count, 8);
  assert.equal(provenance.macos_navigation_acceptance.duration_ms, 2000);
  assert.equal(provenance.macos_navigation_acceptance.changed, true);
  assert.equal(provenance.macos_navigation_acceptance.restored, true);
  assert.equal(provenance.macos_navigation_acceptance.english_postcheck, true);
});

test('production translates the global group and fixed tail, never homonymous projects or account', () => {
  const f = makeSourceDerivedFixture({ navigation: true, internal: true });
  const api = createProduction(f.host, f.config);
  assert.equal(api.apply().translated, 9);
  assert.equal(f.heading.firstChild.data, '设置');
  for (const key of verifiedNavLabels) assert.equal(labelOf(f.controls.get(key)).data, f.config.dictionary.exact[key === 'App' ? 'Application' : key]);
  assert.equal(labelOf(f.shortcuts).data, '快捷键');
  assert.equal(labelOf(f.feedback).data, '提供反馈');
  assert.equal(labelOf(f.controls.get('Jetski Chat')).data, 'Jetski Chat');
  assert.equal(labelOf(f.controls.get('Regroup Google3 Chats')).data, 'Regroup Google3 Chats');
  for (const key of unverifiedNavLabels) assert.equal(labelOf(f.controls.get(key)).data, key);
  assert.equal(f.host.metrics.writes, 9, 'The full source fixture must still write only nine verified labels');
  assertUserNamesUntouched(f);
  assert.equal(api.dispose().restored, 9);
  assert.equal(f.heading.firstChild.data, 'Settings');
  for (const key of navLabels) assert.equal(labelOf(f.controls.get(key)).data, key === 'App' ? 'Application' : key);
  assertUserNamesUntouched(f);
  assert.equal(f.host.pendingTimers, 0);
  assert.equal(f.host.activeObservers, 0);
});

test('the exact observed macOS navigation subset translates eight labels plus the independent button', () => {
  const f = makeSourceDerivedFixture({ navigation: true, verifiedOnly: true });
  const api = createProduction(f.host, f.config);
  assert.equal(api.probe().labelCount, 9);
  assert.equal(api.apply().translated, 9);
  assert.equal(api.getStatus().scopes[1].labelCount, 8);
  assertUserNamesUntouched(f);
  assert.equal(api.dispose().restored, 9);
  assert.equal(f.heading.firstChild.data, 'Settings');
  for (const key of verifiedNavLabels) assert.equal(labelOf(f.controls.get(key)).data, key === 'App' ? 'Application' : key);
  assertUserNamesUntouched(f);
});

test('unverified dictionaries and config candidate flags cannot expand production scope', () => {
  for (const withAccessors of [false, true]) {
    const f = makeSourceDerivedFixture({ navigation: true });
    for (const key of unverifiedNavLabels) {
      delete f.config.dictionary.exact[key];
      if (withAccessors) Object.defineProperty(f.config.dictionary.exact, key, {
        get() { throw new Error('Unverified dictionary entry must never be read'); },
      });
    }
    Object.assign(f.config, { navigationCandidateEnabled: true, ENABLE_SOURCE_DERIVED_NAVIGATION: true,
      verifiedNavSources: navLabels, adapters: [sourceDerivedAdapter] });
    const api = createProduction(f.host, f.config);
    assert.equal(api.apply().translated, 9);
    for (const key of unverifiedNavLabels) assert.equal(labelOf(f.controls.get(key)).data, key);
    assert.equal(api.dispose().restored, 9);
    assertUserNamesUntouched(f);
  }
});

test('known unverified controls still require exact static labels and structure before any write', () => {
  const mutations = [
    (f, key) => { labelOf(f.controls.get(key)).data = key + ' project'; },
    (f, key) => { f.controls.get(key).setAttribute('type', 'submit'); },
    (f, key) => { f.controls.get(key).children[0].appendChild(f.host.element('span', null, key)); },
    (f, key) => { f.controls.get(key).setAttribute('data-user-content', ''); },
    (f, key) => { f.global.appendChild(navButton(f.host, key)); },
  ];
  for (const key of unverifiedNavLabels) for (const mutate of mutations) {
    const f = makeSourceDerivedFixture({ navigation: true });
    mutate(f, key);
    f.host.metrics.writes = 0;
    const api = createProduction(f.host, f.config);
    assert.equal(api.apply().status, 'unsupported_dom');
    assert.equal(f.host.metrics.writes, 0);
    assert.equal(f.host.activeObservers, 0);
    assertUserNamesUntouched(f);
  }
});

test('unverified label drift stops production and restores nine still-owned labels', () => {
  for (const key of unverifiedNavLabels) {
    const f = makeSourceDerivedFixture({ navigation: true });
    const api = createProduction(f.host, f.config);
    assert.equal(api.apply().translated, 9);
    labelOf(f.controls.get(key)).data = key + ' changed by App';
    f.host.flush();
    assert.equal(api.getStatus().status, 'unsupported_dom');
    assert.equal(api.getStatus().active, false);
    assert.equal(api.getStatus().translated, 0);
    assert.equal(labelOf(f.controls.get(key)).data, key + ' changed by App');
    assert.equal(f.label.firstChild.data, 'Settings');
    assert.equal(f.heading.firstChild.data, 'Settings');
    for (const verified of verifiedNavLabels) assert.equal(labelOf(f.controls.get(verified)).data, verified === 'App' ? 'Application' : verified);
    assert.equal(labelOf(f.shortcuts).data, 'Shortcuts');
    assert.equal(labelOf(f.feedback).data, 'Provide Feedback');
    assert.equal(f.host.pendingTimers, 0);
    assert.equal(f.host.activeObservers, 0);
    assertUserNamesUntouched(f);
  }
});

test('production navigation stays disabled for every non-exact release spelling', () => {
  for (const appVersion of ['2.19.2', '2.19.0', '2.19.1-beta', '2.19.1+build', '2.19.1 ', 'v2.19.1']) {
    const f = makeSourceDerivedFixture({ navigation: true });
    const api = createProduction(f.host, { ...f.config, appVersion });
    assert.equal(api.probe().status, 'unsupported_version');
    assert.equal(api.apply().status, 'unsupported_version');
    assert.equal(f.host.metrics.writes, 0);
    assert.equal(f.host.metrics.queries.length, 0);
    assertUserNamesUntouched(f);
  }
});

test('dark/light and every active nav class variant are accepted without changing classes', () => {
  for (const dark of [false, true]) for (const active of [...navLabels, 'Shortcuts', 'Provide Feedback', 'none']) {
    const f = makeSourceDerivedFixture({ navigation: true, buttonPresent: false, active, dark });
    const api = createProduction(f.host, f.config);
    const controls = [...f.controls.values(), f.shortcuts, f.feedback];
    const classes = controls.map(control => [control.getAttribute('class'), control.children[0].getAttribute('class')]);
    assert.equal(api.apply().translated, 8);
    assert.deepEqual(controls.map(control => [control.getAttribute('class'), control.children[0].getAttribute('class')]), classes);
    assert.equal(api.dispose().restored, 8);
  }
});

test('main screen and settings screen are independent optional scopes; absent route can wait', () => {
  for (const [navigation, buttonPresent, count] of [[false, true, 1], [true, false, 8], [true, true, 9], [false, false, 0]]) {
    const f = makeSourceDerivedFixture({ navigation, buttonPresent });
    const api = createProduction(f.host, f.config);
    assert.equal(api.probe().status, 'supported');
    assert.equal(api.apply().translated, count);
    assert.equal(api.getStatus().awaitingScope, count === 0);
    assert.equal(f.host.pendingTimers, 1);
    api.dispose();
  }
});

test('production supports conditional screen and tail omissions without using other groups', () => {
  const f = makeSourceDerivedFixture({ navigation: true });
  f.global.removeChild(f.controls.get('Developer'));
  f.scroll.removeChild(f.shortcuts);
  f.scroll.removeChild(f.feedback);
  const api = createProduction(f.host, f.config);
  assert.equal(api.apply().translated, 7);
  assertUserNamesUntouched(f);
  assert.equal(api.dispose().restored, 7);
});

test('production repeated apply, same-label official re-render and active-tab changes are idempotent', () => {
  const f = makeSourceDerivedFixture({ navigation: true });
  const api = createProduction(f.host, f.config);
  api.apply();
  const writes = f.host.metrics.writes;
  for (let i = 0; i < 3; i += 1) assert.equal(api.apply().translated, 9);
  assert.equal(f.host.metrics.writes, writes);
  const general = f.controls.get('General');
  general.setAttribute('class', navClasses.button + ' hover:bg-sidebar-muted');
  general.children[0].setAttribute('class', navClasses.label + ' text-secondary-foreground group-hover:text-foreground');
  const models = f.controls.get('Models');
  models.setAttribute('class', navClasses.button + ' bg-sidebar-secondary');
  models.children[0].setAttribute('class', navClasses.label + ' text-foreground');
  labelOf(models).data = 'Models';
  f.host.flush();
  assert.equal(api.getStatus().translated, 9);
  assert.equal(labelOf(models).data, '模型配置');
  assert.equal(api.dispose().restored, 9);
  assert.equal(labelOf(models).data, 'Models');
  assert.equal(f.host.pendingTimers, 0);
});

test('production reinjection and disable restore all owned labels with one observer and lease', () => {
  const f = makeSourceDerivedFixture({ navigation: true });
  const first = createProduction(f.host, f.config);
  first.apply();
  const second = createProduction(f.host, f.config);
  assert.equal(first.getStatus().status, 'disposed');
  assert.equal(labelOf(f.controls.get('Models')).data, 'Models');
  assert.equal(second.apply().translated, 9);
  assert.equal(f.host.activeObservers, 1);
  assert.equal(f.host.pendingTimers, 1);
  first.dispose();
  assert.equal(f.host[registryKey], second);
  assert.equal(second.dispose().restored, 9);
  assert.equal(f.host.activeObservers, 0);
});

test('production label drift stops and rolls back unaffected ownership while preserving external writes', () => {
  const f = makeSourceDerivedFixture({ navigation: true });
  const api = createProduction(f.host, f.config);
  api.apply();
  labelOf(f.controls.get('Models')).data = 'Official Models replacement';
  // Same-value writes also belong to the App, never our restore operation.
  labelOf(f.controls.get('General')).data = '通用';
  f.host.flush();
  assert.equal(api.getStatus().status, 'unsupported_dom');
  assert.equal(api.getStatus().active, false);
  assert.equal(labelOf(f.controls.get('Models')).data, 'Official Models replacement');
  assert.equal(labelOf(f.controls.get('General')).data, '通用');
  assert.equal(f.label.firstChild.data, 'Settings');
  assert.equal(f.heading.firstChild.data, 'Settings');
  assert.equal(labelOf(f.feedback).data, 'Provide Feedback');
  assertUserNamesUntouched(f);
  assert.equal(f.host.activeObservers, 0);
  assert.equal(f.host.pendingTimers, 0);
});

test('production unknown, duplicate, moved and user-content nav shapes fail before any write', () => {
  const mutations = [
    f => f.global.appendChild(navButton(f.host, 'General')),
    f => f.global.appendChild(navButton(f.host, 'Account')),
    f => f.global.appendChild(navButton(f.host, 'New Future Tab')),
    f => f.global.appendChild(navButton(f.host, 'toString')),
    f => f.global.appendChild(navButton(f.host, 'constructor')),
    f => f.global.appendChild(navButton(f.host, '__proto__')),
    f => f.global.appendChild(f.host.element('input', null, 'Models')),
    f => { f.controls.get('General').setAttribute('type', 'submit'); },
    f => { f.controls.get('General').children[0].setAttribute('class', navClasses.label + ' text-foreground extra'); },
    f => { f.global.removeChild(f.controls.get('General')); f.global.appendChild(navButton(f.host, 'General', false, 'General/project')); },
    f => { f.scroll.removeChild(f.headingWrap); f.scroll.appendChild(f.headingWrap); },
    f => { f.scroll.removeChild(f.global); f.scroll.appendChild(f.global); },
    f => { f.spacer.setAttribute('class', 'flex-1 changed'); },
    f => f.scroll.appendChild(navButton(f.host, 'Shortcuts')),
    f => { f.nav.setAttribute('data-user-content', ''); },
    f => { f.chat.appendChild(f.nav); },
    f => { f.heading.appendChild(f.host.element('span', null, 'Settings')); },
    f => { f.global.setAttribute('contenteditable', 'false'); },
    f => { const duplicate = f.host.element('div'); duplicate.setAttribute('class', navClasses.root); f.mount.appendChild(duplicate); },
  ];
  for (const mutate of mutations) {
    const f = makeSourceDerivedFixture({ navigation: true });
    mutate(f); f.host.metrics.writes = 0;
    const result = createProduction(f.host, f.config).apply();
    assert.equal(result.status, 'unsupported_dom');
    assert.equal(f.host.metrics.writes, 0);
    assert.equal(f.label.firstChild.data, 'Settings');
    assertUserNamesUntouched(f);
  }
});

test('production navigation selectors never authorize homonymous groups without the exact header', () => {
  const f = makeSourceDerivedFixture({ navigation: true, buttonPresent: false });
  f.heading.firstChild.data = 'Workspaces';
  f.host.metrics.writes = 0;
  assert.equal(createProduction(f.host, f.config).apply().status, 'unsupported_dom');
  assert.equal(f.host.metrics.writes, 0);
  assertUserNamesUntouched(f);
});

test('production route replacement drops detached ownership and revalidates newly mounted roots', () => {
  const f = makeSourceDerivedFixture({ navigation: true, buttonPresent: false });
  const api = createProduction(f.host, f.config);
  api.apply();
  const oldNav = f.nav;
  f.mount.removeChild(oldNav);
  f.host.flush();
  assert.equal(api.getStatus().active, true);
  assert.equal(api.getStatus().translated, 0);
  assert.equal(api.getStatus().awaitingScope, true);
  f.mount.appendChild(f.button);
  f.host.flush();
  assert.equal(f.label.firstChild.data, '设置');
  assert.equal(api.getStatus().translated, 1);
  const old = f.label.firstChild;
  f.label.replaceChildren(f.host.document.createTextNode('Settings'));
  f.host.flush();
  assert.equal(f.label.firstChild.data, '设置');
  assert.equal(api.dispose().restored, 1);
  assert.equal(old.data, '设置', 'Detached old text has no retained ownership');
  assert.equal(f.heading.firstChild.data, '设置', 'Detached route is not modified');
  assert.equal(f.host.pendingTimers, 0);
});

test('production awaiting route discovers nested inserted controls without global text reads', () => {
  const f = makeSourceDerivedFixture({ buttonPresent: false });
  const api = createProduction(f.host, f.config);
  api.apply();
  const wrapper = f.host.element('div');
  wrapper.appendChild(f.button);
  f.mount.appendChild(wrapper);
  f.host.flush();
  assert.equal(api.getStatus().translated, 1);
  assert.equal(f.label.firstChild.data, '设置');
  assert.equal(api.dispose().restored, 1);
});

test('production renew rechecks version-bound scopes without extending on ordinary apply', () => {
  const f = makeSourceDerivedFixture({ navigation: true });
  const api = createProduction(f.host, f.config);
  api.apply();
  for (let i = 0; i < 4; i += 1) { f.host.advanceTime(3000); assert.equal(api.renewLease().leaseMs, 15000); }
  f.host.advanceTime(12000);
  api.apply();
  labelOf(f.controls.get('Models')).data = 'Models'; f.host.flush();
  f.host.advanceTime(2999);
  assert.equal(api.getStatus().active, true);
  f.host.advanceTime(1);
  assert.equal(api.getStatus().reason, 'lease_expired');
  assert.equal(labelOf(f.controls.get('Models')).data, 'Models');
  assert.equal(f.heading.firstChild.data, 'Settings');
  assert.equal(f.host[registryKey], undefined);
  assert.equal(f.host.pendingTimers, 0);
  assert.equal(f.host.activeObservers, 0);
  assertUserNamesUntouched(f);
});

test('production protected reparenting prevents rollback into user content', () => {
  const f = makeSourceDerivedFixture({ navigation: true });
  const api = createProduction(f.host, f.config);
  api.apply();
  f.chat.appendChild(f.nav);
  assert.equal(api.dispose().restored, 1, 'Only the safe main-screen button may be restored');
  assert.equal(f.heading.firstChild.data, '设置');
  assert.equal(f.host.activeObservers, 0);
  assert.equal(f.host.pendingTimers, 0);
});

test('production ignores account, workspace and chat streaming mutations', () => {
  const f = makeSourceDerivedFixture({ navigation: true });
  const api = createProduction(f.host, f.config);
  api.apply();
  f.accountName.firstChild.data = 'Models';
  labelOf(f.homonyms[0]).data = 'Models';
  f.chat.firstChild.data = 'General';
  f.host.flushObservers();
  assert.equal(f.host.pendingMutationTimers, 0);
  assert.equal(api.dispose().restored, 9);
  assert.equal(f.accountName.firstChild.data, 'Models');
  assert.equal(labelOf(f.homonyms[0]).data, 'Models');
  assert.equal(f.chat.firstChild.data, 'General');
});

test('optional scope identity drift fails closed rather than being treated as a route removal', () => {
  for (const mutate of [
    f => f.nav.setAttribute('class', navClasses.root + ' changed'),
    f => f.button.removeAttribute('data-testid'),
  ]) {
    const f = makeSourceDerivedFixture({ navigation: true });
    const api = createProduction(f.host, f.config);
    api.apply(); mutate(f); f.host.flush();
    assert.equal(api.getStatus().status, 'unsupported_dom');
    assert.equal(api.getStatus().active, false);
    assert.equal(f.host.activeObservers, 0);
    assert.equal(f.host.pendingTimers, 0);
  }
});

test('connected root moves and protected document ancestors stop without touching newly owned content', () => {
  for (const mutate of [
    f => { const wrapper = f.host.document.body.appendChild(f.host.element('div')); wrapper.appendChild(f.nav); },
    f => f.host.document.documentElement.setAttribute('contenteditable', 'true'),
  ]) {
    const f = makeSourceDerivedFixture({ navigation: true });
    const api = createProduction(f.host, f.config);
    api.apply(); mutate(f); f.host.flush();
    assert.equal(api.getStatus().status, 'unsupported_dom');
    assert.equal(api.getStatus().active, false);
    assert.equal(f.heading.firstChild.data, '设置');
    assert.equal(f.host.activeObservers, 0);
    assert.equal(f.host.pendingTimers, 0);
  }
});

console.log(`\n${passed} offline runtime tests passed. Production is restricted to the separately live-verified Settings button and eight navigation labels.`);
