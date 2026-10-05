import test from 'node:test';
import assert from 'node:assert/strict';
import { generateKeyPairSync, sign, createHash } from 'node:crypto';
import { verifyUpdateSignature, updatePackages } from './update-assets.mjs';

function fixture(prehashed = true) {
  const { publicKey, privateKey } = generateKeyPairSync('ed25519');
  const keyId = Buffer.from('0102030405060708', 'hex');
  const publicBytes = publicKey.export({ format: 'der', type: 'spki' }).subarray(-32);
  const pubkey = Buffer.from(`untrusted comment: test public key\n${Buffer.concat([Buffer.from('Ed'), keyId, publicBytes]).toString('base64')}\n`).toString('base64');
  const bytes = Buffer.from('synthetic update package');
  const payload = prehashed ? createHash('blake2b512').update(bytes).digest() : bytes;
  const signature = sign(null, payload, privateKey);
  const comment = 'timestamp:1234567890 file:fixture';
  const global = sign(null, Buffer.concat([signature, Buffer.from(comment)]), privateKey);
  const signed = Buffer.from(`untrusted comment: fixture\n${Buffer.concat([Buffer.from(prehashed ? 'ED' : 'Ed'), keyId, signature]).toString('base64')}\ntrusted comment: ${comment}\n${global.toString('base64')}\n`).toString('base64');
  return { bytes, pubkey, signed };
}
test('updater signatures verify the payload and authenticated comment in both modes', () => {
  for (const mode of [true, false]) { const f = fixture(mode); assert.doesNotThrow(() => verifyUpdateSignature(f.bytes, f.signed, f.pubkey)); }
});
test('changed payload, wrong key, modified signature and comment are rejected', () => {
  const f = fixture();
  assert.throws(() => verifyUpdateSignature(Buffer.from('changed'), f.signed, f.pubkey));
  assert.throws(() => verifyUpdateSignature(f.bytes, f.signed, fixture().pubkey));
  assert.throws(() => verifyUpdateSignature(f.bytes, 'UNSIGNED-CANDIDATE-NOT-FOR-PUBLICATION', f.pubkey));
  const text = Buffer.from(f.signed, 'base64').toString();
  assert.throws(() => verifyUpdateSignature(f.bytes, Buffer.from(text.replace('file:fixture', 'file:evil')).toString('base64'), f.pubkey));
});
test('installer targets use native formats, including Linux deb', () => {
  assert.deepEqual(Object.keys(updatePackages('4.8.1')), ['darwin-aarch64', 'windows-x86_64', 'linux-x86_64-deb']);
});
