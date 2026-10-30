import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { validateManifest, validateSchedule, validateTrace } from './schema.mjs';

import { parse, stringify } from './json.mjs';
export { parse, stringify } from './json.mjs';
export const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
export const scheduleHash = schedule => sha256(stringify(validateSchedule(schedule)));
export async function buildManifest({ schedule, artifactFiles, inputFile = null, ...fields }) {
  const artifacts = [];
  for (const [name, filename] of Object.entries(artifactFiles).sort(([a], [b]) => a < b ? -1 : a > b ? 1 : 0)) {
    artifacts.push({ name, sha256: sha256(await readFile(filename)) });
  }
  const input = { ...fields.input, sha256: inputFile === null ? fields.input.sha256 : sha256(await readFile(inputFile)) };
  return validateManifest({ ...fields, schema: 2, artifacts, input, scheduleSha256: scheduleHash(schedule) });
}
export async function readSchedule(filename) { return validateSchedule(parse(await readFile(filename, 'utf8'))); }
export async function readTrace(filename, schedule) { return validateTrace(parse(await readFile(filename, 'utf8')), schedule); }
