// A1 encoding, shared by Node and browser peers. Encode raw Numbers before JSON.
import { fail } from './schema.mjs';
export function floatBits(value) {
  const bytes = new DataView(new ArrayBuffer(8));
  bytes.setFloat64(0, value);
  return Array.from({ length: 8 }, (_, i) => bytes.getUint8(i).toString(16).padStart(2, '0')).join('');
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
      const bytes = new DataView(new ArrayBuffer(8));
      for (let i = 0; i < 8; i++) bytes.setUint8(i, Number.parseInt(value.$f64.slice(i * 2, i * 2 + 2), 16));
      const number = bytes.getFloat64(0);
      if (!Number.isFinite(number)) fail('encoding', 'non-finite f64');
      return number;
    }
    return Object.fromEntries(Object.entries(value).map(([k, v]) => [k, decode(v)]));
  }
  return value;
}
export function stringify(value) { return `${JSON.stringify({ encoding: 'feel-f64-v1', value: encode(value) }, null, 2)}\n`; }
export function parse(text) {
  const wrapper = JSON.parse(text);
  if (!wrapper || wrapper.encoding !== 'feel-f64-v1' || Object.keys(wrapper).sort().join(',') !== 'encoding,value') fail('encoding', 'expected feel-f64-v1 wrapper');
  return decode(wrapper.value);
}
