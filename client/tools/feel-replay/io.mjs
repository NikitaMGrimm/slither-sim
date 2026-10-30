import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { fail, validateManifest, validateSchedule, validateTrace } from './schema.mjs';

export function floatBits(value) {
  const bytes = Buffer.alloc(8);
  bytes.writeDoubleBE(value);
  return bytes.toString('hex');
}
function encode(value) {
  if (typeof value === 'number') {
    if (!Number.isFinite(value)) fail('encoding', 'finite numbers required');
    return { $f64: floatBits(value) };
  }
  if (Array.isArray(value)) return value.map(encode);
  if (value && typeof value === 'object') {
    if (Object.hasOwn(value, '$f64')) fail('encoding', 'reserved $f64 key');
    return Object.fromEntries(Object.keys(value).sort().map(k => [k, encode(value[k])]));
  }
  if (value === null || typeof value === 'string' || typeof value === 'boolean') return value;
  fail('encoding', 'JSON values required');
}
function decode(value) {
  if (typeof value === 'number') fail('encoding', 'bare JSON number; float bits required');
  if (Array.isArray(value)) return value.map(decode);
  if (value && typeof value === 'object') {
    if (Object.hasOwn(value, '$f64')) {
      if (Object.keys(value).length !== 1 || typeof value.$f64 !== 'string' || !/^[a-f0-9]{16}$/.test(value.$f64)) fail('encoding', 'invalid f64 hex');
      const number = Buffer.from(value.$f64, 'hex').readDoubleBE();
      if (!Number.isFinite(number)) fail('encoding', 'non-finite f64');
      return number;
    }
    return Object.fromEntries(Object.entries(value).map(([k, v]) => [k, decode(v)]));
  }
  return value;
}
// Transform BEFORE JSON.stringify: a replacer after lossy JSON cannot recover -0.
export function stringify(value) { return `${JSON.stringify({ encoding: 'feel-f64-v1', value: encode(value) }, null, 2)}\n`; }
export function parse(text) {
  const wrapper = JSON.parse(text);
  if (!wrapper || wrapper.encoding !== 'feel-f64-v1' || Object.keys(wrapper).sort().join(',') !== 'encoding,value') fail('encoding', 'expected feel-f64-v1 wrapper');
  return decode(wrapper.value);
}
export const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
export const scheduleHash = schedule => sha256(stringify(validateSchedule(schedule)));
export async function buildManifest({ schedule, artifactFiles, inputFile = null, ...fields }) {
  const artifacts = [];
  for (const [name, filename] of Object.entries(artifactFiles).sort(([a], [b]) => a < b ? -1 : a > b ? 1 : 0)) {
    artifacts.push({ name, sha256: sha256(await readFile(filename)) });
  }
  const input = { ...fields.input, sha256: inputFile === null ? fields.input.sha256 : sha256(await readFile(inputFile)) };
  return validateManifest({ ...fields, schema: 1, artifacts, input, scheduleSha256: scheduleHash(schedule) });
}
export async function readSchedule(filename) { return validateSchedule(parse(await readFile(filename, 'utf8'))); }
export async function readTrace(filename, schedule) { return validateTrace(parse(await readFile(filename, 'utf8')), schedule); }
