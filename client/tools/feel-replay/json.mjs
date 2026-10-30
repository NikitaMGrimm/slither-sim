// Ordinary JSON; reject values JSON would silently replace or omit.
function finite(value) {
  if (typeof value === 'number' && !Number.isFinite(value)) throw new Error('JSON: finite numbers required');
  if (value === undefined || typeof value === 'bigint' || typeof value === 'function') throw new Error('JSON value required');
  if (value && typeof value === 'object') for (const child of Object.values(value)) finite(child);
  return value;
}
export const stringify = value => JSON.stringify(finite(value), null, 2) + '\n';
export const parse = text => finite(JSON.parse(text));
