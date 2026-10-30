import { execFileSync, spawn } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { checkEntry } from './repository-policy.mjs';

function git(directory, args) {
  return execFileSync('git', args, { cwd: directory, maxBuffer: 32 * 1024 * 1024 });
}

function batchReader(directory) {
  const child = spawn('git', ['cat-file', '--batch'], { cwd: directory, stdio: ['pipe', 'pipe', 'pipe'] });
  let stderr = '';
  child.stderr.on('data', bytes => { stderr += bytes; });
  child.stdin.on('error', () => {}); // A failed child is reported by its completion/output.
  const finished = new Promise((resolve, reject) => {
    child.once('error', reject);
    child.once('close', code => code === 0 ? resolve() : reject(new Error(stderr || `git cat-file exited with ${code}`)));
  });
  // Observe failures immediately, including while the caller is inspecting a blob.
  finished.catch(() => {});
  const chunks = child.stdout[Symbol.asyncIterator]();
  let buffer = Buffer.alloc(0);
  let offset = 0;
  async function available() {
    if (offset < buffer.length) return;
    const next = await chunks.next();
    if (next.done) {
      await finished;
      throw new Error('Unexpected end of git cat-file output');
    }
    buffer = next.value;
    offset = 0;
  }
  async function line() {
    const parts = [];
    for (;;) {
      await available();
      const end = buffer.indexOf(10, offset);
      parts.push(buffer.subarray(offset, end < 0 ? buffer.length : end));
      offset = end < 0 ? buffer.length : end + 1;
      if (end >= 0) return Buffer.concat(parts).toString('utf8');
    }
  }
  async function read(size) {
    const bytes = Buffer.allocUnsafe(size);
    for (let used = 0; used < size;) {
      await available();
      const length = Math.min(size - used, buffer.length - offset);
      buffer.copy(bytes, used, offset, offset + length);
      offset += length;
      used += length;
    }
    return bytes;
  }
  return {
    async read(object, type) {
      child.stdin.write(`${object}\n`);
      const header = await line();
      const [, actualType, size] = header.split(' ');
      if (!/^\d+$/.test(size)) throw new Error(`Cannot read ${type} ${object}: ${header}`);
      const bytes = await read(Number(size));
      if ((await read(1))[0] !== 10) throw new Error('Invalid git cat-file object delimiter');
      if (actualType !== type) throw new Error(`Cannot read ${type} ${object}: ${header}`);
      return bytes;
    },
    async close() {
      child.stdin.end();
      // Keep draining through the iterator: flowing mode cannot unblock its active reader.
      while (!(await chunks.next()).done) {}
      await finished;
    },
  };
}

/** Equivalent to recursive ls-tree, skipping only identical trees at the same path. */
export async function* treeEntries(trees, readTree) {
  const visited = new Set();
  async function* walk(object, prefix) {
    const identity = `${object}\0${prefix}`;
    if (visited.has(identity)) return;
    visited.add(identity);
    const bytes = await readTree(object);
    const hashBytes = object.length / 2; // Git supports both SHA-1 and SHA-256 repositories.
    for (let offset = 0; offset < bytes.length;) {
      const space = bytes.indexOf(32, offset);
      const nul = bytes.indexOf(0, space + 1);
      if (space < 0 || nul < 0 || nul + 1 + hashBytes > bytes.length) throw new Error('Invalid Git tree entry');
      if (space === offset) throw new Error('malformed mode in tree entry');
      let rawMode = 0;
      for (let index = offset; index < space; index++) {
        const digit = bytes[index] - 48;
        if (digit < 0 || digit > 7) throw new Error('malformed mode in tree entry');
        // Git's get_mode accumulates in an unsigned int, wrapping at 32 bits.
        rawMode = ((rawMode << 3) | digit) >>> 0;
      }
      const type = rawMode & 0o170000;
      // Git's canon_mode treats every unrecognized type as a gitlink.
      const mode = type === 0o100000
        ? (rawMode & 0o100 ? '100755' : '100644')
        : type === 0o120000 ? '120000' : type === 0o040000 ? '040000' : '160000';
      const name = prefix + bytes.toString('utf8', space + 1, nul);
      const child = bytes.subarray(nul + 1, nul + 1 + hashBytes).toString('hex');
      offset = nul + 1 + hashBytes;
      if (mode === '040000') yield* walk(child, `${name}/`);
      else yield { mode, object: child, name };
    }
  }
  for (const tree of trees) yield* walk(tree, '');
}

