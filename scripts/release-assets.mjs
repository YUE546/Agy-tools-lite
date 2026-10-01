// Validate release inputs and the complete artifact set without network access.
import { createHash } from 'node:crypto';
import { readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { renderCask } from './generate-homebrew.mjs';

export function releaseVersion(tag) {
  if (!/^v(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(tag ?? '')) throw new Error('Release tag must be vMAJOR.MINOR.PATCH');
  const version = tag.slice(1);
  if (!version.split('.').every(n => Number.isSafeInteger(Number(n)))) throw new Error('Invalid version number');
  return version;
}

export function validateSource(root, tag) {
  const version = releaseVersion(tag);
  const json = name => JSON.parse(readFileSync(join(root, name), 'utf8'));
  const lock = json('package-lock.json');
  const cargo = readFileSync(join(root, 'src-tauri/Cargo.toml'), 'utf8').split(/^\[/m).find(s => s.startsWith('package]'));
  const cargoLock = readFileSync(join(root, 'src-tauri/Cargo.lock'), 'utf8').split('[[package]]').find(s => /^name = "antigravity-tools"$/m.test(s));
  const versions = [json('package.json').version, lock.version, lock.packages?.['']?.version,
    json('src-tauri/tauri.conf.json').version, cargo?.match(/^version = "([^"]+)"$/m)?.[1], cargoLock?.match(/^version = "([^"]+)"$/m)?.[1]];
  if (versions.some(v => v !== version)) throw new Error(`Tag ${tag} does not match all package, Tauri and Cargo versions`);
  return version;
}

export function packageNames(version) {
  releaseVersion(`v${version}`);
  return [`Antigravity-Tools-Lite-${version}-macos-arm64.zip`,
    `Antigravity-Tools-Lite-${version}-windows-x64-setup.exe`,
    `Antigravity-Tools-Lite-${version}-linux-amd64.deb`];
}

export function sha256(file) { return createHash('sha256').update(readFileSync(file)).digest('hex'); }

export function writeChecksum(file) {
  const name = file.replaceAll('\\', '/').split('/').at(-1);
  writeFileSync(`${file}.sha256`, `${sha256(file)}  ${name}\n`);
}

export function verifyAssets(directory, { tag, repository }) {
  const version = releaseVersion(tag);
  if (!/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(repository ?? '')) throw new Error('Invalid repository');
  const packages = packageNames(version);
  const expected = [...packages, ...packages.map(p => `${p}.sha256`), 'antigravity-tools-lite.rb'].sort();
  const actual = readdirSync(directory).filter(name => name !== 'release-manifest.json').sort();
  if (JSON.stringify(actual) !== JSON.stringify(expected)) throw new Error('Release must contain exactly all three packages, their checksums, and the generated cask');
  for (const name of packages) {
    const digest = sha256(join(directory, name));
    if (readFileSync(join(directory, `${name}.sha256`), 'utf8') !== `${digest}  ${name}\n`) throw new Error(`Checksum mismatch: ${name}`);
    if (readFileSync(join(directory, name)).length === 0) throw new Error(`Empty package: ${name}`);
  }
  const template = readFileSync(new URL('../packaging/homebrew/antigravity-tools-lite.rb.in', import.meta.url), 'utf8');
  const cask = renderCask({ archive: join(directory, packages[0]), version, template,
    url: `https://github.com/${repository}/releases/download/${tag}/${packages[0]}` });
  if (readFileSync(join(directory, 'antigravity-tools-lite.rb'), 'utf8') !== cask) throw new Error('Cask differs from the exact release ZIP, version, URL or template');
  return expected;
}

export function writeManifest(directory, context) {
  if (!/^[0-9a-f]{40}$/.test(context.commit ?? '')) throw new Error('A full source commit SHA is required');
  const files = verifyAssets(directory, context).map(name => ({ name, sha256: sha256(join(directory, name)) }));
  writeFileSync(join(directory, 'release-manifest.json'), `${JSON.stringify({ schema_version: 1,
    repository: context.repository, tag: context.tag, source_commit: context.commit, files }, null, 2)}\n`);
  return [...files.map(f => f.name), 'release-manifest.json'];
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const [command, ...args] = process.argv.slice(2);
    if (command === 'source' && args.length === 1) console.log(validateSource(process.cwd(), args[0]));
    else if (command === 'checksum' && args.length === 1) writeChecksum(args[0]);
    else if (command === 'verify' && args.length === 4) {
      writeManifest(args[0], { tag: args[1], commit: args[2], repository: args[3] });
      console.log('Verified all three packages, checksums, generated cask and source manifest; no publication performed');
    } else throw new Error('Usage: release-assets.mjs source TAG | checksum FILE | verify DIRECTORY TAG COMMIT OWNER/REPOSITORY');
  } catch (error) { console.error(error.message); process.exitCode = 1; }
}
