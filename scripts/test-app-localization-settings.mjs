#!/usr/bin/env node
// Execute the real Settings component with deterministic hook and IPC doubles.
// No browser, DOM snapshot, local debugging connection, or user data is needed.
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { runInNewContext } from 'node:vm';
import ts from 'typescript';

const source = await readFile(new URL('../src/components/settings/AppLocalizationSettings.tsx', import.meta.url), 'utf8');
const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.CommonJS, jsx: ts.JsxEmit.ReactJSX, target: ts.ScriptTarget.ES2020 },
}).outputText;

const applied = Object.freeze({
  enabled: true, active: true, translated: 1, state: 'applied', can_apply: true,
  installed_version: '2.19.1', dictionary_version: 'fixture', dictionary_entries: 1,
  supported_versions: ['2.19.1'], supported: true, detail: null,
});
const off = Object.freeze({ ...applied, enabled: false, active: false, translated: 0, state: 'disabled' });
const stale = Object.freeze({ ...applied, translated: 999 });

function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
async function settle() {
  // IPC and loadConfig each cross promise boundaries; no wall-clock sleeps.
  for (let index = 0; index < 20; index++) await Promise.resolve();
}
function walk(node, predicate, matches = []) {
  if (!node) return matches;
  if (Array.isArray(node)) {
    for (const child of node) walk(child, predicate, matches);
  } else if (typeof node === 'object') {
    if (predicate(node)) matches.push(node);
    walk(node.props?.children, predicate, matches);
  }
  return matches;
}
function hasText(node, text) {
  if (node === text) return true;
  if (Array.isArray(node)) return node.some(child => hasText(child, text));
  return Boolean(node && typeof node === 'object' && hasText(node.props?.children, text));
}

function harness() {
  const states = [];
  const refs = [];
  const calls = [];
  let stateIndex = 0;
  let refIndex = 0;
  let effect;
  let cleanup;
  let interval;
  let handler = async command => {
    assert.equal(command, 'get_app_localization_status');
    return applied;
  };
  let configLoads = 0;
  const imports = {
    react: {
      useState(initial) {
        const index = stateIndex++;
        if (!(index in states)) states[index] = typeof initial === 'function' ? initial() : initial;
        return [states[index], value => { states[index] = typeof value === 'function' ? value(states[index]) : value; }];
      },
      useRef(initial) { const index = refIndex++; return refs[index] ?? (refs[index] = { current: initial }); },
      useCallback: callback => callback,
      useEffect: callback => { effect ??= callback; },
    },
    'react/jsx-runtime': {
      jsx: (type, props) => ({ type, props }),
      jsxs: (type, props) => ({ type, props }),
    },
    'lucide-react': Object.fromEntries(['AlertTriangle', 'Languages', 'RefreshCw', 'RotateCcw'].map(name => [name, name])),
    'react-i18next': { useTranslation: () => ({ t: key => key }) },
    '../../stores/useConfigStore': { useConfigStore: () => ({
      config: { app_localization: { enabled: true } },
      loadConfig: async () => { configLoads += 1; },
    }) },
    '../../utils/request': { request: (command, args) => {
      calls.push({ command, args });
      return handler(command, args);
    } },
  };
  const module = { exports: {} };
  runInNewContext(compiled, {
    exports: module.exports,
    require: name => { assert.ok(name in imports, `Unexpected import: ${name}`); return imports[name]; },
    window: {
      setInterval(callback) { interval = callback; return 1; },
      clearInterval() { interval = undefined; },
    },
  });
  function render() {
    stateIndex = 0;
    refIndex = 0;
    return module.exports.default();
  }
  function button(key) {
    const found = walk(render(), node => node.type === 'button' && hasText(node.props?.children, `app_localization.${key}`));
    assert.equal(found.length, 1, `Expected one ${key} button`);
    return found[0];
  }
  return {
    async mount() { render(); cleanup = effect(); await settle(); },
    unmount() { cleanup(); },
    setHandler(next) { handler = next; },
    poll() { assert.ok(interval, 'Component polling must be mounted'); interval(); },
    button,
    render,
    get status() { return states[0]; },
    get busy() { return states[1]; },
    get error() { return states[2]; },
    get configLoads() { return configLoads; },
    calls,
  };
}

