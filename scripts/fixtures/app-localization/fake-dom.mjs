/* Minimal offline DOM/MutationObserver fixture. It never opens an App or port. */
export function createFixtureHost() {
  const observers = new Set();
  const timers = new Map();
  let nextTimer = 0;
  let now = 0;
  let document;
  const host = { metrics: { observerInstances: 0, writes: 0, queries: [] } };

  class Node {
    constructor(nodeType) { this.nodeType = nodeType; this.parentNode = null; this.childNodes = []; }
    get parentElement() { return this.parentNode?.nodeType === 1 ? this.parentNode : null; }
    get isConnected() {
      let current = this;
      while (current?.parentNode) current = current.parentNode;
      return current === document;
    }
    get firstChild() { return this.childNodes[0] || null; }
    get children() { return this.childNodes.filter((child) => child.nodeType === 1); }
    contains(node) {
      for (let current = node; current; current = current.parentNode) if (current === this) return true;
      return false;
    }
    appendChild(node) {
      if (node.parentNode) node.parentNode.removeChild(node);
      this.childNodes.push(node);
      node.parentNode = this;
      notify({ type: 'childList', target: this, addedNodes: [node], removedNodes: [] });
      return node;
    }
    removeChild(node) {
      const index = this.childNodes.indexOf(node);
      if (index < 0) throw new Error('Missing child');
      this.childNodes.splice(index, 1);
      node.parentNode = null;
      notify({ type: 'childList', target: this, addedNodes: [], removedNodes: [node] });
      return node;
    }
    replaceChildren(...nodes) {
      for (const node of [...this.childNodes]) this.removeChild(node);
      for (const node of nodes) this.appendChild(node);
    }
    get textContent() {
      throw new Error('Runtime must not scan element textContent');
    }
    set textContent(_) {
      throw new Error('Runtime must not replace element textContent');
    }
  }

  class Text extends Node {
    constructor(data) { super(3); this._data = data; }
    get data() { return this._data; }
    set data(value) {
      const oldValue = this._data;
      this._data = value;
      host.metrics.writes += 1;
      notify({ type: 'characterData', target: this, oldValue });
    }
  }

  class Element extends Node {
    constructor(tagName) { super(1); this.tagName = tagName.toUpperCase(); this.attributes = new Map(); }
    getAttribute(name) { return this.attributes.get(name) ?? null; }
    hasAttribute(name) { return this.attributes.has(name); }
    setAttribute(name, value) {
      const oldValue = this.getAttribute(name);
      this.attributes.set(name, String(value));
      host.metrics.writes += 1;
      notify({ type: 'attributes', target: this, attributeName: name, oldValue });
    }
    removeAttribute(name) {
      const oldValue = this.getAttribute(name);
      if (!this.attributes.delete(name)) return;
      host.metrics.writes += 1;
      notify({ type: 'attributes', target: this, attributeName: name, oldValue });
    }
    get classList() { return (this.getAttribute('class') || '').split(/\s+/).filter(Boolean); }
    get isContentEditable() { return this.getAttribute('contenteditable') === 'true'; }
  }

  class Document extends Node {
    constructor() { super(9); }
    createElement(tagName) { return new Element(tagName); }
    createTextNode(data) { return new Text(data); }
    querySelectorAll(selector) {
      host.metrics.queries.push(selector);
      const match = /^\[data-testid="([a-zA-Z0-9_-]+)"\]$/.exec(selector);
      if (!match) throw new Error('Unapproved global selector: ' + selector);
      const found = [];
      const visit = (node) => {
        if (node.nodeType === 1 && node.getAttribute('data-testid') === match[1]) found.push(node);
        for (const child of node.childNodes) visit(child);
      };
      visit(this);
      return found;
    }
  }

  class MutationObserver {
    constructor(callback) {
      this.callback = callback;
      this.targets = new Map();
      this.records = [];
      observers.add(this);
      host.metrics.observerInstances += 1;
    }
    observe(target, options) { this.targets.set(target, options); }
    disconnect() { this.targets.clear(); this.records = []; }
    takeRecords() { const records = this.records; this.records = []; return records; }
  }

  function notify(record) {
    for (const observer of observers) {
      for (const [target, options] of observer.targets) {
        if (record.target !== target && !(options.subtree && target.contains(record.target))) continue;
        if (!options[record.type]) continue;
        if (record.type === 'attributes' && options.attributeFilter &&
            !options.attributeFilter.includes(record.attributeName)) continue;
        observer.records.push(record);
        break;
      }
    }
  }

  document = new Document();
  document.documentElement = document.appendChild(new Element('html'));
  document.body = document.documentElement.appendChild(new Element('body'));
  Object.assign(host, {
    document,
    MutationObserver,
    setTimeout(callback, delay = 0) { const id = ++nextTimer; timers.set(id, { callback, due: now + delay }); return id; },
    clearTimeout(id) { timers.delete(id); },
    flushObservers() {
      for (const observer of observers) {
        const records = observer.takeRecords();
        if (records.length) observer.callback(records);
      }
    },
    flushTimers() {
      const queued = [...timers.entries()].filter(([, timer]) => timer.due <= now);
      for (const [id, timer] of queued) { timers.delete(id); timer.callback(); }
    },
    flush() { host.flushObservers(); host.flushTimers(); host.flushObservers(); },
    advanceTime(milliseconds) { now += milliseconds; host.flush(); },
    element(tagName, testId, text) {
      const element = document.createElement(tagName);
      if (testId) element.setAttribute('data-testid', testId);
      if (text !== undefined) element.appendChild(document.createTextNode(text));
      return element;
    },
  });
  Object.defineProperties(host, {
    pendingTimers: { get: () => timers.size },
    pendingMutationTimers: { get: () => [...timers.values()].filter((timer) => timer.due <= now).length },
    activeObservers: { get: () => [...observers].filter((observer) => observer.targets.size > 0).length },
  });
  return host;
}

