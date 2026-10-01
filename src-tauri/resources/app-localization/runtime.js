/*
 * Antigravity Tools Lite: reversible, allowlisted App UI localization.
 *
 * This file is a JavaScript expression yielding a factory. The caller invokes
 * it with (window, JSON data). Dictionaries are data, never executable scripts.
 * No adapter may be supplied by config. Add a production adapter only after
 * verifying its exact release, static-label ownership and DOM paths against an
 * official App build. A dictionary match alone is never permission to translate.
 *
 * There are currently NO verified production adapters. The synthetic adapter
 * in the offline tests is not an Antigravity compatibility claim.
 */
(function createAppLocalization(host, config) {
  'use strict';

  const REGISTRY_KEY = '__ANTIGRAVITY_TOOLS_LOCALIZATION__';
  const BRAND = 'antigravity-tools-scoped-localization-v1';
  const VERIFIED_ADAPTERS = Object.freeze([]); // OFFLINE_TEST_ADAPTERS_ONLY
  const LEASE_MS = 15000; // Tools must explicitly renew every 3000 ms.
  const TEXT = 'text';
  const ATTRIBUTE_KINDS = new Set(['title', 'aria-label', 'aria-description']);
  const FORBIDDEN_TAGS = new Set([
    'INPUT', 'TEXTAREA', 'SELECT', 'OPTION', 'OUTPUT', 'CODE', 'PRE', 'SCRIPT',
    'STYLE', 'TEMPLATE', 'NOSCRIPT', 'IFRAME', 'OBJECT', 'EMBED', 'CANVAS',
    'SVG', 'MATH', 'VIDEO', 'AUDIO', 'SAMP', 'KBD', 'VAR',
  ]);
  const FORBIDDEN_CLASSES = new Set([
    'monaco-editor', 'view-lines', 'view-line', 'xterm', 'terminal', 'code-block',
    'editor-instance', 'hljs', 'token', 'markdown', 'markdown-body',
    'chat-message', 'message-content', 'user-content', 'file-path',
  ]);
  const FORBIDDEN_ROLES = new Set(['textbox', 'searchbox', 'combobox', 'log']);
  const FORBIDDEN_ATTRIBUTES = [
    'contenteditable', 'data-user-content', 'data-message-id', 'data-chat-message',
    'data-file-path', 'data-code',
  ];
  const OBSERVED_ATTRIBUTES = [
    'data-testid', 'class', 'role', 'translate', ...FORBIDDEN_ATTRIBUTES,
    ...ATTRIBUTE_KINDS,
  ];
  const doc = host && host.document;
  let adapter = null;
  let configError = null;
  let observer = null;
  let pendingTimer = null;
  let leaseTimer = null;
  let disposedReason = null;
  let active = false;
  let disposed = false;
  let api;
  let observedRoot = null;
  let observedRootParent = null;
  let dependencies = new Set();
  const owned = new Map();
  const exact = Object.create(null);
  let appVersion = null;
  let lastFailure = null;

  function ownData(object, key) {
    if (!object || typeof object !== 'object' || Array.isArray(object)) return undefined;
    const property = Object.getOwnPropertyDescriptor(object, key);
    return property && Object.prototype.hasOwnProperty.call(property, 'value')
      ? property.value : undefined;
  }

  function status(code, reason, extra) {
    return Object.assign({
      status: code,
      supported: code === 'supported' || code === 'applied',
      active,
      appVersion,
      adapterId: adapter ? adapter.id : null,
      translated: owned.size,
      reason: reason || null,
    }, extra || {});
  }

  // Read only ordinary own data properties; config accessors cannot run here.
  appVersion = ownData(config, 'appVersion');
  if (typeof appVersion !== 'string' || !appVersion || appVersion.length > 100 ||
      ownData(config, 'locale') !== 'zh-CN') {
    appVersion = typeof appVersion === 'string' ? appVersion : null;
    configError = 'Expected an exact appVersion and locale zh-CN';
  } else {
    adapter = VERIFIED_ADAPTERS.find((item) => item.appVersion === appVersion) || null;
  }

  if (adapter) {
    const dictionary = ownData(config, 'dictionary');
    const inputExact = ownData(dictionary, 'exact');
    for (const field of adapter.fields) {
      const value = ownData(inputExact, field.source);
      if (typeof value !== 'string' || value.length === 0 || value.length > 2000) {
        configError = 'Missing or invalid allowlisted exact dictionary entry';
        break;
      }
      exact[field.source] = value;
    }
  }

  function safeElement(element) {
    if (!element || element.nodeType !== 1) return false;
    // No depth limit: a deeply nested editor/chat descendant stays excluded.
    for (let current = element; current; current = current.parentElement) {
      if (FORBIDDEN_TAGS.has(current.tagName) || current.isContentEditable ||
          FORBIDDEN_ROLES.has(current.getAttribute('role')) ||
          current.getAttribute('translate') === 'no' ||
          FORBIDDEN_ATTRIBUTES.some((name) => current.hasAttribute(name))) return false;
      if (current.classList && [...current.classList].some((name) => FORBIDDEN_CLASSES.has(name))) {
        return false;
      }
    }
    return element.isConnected === true;
  }

  function matches(element, anchor) {
    if (!element || element.nodeType !== 1 || element.tagName !== anchor.tagName) return false;
    if (anchor.testId) {
      if (element.getAttribute('data-testid') !== anchor.testId) return false;
    } else if (!anchor.attributes || typeof anchor.attributes.class !== 'string' || !anchor.attributes.class) {
      // Some official static label children have no test id. Only a literal,
      // source-reviewed class signature is allowed inside the exact root.
      return false;
    }
    return Object.entries(anchor.attributes || {}).every(([name, value]) =>
      element.getAttribute(name) === value);
  }

  function findRoot() {
    // Exactly one pre-reviewed test id, never body text, a TreeWalker or '*'.
    const roots = doc.querySelectorAll('[data-testid="' + adapter.root.testId + '"]');
    return roots.length === 1 && matches(roots[0], adapter.root) && safeElement(roots[0])
      ? roots[0] : null;
  }

  function resolve(root, field, dependents) {
    let current = root;
    for (const anchor of field.path) {
      // Paths are direct child chains, not unconstrained descendant queries.
      const found = [...current.children].filter((child) => matches(child, anchor));
      if (found.length !== 1) return null;
      current = found[0];
      if (dependents) dependents.add(current);
    }
    if (!safeElement(current)) return null;
    if (field.kind === TEXT) {
      if (current.childNodes.length !== 1 || current.firstChild.nodeType !== 3) return null;
      if (dependents) dependents.add(current.firstChild);
      return { element: current, node: current.firstChild, field };
    }
    if (!ATTRIBUTE_KINDS.has(field.kind) || !current.hasAttribute(field.kind)) return null;
    return { element: current, node: current, field };
  }

  function readValue(target) {
    return target.field.kind === TEXT ? target.node.data : target.element.getAttribute(target.field.kind);
  }

  function writeValue(target, value) {
    if (target.field.kind === TEXT) target.node.data = value;
    else target.element.setAttribute(target.field.kind, value);
  }

  function affected(record, mutation) {
    return (mutation.type === 'characterData' && mutation.target === record.node) ||
      (mutation.type === 'attributes' && mutation.target === record.element &&
        mutation.attributeName === record.field.kind) ||
      (mutation.type === 'childList' && mutation.target === record.element && record.field.kind === TEXT);
  }

  function releaseExternalWrites(mutations) {
    // Our writes occur with the observer disconnected. Any queued write to an
    // owned value is external, including an external write of the SAME value.
    for (const [index, record] of owned) {
      if (mutations.some((mutation) => affected(record, mutation))) owned.delete(index);
    }
  }

  function drainExternalWrites() {
    if (observer) releaseExternalWrites(observer.takeRecords());
  }

  function inspect() {
    if (disposed) return { error: status('disposed', 'Controller has been disposed') };
    if (configError) return { error: status('invalid_config', configError) };
    if (!adapter) return { error: status('unsupported_version', 'No verified adapter for this exact App version') };
    if (!doc || typeof doc.querySelectorAll !== 'function' ||
        typeof host.MutationObserver !== 'function' ||
        typeof host.setTimeout !== 'function' || typeof host.clearTimeout !== 'function') {
      return { error: status('runtime_error', 'Required DOM lifecycle APIs are unavailable') };
    }
    const root = findRoot();
    if (!root) return { error: status('unsupported_dom', 'Verified root is missing, ambiguous, or excluded') };
    const nextDependencies = new Set([root]);
    for (let parent = root.parentElement; parent; parent = parent.parentElement) nextDependencies.add(parent);
    const targets = [];
    const uniqueProperties = new Map();
    for (const [index, field] of adapter.fields.entries()) {
      const target = resolve(root, field, nextDependencies);
      if (!target) return { error: status('unsupported_dom', 'Verified static label path is missing or excluded') };
      const properties = uniqueProperties.get(target.node) || new Set();
      if (properties.has(field.kind)) return { error: status('unsupported_dom', 'Adapter contains overlapping fields') };
      properties.add(field.kind);
      uniqueProperties.set(target.node, properties);
      const value = readValue(target);
      const record = owned.get(index);
      const isOurs = record && record.element === target.element && record.node === target.node &&
        value === record.translated;
      if (value !== field.source && !isOurs) {
        return { error: status('unsupported_dom', 'Static label drift; no translation was attempted') };
      }
      targets.push(target);
    }
    return { root, targets, dependencies: nextDependencies };
  }

  function clearTimer() {
    if (pendingTimer !== null) host.clearTimeout(pendingTimer);
    pendingTimer = null;
  }

  function stopObserving() {
    clearTimer();
    if (leaseTimer !== null) host.clearTimeout(leaseTimer);
    leaseTimer = null;
    if (observer) observer.disconnect();
    active = false;
  }

  function restoreOwned() {
    let restored = 0;
    let preserved = 0;
    let currentRoot = null;
    // Undo is tied to the exact root object we changed. A newly added duplicate
    // root must not prevent rollback of the still-owned original UI.
    try {
      if (observedRoot && observedRoot.parentNode === observedRootParent &&
          matches(observedRoot, adapter.root) && safeElement(observedRoot)) currentRoot = observedRoot;
    } catch (_) { /* fail closed */ }
    for (const record of owned.values()) {
      let currentTarget = null;
      try { if (currentRoot === observedRoot) currentTarget = resolve(currentRoot, record.field); } catch (_) { /* fail closed */ }
      // A moved/replaced/excluded target or changed value belongs to the App.
      if (currentTarget && currentTarget.element === record.element && currentTarget.node === record.node &&
          readValue(record) === record.translated) {
        try {
          writeValue(record, record.original);
          restored += 1;
        } catch (_) { preserved += 1; }
      } else preserved += 1;
    }
    owned.clear();
    return { restored, preserved };
  }

  function fail(error) {
    drainExternalWrites();
    stopObserving();
    const restored = restoreOwned();
    dependencies.clear();
    lastFailure = { code: error.status, reason: error.reason };
    return status(error.status, error.reason, restored);
  }

  function watch() {
    if (!observer) {
      observer = new host.MutationObserver((mutations) => {
        if (!active || disposed) return;
        releaseExternalWrites(mutations);
        if (!mutations.some((mutation) => dependencies.has(mutation.target))) return;
        if (pendingTimer === null) {
          pendingTimer = host.setTimeout(() => {
            pendingTimer = null;
            if (active && !disposed) apply();
          }, 0);
        }
      });
    }
    const attributeFilter = [...new Set([...OBSERVED_ATTRIBUTES,
      ...Object.keys(adapter.root.attributes || {}),
      ...adapter.fields.flatMap((field) => field.path.flatMap((anchor) => Object.keys(anchor.attributes || {}))),
    ])];
    observer.observe(observedRoot, {
      subtree: true, childList: true, characterData: true, characterDataOldValue: true,
      attributes: true, attributeOldValue: true, attributeFilter,
    });
    // Ancestor identity/removal can invalidate a formerly safe root. No ancestor
    // subtree text is observed and no listeners, intervals or global scans exist.
    for (let parent = observedRoot.parentElement; parent; parent = parent.parentElement) {
      observer.observe(parent, { childList: true, attributes: true, attributeFilter });
    }
  }

  function probe() {
    drainExternalWrites();
    try {
      const result = inspect();
      return result.error || status('supported', null, { labelCount: result.targets.length });
    } catch (_) {
      return status('runtime_error', 'DOM inspection failed');
    }
  }

  function apply() {
    drainExternalWrites();
    clearTimer();
    try {
      const result = inspect();
      if (result.error) return disposed ? result.error : fail(result.error);
      if (observer) observer.disconnect();
      // Drop references to detached or replaced nodes before taking ownership.
      for (const [index, record] of owned) {
        const target = result.targets[index];
        if (record.node !== target.node || record.element !== target.element) owned.delete(index);
      }
      observedRoot = result.root;
      observedRootParent = result.root.parentNode;
      dependencies = result.dependencies;
      for (const [index, target] of result.targets.entries()) {
        if (owned.has(index)) continue;
        const translated = exact[target.field.source];
        if (translated === target.field.source) continue;
        const record = { ...target, original: target.field.source, translated };
        owned.set(index, record);
        writeValue(target, translated);
      }
      active = true;
      lastFailure = null;
      watch();
      // Only initial apply starts a lease. Observer reapply does not renew it.
      if (leaseTimer === null) renewLease();
      return status('applied', null);
    } catch (_) {
      return fail(status('runtime_error', 'DOM update failed; owned changes were reverted where safe'));
    }
  }

  function renewLease() {
    if (disposed) return status('disposed', disposedReason);
    if (!active) return status('inactive', 'Apply successfully before renewing a lease');
    if (leaseTimer !== null) host.clearTimeout(leaseTimer);
    leaseTimer = host.setTimeout(() => {
      leaseTimer = null;
      dispose('lease_expired');
    }, LEASE_MS);
    return status('applied', null, { leaseMs: LEASE_MS });
  }

  function dispose(reason) {
    drainExternalWrites();
    stopObserving();
    const restored = restoreOwned();
    dependencies.clear();
    observedRoot = null;
    observedRootParent = null;
    observer = null;
    disposed = true;
    disposedReason = reason === 'lease_expired' ? reason : disposedReason;
    if (host && host[REGISTRY_KEY] === api) delete host[REGISTRY_KEY];
    return status('disposed', disposedReason, restored);
  }

  api = Object.freeze({
    brand: BRAND, probe, apply, dispose, renewLease,
    getStatus: () => disposed ? status('disposed', disposedReason) : lastFailure
      ? status(lastFailure.code, lastFailure.reason) : status(active ? 'applied' : 'inactive'),
  });
  const previous = host && host[REGISTRY_KEY];
  if (previous) {
    if (previous.brand !== BRAND || typeof previous.dispose !== 'function') {
      configError = 'An unknown controller already occupies the localization registry';
      return api;
    }
    previous.dispose();
  }
  if (host) host[REGISTRY_KEY] = api;
  return api;
})
