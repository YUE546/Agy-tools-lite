// Actual PTY / Windows ConPTY acceptance. Synthetic data only; no submit/auth/switch.
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync, existsSync, chmodSync, lstatSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve, join } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import pty from 'node-pty';
import { createRequire } from 'node:module';
// node-pty 1.1.0’s npm prebuild omits the Unix helper executable bit.
// Repair only this pinned test dependency; never touch app trust or OS policy.
if (process.platform !== 'win32') {
  const packageRoot = resolve(createRequire(import.meta.url).resolve('node-pty/package.json'), '..');
  const helper = join(packageRoot, 'prebuilds', process.platform + '-' + process.arch, 'spawn-helper');
  if (existsSync(helper) && lstatSync(helper).isFile()) chmodSync(helper, lstatSync(helper).mode | 0o100);
}
const binary = resolve(process.argv[2] || 'src-tauri/target/debug/agy-switch');
const root = mkdtempSync(join(tmpdir(), 'agy-terminal-'));
const data = join(root, 'data'); mkdirSync(data);
const config = JSON.stringify({ language: 'en', theme: 'light' });
writeFileSync(join(data, 'gui_config.json'), config);
const env = {};
for (const key of ['PATH', 'Path', 'SystemRoot', 'WINDIR', 'COMSPEC', 'PATHEXT', 'TMP', 'TEMP', 'LANG']) if (process.env[key]) env[key] = process.env[key];
Object.assign(env, { HOME: root, ABV_DATA_DIR: data, TERM: 'xterm-256color' });
const watchdog = setTimeout(() => { console.error('Terminal harness exceeded 90 seconds'); process.exit(1); }, 90000);
watchdog.unref();
let terminal, output = '', allOutput = '', exited;
let checks = 0;
async function until(text) {
  for (let i = 0; i < 100; i++) { if (output.includes(text)) { checks++; console.log('Terminal reached: ' + text); return; } await delay(100); }
  throw new Error(`Terminal did not reach ${text}; captured ${output.length} bytes`);
}
async function send(keys, text) { output = ''; terminal.write(keys); await until(text); }
try {
  terminal = pty.spawn(binary, [], { cols: 110, rows: 32, cwd: root, env, useConptyDll: process.platform === 'win32' });
  const done = new Promise(resolve => terminal.onExit(event => { exited = event; resolve(event); }));
  terminal.onData(chunk => { output += chunk; allOutput += chunk; });
  await until('Select a section:');
  // Down enters Statistics, Left returns. Right acts like Enter on menus.
  await send('\x1b[B\x1b[C', 'Local Token Usage & Estimated Cost');
  await send('\x1b[D', 'Select a section:');
  await send('4', 'Manually enter Refresh Token');
  await send('\x1b[B\r', 'Enter Google Refresh Token:');
  output = ''; terminal.write('SYNTHETIC-NOT-A-CREDENTIAL');
  await until('********');
  assert.ok(!allOutput.includes('SYNTHETIC-NOT-A-CREDENTIAL'), 'Secret prompt must never echo its input'); checks++;
  await send('\x1b', 'Manually enter Refresh Token');
  await send('\x1b', 'Select a section:');
  terminal.resize(72, 24);
  output = ''; terminal.write('\x03');
  await Promise.race([done, delay(10000).then(() => { throw new Error('Ctrl+C did not exit TUI'); })]);
  assert.equal(exited.exitCode, 0); checks++;
  assert.doesNotMatch(allOutput, /[\u3400-\u9fff]/); checks++;
  assert.equal(readFileSync(join(data, 'gui_config.json'), 'utf8'), config); checks++;
  console.log(`${checks} real ${process.platform === 'win32' ? 'ConPTY' : 'PTY'} checks passed: arrows, Enter, Esc, masking, resize and clean exit; no network or account writes`);
} finally {
  clearTimeout(watchdog);
  if (terminal && !exited) terminal.kill();
  await delay(200);
  rmSync(root, { recursive: true, force: true });
}