/** Read Git blobs rather than the working tree, so unstaged edits cannot mask staged artifacts. */
export async function checkRepository(directory, history = false, outgoingTips = [], hygieneRoot = '') {
  const errors = [];
  const checked = new Set();
  let count = 0;
  let reader;
  const readObject = (object, type) => (reader ??= batchReader(directory)).read(object, type);
  async function inspect(mode, object, name) {
    const identity = `${mode}\0${object}\0${name}`;
    if (checked.has(identity)) return;
    checked.add(identity);
    if (mode === '160000') {
      errors.push(`${name}: submodules require explicit policy review`);
      return;
    }
    const bytes = await readObject(object, 'blob');
    errors.push(...checkEntry(name, bytes, mode, undefined, hygieneRoot));
    count++;
  }
  try {
    const staged = git(directory, ['ls-files', '--stage', '-z']).toString('utf8').split('\0').filter(Boolean);
    for (const entry of staged) {
      const tab = entry.indexOf('\t');
      const [mode, object, stage] = entry.slice(0, tab).split(' ');
      const name = entry.slice(tab + 1);
      if (stage !== '0') errors.push(`${name}: unresolved index stage ${stage}`);
      else await inspect(mode, object, name);
    }
    if (!staged.length) errors.push('No tracked files to inspect; stage the intended source before checking.');
    if (history || outgoingTips.length) {
      // A push can name an old, now-unreferenced commit directly. Include Git's
      // actual outgoing tips, not just the local branches currently retaining it.
      // Tool-private namespaces (for example refs/t3 checkpoints) are never pushed,
      // so history covers only branches, remote-tracking branches and tags.
      for (const tip of outgoingTips) {
        if (!/^[0-9a-f]{40}(?:[0-9a-f]{24})?$/.test(tip)) throw new Error('Invalid outgoing object ID');
        await readObject(`${tip}^{commit}`, 'commit');
      }
      const revisions = history ? ['--branches', '--remotes', '--tags', ...outgoingTips]
        : [...outgoingTips, '--not', '--remotes'];
      const trees = git(directory, ['rev-list', '--format=%T', ...revisions]).toString('utf8').trim().split('\n').filter(line => line && !line.startsWith('commit '));
      for await (const { mode, object, name } of treeEntries(trees, object => readObject(object, 'tree'))) {
        await inspect(mode, object, name);
      }
    }
    return { count, errors: [...new Set(errors)] };
  } finally {
    await reader?.close();
  }
}

export function outgoingPushTips(input) {
  const tips = new Set();
  for (const line of input.split(/\r?\n/).filter(line => line.trim())) {
    const fields = line.trim().split(/\s+/);
    if (fields.length !== 4 || !fields[1].match(/^[0-9a-f]{40}(?:[0-9a-f]{24})?$/) || !fields[3].match(/^[0-9a-f]{40}(?:[0-9a-f]{24})?$/)) {
      throw new Error('Malformed Git pre-push input');
    }
    if (!/^0+$/.test(fields[1])) tips.add(fields[1]);
  }
  return [...tips];
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const args = process.argv.slice(2);
  if (args.some(arg => !['--history', '--pre-push'].includes(arg))) throw new Error('Usage: node client/tools/check-repository.mjs [--history | --pre-push]');
  const history = args.includes('--history');
  const tips = args.includes('--pre-push') ? outgoingPushTips(fs.readFileSync(0, 'utf8')) : [];
  // Check the whole repository from any working directory. Hygiene rules cover only this
  // tool's own project directory when it is nested inside a larger repository.
  const top = git(process.cwd(), ['rev-parse', '--show-toplevel']).toString('utf8').trim();
  const own = path.relative(top, fileURLToPath(new URL('..', import.meta.url)));
  const hygieneRoot = !own || own.startsWith('..') || path.isAbsolute(own) ? '' : `${own.split(path.sep).join('/')}/`;
  const result = await checkRepository(top, history, tips, hygieneRoot);
  if (result.errors.length) {
    console.error(result.errors.join('\n'));
    process.exitCode = 1;
  } else console.log(`Repository boundary passed: ${result.count} indexed${history || args.includes('--pre-push') ? ' and historical' : ''} file versions checked.`);
}
