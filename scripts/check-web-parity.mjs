#!/usr/bin/env node
/**
 * Web 对齐度防回归检查（无 WebView 服务版）
 *
 * 交叉对照三方数据，保证"前端实际用到的每个命令在 Web 模式都可用"：
 *   1. src/**                      —— 前端实际以字符串字面量调用的命令
 *   2. src/utils/request.ts        —— COMMAND_MAPPING（命令 → HTTP 映射）+ DESKTOP_ONLY 白名单
 *   3. src-tauri/src/server.rs     —— 后端 /api 路由全集
 *
 * 失败条件（exit 1）：
 *   A. UNMAPPED：前端实际调用的命令既无 COMMAND_MAPPING 又不在 DESKTOP_ONLY 白名单
 *   B. DEAD ROUTE：COMMAND_MAPPING 指向的 URL 在 server.rs 中不存在对应路由
 */
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';

const ROOT = join(import.meta.dirname, '..');

function read(path) {
  return readFileSync(join(ROOT, path), 'utf8');
}

function walk(dir, exts, acc = []) {
  for (const name of readdirSync(dir)) {
    const full = join(dir, name);
    if (name === 'node_modules' || name === 'dist') continue;
    const st = statSync(full);
    if (st.isDirectory()) walk(full, exts, acc);
    else if (exts.some((e) => name.endsWith(e))) acc.push(full);
  }
  return acc;
}

// 1. 前端实际调用（命令名以字符串字面量出现于 src/**）
const srcFiles = walk(join(ROOT, 'src'), ['.ts', '.tsx']);
const usedByFrontend = new Set();
const CALL_SITE = /(?:\brequest|\binvoke)\s*(?:<[^>()]*>)?\(\s*['"`]([a-zA-Z0-9_]+)['"`]/g;
for (const file of srcFiles) {
  const text = readFileSync(file, 'utf8');
  for (const m of text.matchAll(CALL_SITE)) usedByFrontend.add(m[1]);
}

// 2. request.ts：COMMAND_MAPPING 与 DESKTOP_ONLY 白名单（作为唯一事实来源解析，不重复维护）
const requestTs = read('src/utils/request.ts');
const mapBlock = requestTs.match(/const COMMAND_MAPPING[^=]*=\s*\{([\s\S]*?)\n\};/);
if (!mapBlock) {
  console.error('FATAL: cannot locate COMMAND_MAPPING in src/utils/request.ts');
  process.exit(2);
}
const commandMapping = new Map();
for (const m of mapBlock[1].matchAll(/'([^']+)':\s*\{\s*url:\s*'([^']+)'/g)) {
  commandMapping.set(m[1], m[2]);
}
const desktopOnlyBlock = requestTs.match(/const DESKTOP_ONLY = new Set\(\[([\s\S]*?)\]\);/);
if (!desktopOnlyBlock) {
  console.error('FATAL: cannot locate DESKTOP_ONLY in src/utils/request.ts');
  process.exit(2);
}
const desktopOnly = new Set(
  [...desktopOnlyBlock[1].matchAll(/'([^']+)'/g)].map((m) => m[1]),
);

// 3. server.rs 全部 .route("...")（/api 前缀已含在路径里）
const serverRs = read('src-tauri/src/server.rs');
const apiRoutes = new Set();
for (const m of serverRs.matchAll(/\.route\(\s*"([^"]+)"/g)) {
  apiRoutes.add(m[1].replace(/:[^/"]+/g, ':*'));
}

const toBackendPath = (url) => url.replace(/:[^/]+/g, ':*');

const failures = [];
const desktopOnlyHits = [];

for (const cmd of [...usedByFrontend].sort()) {
  const mapped = commandMapping.has(cmd);
  if (!mapped && !desktopOnly.has(cmd)) continue; // 纯前端内部字符串（误报容忍）
  if (desktopOnly.has(cmd)) {
    desktopOnlyHits.push(cmd);
    continue;
  }
  if (!mapped) {
    failures.push(`UNMAPPED: "${cmd}" is invoked by the frontend but has no COMMAND_MAPPING entry (web mode throws "not supported in Web mode")`);
  }
}

for (const [cmd, url] of [...commandMapping].sort()) {
  if (url.startsWith('/api/') && !apiRoutes.has(toBackendPath(url))) {
    failures.push(`DEAD ROUTE: "${cmd}" maps to ${url} but no matching route exists in src-tauri/src/server.rs`);
  }
}

// 汇总输出
console.log(`Frontend-invoked commands: ${usedByFrontend.size}`);
console.log(`Web mappings             : ${commandMapping.size}`);
console.log(`Backend /api routes      : ${apiRoutes.size}`);
console.log(`Desktop-only (whitelist) : ${desktopOnlyHits.length}`);
console.log('');

const unmapped = [...commandMapping.keys()].filter((c) => !usedByFrontend.has(c));
if (unmapped.length) {
  console.log(`[info] ${unmapped.length} mapped command(s) currently unused by frontend (web-only/backend-reserved):`);
  for (const c of unmapped) console.log(`  - ${c}`);
  console.log('');
}

if (failures.length) {
  console.error(`[FAIL] ${failures.length} web-parity problem(s):`);
  for (const f of failures) console.error(`  - ${f}`);
  process.exit(1);
}

console.log('[ok] web parity check passed.');