let passed = 0;
async function test(name, body) {
  await body();
  passed += 1;
  console.log(`ok ${passed} - ${name}`);
}

await test('failed off persistence remains retryable across successful and failed polls until saved', async () => {
  const h = harness();
  await h.mount();
  let failSave = true;
  let failPoll = false;
  h.setHandler(async (command, args) => {
    if (command === 'get_app_localization_status') {
      if (failPoll) throw 'connection_lost';
      return off;
    }
    assert.equal(command, 'set_app_localization_enabled');
    assert.equal(args.enabled, false);
    if (failSave) throw 'disable_not_saved';
    return off;
  });
  h.button('restore').props.onClick();
  await settle();
  assert.equal(h.error, 'disable_not_saved');
  h.poll();
  await settle();
  assert.equal(h.status.enabled, false);
  assert.equal(h.button('restore').props.disabled, false, 'Off must remain retryable after polling disabled status');
  failPoll = true;
  h.poll();
  await settle();
  assert.equal(h.error, 'connection_lost');
  assert.equal(h.button('restore').props.disabled, false, 'A later poll error must not erase the pending save');
  failSave = false;
  h.button('restore').props.onClick();
  await settle();
  assert.equal(h.error, null);
  assert.equal(h.configLoads, 1);
  assert.equal(h.button('restore').props.disabled, true, 'Successful dedicated save clears the retry requirement');
  h.unmount();
});

for (const completion of ['during', 'after']) {
  await test(`an older status poll cannot overwrite disable ${completion} the operation`, async () => {
    const h = harness();
    await h.mount();
    const poll = deferred();
    const operation = deferred();
    h.setHandler((command, args) => {
      if (command === 'get_app_localization_status') return poll.promise;
      assert.equal(command, 'set_app_localization_enabled');
      assert.equal(args.enabled, false);
      return operation.promise;
    });
    h.poll();
    h.button('restore').props.onClick();
    assert.equal(h.busy, true);
    if (completion === 'during') {
      poll.resolve(stale);
      await settle();
      assert.equal(h.status, applied);
    }
    operation.resolve(off);
    await settle();
    assert.equal(h.status, off);
    if (completion === 'after') {
      poll.resolve(stale);
      await settle();
      assert.equal(h.status, off, 'Generation must reject an old poll even after the operation releases its guard');
    }
    assert.equal(h.busy, false);
    h.unmount();
  });
}

await test('an older failing poll cannot replace a successful operation result with an error', async () => {
  const h = harness();
  await h.mount();
  const poll = deferred();
  h.setHandler(command => command === 'get_app_localization_status' ? poll.promise : Promise.resolve(off));
  h.poll();
  h.button('restore').props.onClick();
  await settle();
  assert.equal(h.status, off);
  poll.reject('stale_poll_failure');
  await settle();
  assert.equal(h.error, null);
  h.unmount();
});

await test('same-render double clicks and background polls do not duplicate an in-flight command', async () => {
  const h = harness();
  await h.mount();
  const operation = deferred();
  h.setHandler(command => {
    assert.equal(command, 'set_app_localization_enabled');
    return operation.promise;
  });
  const click = h.button('restore').props.onClick;
  click();
  click();
  h.poll();
  assert.equal(h.calls.filter(call => call.command === 'set_app_localization_enabled').length, 1);
  assert.equal(h.calls.filter(call => call.command === 'get_app_localization_status').length, 1);
  assert.equal(h.button('restore').props.disabled, true);
  operation.resolve(off);
  await settle();
  assert.equal(h.busy, false);
  h.unmount();
});

await test('an unmounted component ignores a pending status response and stops polling', async () => {
  const h = harness();
  await h.mount();
  const poll = deferred();
  h.setHandler(() => poll.promise);
  h.poll();
  h.unmount();
  poll.resolve(stale);
  await settle();
  assert.equal(h.status, applied);
  assert.throws(() => h.poll(), /polling must be mounted/);
});

console.log(`Settings localization tests: ${passed} passed`);