// Synthetic test ids deliberately do not imply compatibility with any App.
export const syntheticAdapter = {
  id: 'offline-synthetic-fixture',
  appVersion: 'offline-fixture-1',
  root: { testId: 'offline-fixture-shell', tagName: 'DIV', attributes: { 'data-build-id': 'offline-v1' } },
  fields: [
    { path: [{ testId: 'offline-toolbar', tagName: 'NAV' }, { testId: 'offline-settings', tagName: 'BUTTON' }], kind: 'text', source: 'Settings' },
    { path: [{ testId: 'offline-toolbar', tagName: 'NAV' }, { testId: 'offline-settings', tagName: 'BUTTON' }], kind: 'title', source: 'Settings' },
    { path: [{ testId: 'offline-toolbar', tagName: 'NAV' }, { testId: 'offline-help', tagName: 'BUTTON' }], kind: 'aria-label', source: 'Help' },
  ],
};

export function makeFixture() {
  const host = createFixtureHost();
  const root = host.element('div', 'offline-fixture-shell');
  root.setAttribute('data-build-id', 'offline-v1');
  host.document.body.appendChild(root);
  const toolbar = root.appendChild(host.element('nav', 'offline-toolbar'));
  const settings = toolbar.appendChild(host.element('button', 'offline-settings', 'Settings'));
  settings.setAttribute('title', 'Settings');
  settings.setAttribute('disabled', '');
  const help = toolbar.appendChild(host.element('button', 'offline-help', '?'));
  help.setAttribute('aria-label', 'Help');
  const chat = root.appendChild(host.element('section', 'offline-untrusted-content'));
  const config = { appVersion: syntheticAdapter.appVersion, locale: 'zh-CN', dictionary: { exact: { Settings: '设置', Help: '帮助' } } };
  host.metrics.writes = 0;
  return { host, root, toolbar, settings, help, chat, config };
}
