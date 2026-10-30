import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { execFileSync, spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { checkEntry } from './repository-policy.mjs';
import { checkRepository, outgoingPushTips, treeEntries } from './check-repository.mjs';

const guard = fileURLToPath(new URL('./check-repository.mjs', import.meta.url));

test('original Rust, JavaScript bridge, and synthetic data stay permitted', () => {
  for (const name of ['src/lib.rs', 'web/bridge.js', 'tools/check.mjs', 'tests/fixtures/synthetic.json']) {
    assert.deepEqual(checkEntry(name, Buffer.from('// Original authored test data\n')), []);
  }
});

test('reference directories, disguised known bundles, maps, and binary payloads are rejected', () => {
  for (const name of ['raw/input.txt', 'nested/readable/program.js', 'assets/image.png', 'web/game1107249518.js', 'web/client.js.map']) {
    assert(checkEntry(name, Buffer.from('synthetic placeholder')).length > 0, name);
  }
  const synthetic = Buffer.from('SYNTHETIC forbidden reference payload; not official source');
  const hash = createHash('sha256').update(synthetic).digest('hex');
  assert(checkEntry('web/renamed.js', synthetic, '100644', new Set([hash])).some(error => error.includes('pinned')));
  const map = JSON.stringify({ ['sources' + 'Content']: ['SYNTHETIC source'] });
  assert(checkEntry('web/renamed.json', Buffer.from(map)).some(error => error.includes('source-map')));
  assert(checkEntry('web/renamed.mjs', Buffer.from([65, 0, 66])).some(error => error.includes('binary')));
  assert(checkEntry('web/invalid.mjs', Buffer.from([0xc3, 0x28])).some(error => error.includes('UTF-8')));
});

test('only the self-contained project logo and icon are permitted as vector assets', () => {
  const logo = Buffer.from('<svg xmlns="http://www.w3.org/2000/svg"><path d="M0 0H4"/></svg>');
  for (const name of ['assets/logo.svg', 'assets/icon.svg']) {
    assert.deepEqual(checkEntry(name, logo), []);
    for (const unsafe of [
      '<svg><script>alert(1)</script></svg>',
      '<svg><foreignObject/></svg>',
      '<svg><image href="data:image/png;base64,SYNTHETIC"/></svg>',
      '<svg><use href="https://example.invalid/icon.svg"/></svg>',
      '<svg onload="alert(1)"/>',
      '<svg><style>path { fill: url(https://example.invalid/image); }</style></svg>',
    ]) {
      assert(checkEntry(name, Buffer.from(unsafe)).some(error => error.includes('SVG contains')), name);
    }
  }
  assert(checkEntry('assets/other.svg', logo).length > 0);
});

test('content rules cover the whole repository while hygiene rules cover only the client root', () => {
  const binary = Buffer.from([0x47, 0x49, 0x46, 0x00]);
  assert.deepEqual(checkEntry('assets/banner.gif', binary, '100644', undefined, 'client/'), []);
  assert.deepEqual(checkEntry('tests/fixtures/large.json', Buffer.alloc(600 * 1024, 32), '100644', undefined, 'client/'), []);
  assert(checkEntry('client/assets/banner.gif', binary, '100644', undefined, 'client/').some(error => error.includes('format')));
  assert(checkEntry('research/game1107249518.js', Buffer.from('SYNTHETIC'), '100644', undefined, 'client/').some(error => error.includes('artifact name')));
  assert(checkEntry('research/raw/input.txt', Buffer.from('SYNTHETIC'), '100644', undefined, 'client/').some(error => error.includes('directory')));
  const logo = Buffer.from('<svg xmlns="http://www.w3.org/2000/svg"><path d="M0 0H4"/></svg>');
  assert.deepEqual(checkEntry('client/assets/logo.svg', logo, '100644', undefined, 'client/'), []);
  assert(checkEntry('client/assets/other.svg', logo, '100644', undefined, 'client/').length > 0);
});

async function withRepository(run, objectFormat = 'sha1') {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'slither-client-guard-'));
  const env = { ...process.env, GIT_CONFIG_NOSYSTEM: '1',
    GIT_CONFIG_GLOBAL: path.join(directory, 'no-global-config'),
    GIT_AUTHOR_NAME: 'Synthetic Guard Test', GIT_AUTHOR_EMAIL: 'guard@example.invalid',
    GIT_COMMITTER_NAME: 'Synthetic Guard Test', GIT_COMMITTER_EMAIL: 'guard@example.invalid' };
  const git = (args, input) => execFileSync('git', args, { cwd: directory, stdio: 'pipe', env, input });
  try {
    git(['init', '--quiet', '--initial-branch=main', `--object-format=${objectFormat}`]);
    fs.mkdirSync(path.join(directory, 'web'));
    await run(directory, git, env);
  } finally {
    assert.equal(path.dirname(path.resolve(directory)), path.resolve(os.tmpdir()));
    assert(path.basename(directory).startsWith('slither-client-guard-'));
    fs.rmSync(directory, { recursive: true, force: true });
  }
}

test('an unstaged clean edit cannot conceal a forbidden staged blob', () => withRepository(async (directory, git) => {
  const file = path.join(directory, 'web/bridge.mjs');
  fs.writeFileSync(file, Buffer.from([65, 0, 66]));
  git(['add', 'web/bridge.mjs']);
  fs.writeFileSync(file, '// Original clean working tree\n');
  assert((await checkRepository(directory)).errors.some(error => error.includes('binary')));
}));

test('removing a forbidden file later does not erase it from the published history check', () => withRepository(async (directory, git) => {
  const file = path.join(directory, 'web/bridge.mjs');
  fs.writeFileSync(file, '// Original source\n');
  git(['add', '.']);
  git(['commit', '--quiet', '-m', 'test: original source']);
  fs.writeFileSync(path.join(directory, 'web/program.js.map'), 'SYNTHETIC map placeholder');
  git(['add', '.']);
  git(['commit', '--quiet', '-m', 'test: synthetic prohibited artifact']);
  git(['rm', '--quiet', 'web/program.js.map']);
  git(['commit', '--quiet', '-m', 'test: remove synthetic prohibited artifact']);
  assert.deepEqual((await checkRepository(directory)).errors, []);
  assert((await checkRepository(directory, true)).errors.some(error => error.includes('program.js.map')));
}));

test('pre-push inspects explicitly pushed history even after all local refs stop retaining it', () => withRepository(async (directory, git, env) => {
  fs.writeFileSync(path.join(directory, 'web/bridge.mjs'), '// Original source\n');
  git(['add', '.']);
  git(['commit', '--quiet', '-m', 'test: clean root']);
  const root = git(['rev-parse', 'HEAD']).toString().trim();
  fs.writeFileSync(path.join(directory, 'web/program.js.map'), 'SYNTHETIC prohibited artifact');
  git(['add', '.']);
  git(['commit', '--quiet', '-m', 'test: synthetic prohibited artifact']);
  git(['rm', '--quiet', 'web/program.js.map']);
  git(['commit', '--quiet', '-m', 'test: clean tree retaining forbidden ancestor']);
  const tip = git(['rev-parse', 'HEAD']).toString().trim();
  git(['reset', '--hard', root]);
  assert.deepEqual((await checkRepository(directory, true)).errors, []);
  const input = `refs/heads/candidate ${tip} refs/heads/candidate ${'0'.repeat(40)}\n`;
  assert((await checkRepository(directory, false, outgoingPushTips(input))).errors.some(error => error.includes('program.js.map')));
  const invocation = spawnSync(process.execPath, [guard, '--pre-push'], { cwd: directory, input, encoding: 'utf8', env });
  assert.equal(invocation.status, 1);
  assert(invocation.stderr.includes('program.js.map'));
}));

test('pre-push accepts deletion records and rejects malformed object IDs', () => {
  assert.deepEqual(outgoingPushTips(`(delete) ${'0'.repeat(40)} refs/heads/old ${'a'.repeat(40)}\n`), []);
  assert.throws(() => outgoingPushTips('refs/heads/main --all refs/heads/main nope\n'), /Malformed/);
});

test('pre-push checks outgoing commits, not already-remote or unrelated history', () => withRepository(async (directory, git, env) => {
  fs.writeFileSync(path.join(directory, 'web/bridge.mjs'), '// SYNTHETIC original source\n');
  git(['add', '.']);
  git(['commit', '--quiet', '-m', 'test: synthetic root']);
  const root = git(['rev-parse', 'HEAD']).toString().trim();
  git(['update-ref', 'refs/remotes/origin/main', root]);
  fs.writeFileSync(path.join(directory, 'web/program.js.map'), 'SYNTHETIC prohibited artifact');
  git(['add', '.']);
  git(['commit', '--quiet', '-m', 'test: synthetic outgoing artifact']);
  const forbidden = git(['rev-parse', 'HEAD']).toString().trim();
  const input = tip => `refs/heads/candidate ${tip} refs/heads/candidate ${root}\n`;
  assert((await checkRepository(directory, false, [forbidden])).errors.some(error => error.includes('program.js.map')));
  // Check a different index so the rejection must come from the outgoing tree.
  git(['reset', '--hard', root]);
  const rejected = spawnSync(process.execPath, [guard, '--pre-push'], { cwd: directory, input: input(forbidden), encoding: 'utf8', env });
  assert.equal(rejected.status, 1);
  assert(rejected.stderr.includes('program.js.map'));
  git(['reset', '--hard', forbidden]);
  git(['rm', '--quiet', 'web/program.js.map']);
  git(['commit', '--quiet', '-m', 'test: synthetic remote cleanup']);
  const remote = git(['rev-parse', 'HEAD']).toString().trim();
  git(['update-ref', 'refs/remotes/origin/main', remote]);
  git(['branch', 'unrelated-forbidden', forbidden]);
  fs.writeFileSync(path.join(directory, 'web/bridge.mjs'), '// SYNTHETIC outgoing source\n');
  git(['add', 'web/bridge.mjs']);
  git(['commit', '--quiet', '-m', 'test: synthetic clean outgoing commit']);
  const tip = git(['rev-parse', 'HEAD']).toString().trim();
  assert.deepEqual((await checkRepository(directory, false, [tip])).errors, []);
  const accepted = spawnSync(process.execPath, [guard, '--pre-push'], { cwd: directory, input: input(tip), encoding: 'utf8', env });
  assert.equal(accepted.status, 0, accepted.stderr);
  assert.match(accepted.stdout, /^Repository boundary passed: 1 indexed and historical file versions checked\.\r?\n$/);
  assert((await checkRepository(directory, true)).errors.some(error => error.includes('program.js.map')));
  // Even an up-to-date tip still checks the index.
  fs.writeFileSync(path.join(directory, 'web/staged.js.map'), 'SYNTHETIC staged artifact');
  git(['add', 'web/staged.js.map']);
  assert((await checkRepository(directory, false, [remote])).errors.some(error => error.includes('staged.js.map')));
}));

test('empty and unresolved indexes retain their exact errors', () => withRepository(async (directory, git) => {
  assert.deepEqual(await checkRepository(directory), { count: 0, errors: ['No tracked files to inspect; stage the intended source before checking.'] });
  const object = git(['hash-object', '-w', '--stdin'], 'SYNTHETIC conflict fixture\n').toString().trim();
  git(['update-index', '--index-info'], [1, 2, 3].map(stage => `100644 ${object} ${stage}\tweb/conflict.mjs\n`).join(''));
  assert.deepEqual(await checkRepository(directory), { count: 0, errors: [1, 2, 3].map(stage => `web/conflict.mjs: unresolved index stage ${stage}`) });
}));

test('outgoing tips validate format and commit resolution, including annotated tags', () => withRepository(async (directory, git) => {
  fs.writeFileSync(path.join(directory, 'web/bridge.mjs'), '// SYNTHETIC source\n');
  git(['add', '.']);
  git(['commit', '--quiet', '-m', 'test: synthetic root']);
  git(['tag', '-a', 'synthetic-tag', '-m', 'SYNTHETIC annotated tag']);
  const tag = git(['rev-parse', 'synthetic-tag']).toString().trim();
  assert.deepEqual((await checkRepository(directory, false, [tag])).errors, []);
  for (const tip of ['--all', 'a'.repeat(39), 'a'.repeat(65)]) {
    await assert.rejects(checkRepository(directory, false, [tip]), /Invalid outgoing object ID/);
  }
  const blob = git(['rev-parse', 'HEAD:web/bridge.mjs']).toString().trim();
  for (const tip of [blob, 'a'.repeat(40), 'a'.repeat(64)]) {
    await assert.rejects(checkRepository(directory, false, [tip]));
  }
}));

test('batch framing preserves empty blobs and binary data across pipe chunks', () => withRepository(async (directory, git) => {
  fs.writeFileSync(path.join(directory, 'web/empty.mjs'), '');
  fs.writeFileSync(path.join(directory, 'web/large.bin'), Buffer.concat([
    Buffer.from('SYNTHETIC binary batch-framing fixture\n'), Buffer.alloc(128 * 1024),
    Buffer.from('\n000000 blob 99\nSYNTHETIC text resembling a batch header\n'),
  ]));
  fs.writeFileSync(path.join(directory, 'web/last.mjs'), '// SYNTHETIC final blob\n');
  git(['add', '.']);
  git(['commit', '--quiet', '-m', 'test: synthetic batch framing']);
  assert.deepEqual(await checkRepository(directory, true, [], 'client/'), { count: 3, errors: [] });
}));

test('SHA-256 tips and tree object IDs retain their full width', () => withRepository(async (directory, git) => {
  fs.writeFileSync(path.join(directory, 'web/bridge.mjs'), '// SYNTHETIC SHA-256 fixture\n');
  git(['add', '.']);
  git(['commit', '--quiet', '-m', 'test: synthetic SHA-256 root']);
  const tip = git(['rev-parse', 'HEAD']).toString().trim();
  assert.equal(tip.length, 64);
  const tips = outgoingPushTips(`refs/heads/main ${tip} refs/heads/main ${'0'.repeat(64)}\n`);
  assert.deepEqual(await checkRepository(directory, false, tips), { count: 1, errors: [] });
  assert.deepEqual(await checkRepository(directory, true), { count: 1, errors: [] });
}, 'sha256'));

// Build trees with Git plumbing so symlinks, executable bits and unusual paths
// are tested identically on Windows without creating native symlinks.
function syntheticTree(git, entries) {
  const children = new Map();
  const leaves = [];
  for (const entry of entries) {
    const slash = entry.name.indexOf('/');
    if (slash < 0) leaves.push(entry);
    else {
      const name = entry.name.slice(0, slash);
      if (!children.has(name)) children.set(name, []);
      children.get(name).push({ ...entry, name: entry.name.slice(slash + 1) });
    }
  }
  for (const [name, nested] of children) leaves.push({ mode: '040000', object: syntheticTree(git, nested), name });
  return git(['mktree', '-z'], leaves.map(({ mode, object, name }) => `${mode} ${mode === '040000' ? 'tree' : mode === '160000' ? 'commit' : 'blob'} ${object}\t${name}\0`).join('')).toString().trim();
}

test('batched tree enumeration equals the full per-commit ls-tree audit', () => withRepository(async (directory, git) => {
  const blob = text => git(['hash-object', '-w', '--stdin'], `SYNTHETIC ${text}\n`).toString().trim();
  const original = blob('original');
  const changed = blob('changed');
  const evil = blob('evil merge resolution, different from both parents');
  const link = blob('link target');
  const commit = (entries, parents = []) => git(['commit-tree', syntheticTree(git, entries), ...parents.flatMap(parent => ['-p', parent])], 'test: SYNTHETIC tree fixture\n').toString().trim();
  const base = [{ mode: '100644', object: original, name: 'nested/original.mjs' }];
  const root = commit(base);
  const left = commit([{ ...base[0], object: changed }, { mode: '100644', object: original, name: 'nested/copy.mjs' }], [root]);
  const rightEntries = [{ ...base[0], name: 'renamed/deep/source.mjs' },
    { ...base[0], name: 'copied/deep/source.mjs' },
    { mode: '100755', object: original, name: 'executable.mjs' }];
  const right = commit(rightEntries, [root]);
  const merged = [...rightEntries,
    { mode: '100644', object: original, name: 'nested/copy.mjs' },
    { mode: '100644', object: changed, name: 'nested/original.mjs' }];
  const merge = commit(merged, [left, right]);
  const evilEntries = merged.map(entry => entry.name === 'nested/original.mjs' ? { ...entry, object: evil } : entry);
  const evilMerge = commit(evilEntries, [merge, right]);
  const final = commit([
    ...evilEntries.filter(entry => entry.name !== 'renamed/deep/source.mjs'),
    { mode: '100755', object: original, name: 'renamed/deep/source.mjs' },
    { mode: '120000', object: link, name: 'nested/link' },
    { mode: '160000', object: root, name: 'nested/submodule' },
    { mode: '100644', object: original, name: 'nested/tab\tand\nnewline.mjs' },
  ], [evilMerge]);
  git(['update-ref', 'refs/heads/main', final]);
  const remoteOnly = commit([{ mode: '100644', object: original, name: 'remote-only.mjs' }], [root]);
  const tagOnly = commit([{ mode: '100644', object: original, name: 'tag-only.mjs' }], [root]);
  git(['update-ref', 'refs/remotes/origin/side', remoteOnly]);
  git(['update-ref', 'refs/tags/synthetic-right', tagOnly]);
  const hidden = commit([{ mode: '100644', object: original, name: 'hidden.js.map' }]);
  git(['update-ref', 'refs/t3/private-fixture', hidden]);
  // Windows cannot index the historical tab/newline path; keep a portable index.
  git(['read-tree', root]);
  const commits = git(['rev-list', '--branches', '--remotes', '--tags']).toString().trim().split('\n');
  const expected = new Set();
  for (const revision of commits) {
    for (const record of git(['ls-tree', '-r', '-z', '--full-tree', revision]).toString().split('\0').filter(Boolean)) {
      const tab = record.indexOf('\t');
      const [mode, , object] = record.slice(0, tab).split(' ');
      expected.add(`${mode}\0${object}\0${record.slice(tab + 1)}`);
    }
  }
  const trees = git(['rev-list', '--format=%T', '--no-commit-header', '--branches', '--remotes', '--tags']).toString().trim().split('\n');
  const actual = new Set();
  for await (const { mode, object, name } of treeEntries(trees, object => git(['cat-file', 'tree', object]))) actual.add(`${mode}\0${object}\0${name}`);
  assert.deepEqual([...actual].sort(), [...expected].sort());
  assert(actual.has(`100644\0${original}\0renamed/deep/source.mjs`));
  assert(actual.has(`100755\0${original}\0renamed/deep/source.mjs`));
  assert(actual.has(`100644\0${evil}\0nested/original.mjs`));
  const expectedErrors = new Set();
  let count = 0;
  for (const identity of expected) {
    const [mode, object, name] = identity.split('\0');
    if (mode === '160000') expectedErrors.add(`${name}: submodules require explicit policy review`);
    else {
      count++;
      for (const error of checkEntry(name, git(['cat-file', 'blob', object]), mode)) expectedErrors.add(error);
    }
  }
  const result = await checkRepository(directory, true);
  assert.equal(result.count, count);
  assert.deepEqual(result.errors.sort(), [...expectedErrors].sort());
  assert(result.errors.includes('nested/link: symlinks and submodules require explicit policy review'));
  assert(result.errors.includes('nested/submodule: submodules require explicit policy review'));
}));

test('Git process count stays bounded as commits, blobs and outgoing tips grow', () => withRepository(async (directory, git, env) => {
  const run = (label, mode, input = '') => {
    const trace = path.join(directory, `${label}-trace.jsonl`);
    const result = spawnSync(process.execPath, [guard, mode], { cwd: directory, env: { ...env, GIT_TRACE2_EVENT: trace }, input, encoding: 'utf8', timeout: 10_000 });
    assert.equal(result.status, 0, result.stderr || result.error?.message);
    const starts = fs.readFileSync(trace, 'utf8').trim().split('\n').map(line => JSON.parse(line)).filter(event => event.event === 'start');
    assert(starts.length <= 4, `${label}: ${starts.length} Git processes`);
    return starts;
  };
  fs.writeFileSync(path.join(directory, 'web/root.mjs'), '// SYNTHETIC root\n');
  git(['add', 'web/root.mjs']);
  git(['commit', '--quiet', '-m', 'test: synthetic root']);
  const small = run('small', '--history');
  const tips = [];
  for (let index = 0; index < 20; index++) {
    const name = `web/file-${index}.mjs`;
    fs.writeFileSync(path.join(directory, name), `// SYNTHETIC version ${index}\n`);
    git(['add', name]);
    git(['commit', '--quiet', '-m', 'test: synthetic growth']);
    tips.push(git(['rev-parse', 'HEAD']).toString().trim());
  }
  assert.equal(run('large', '--history').length, small.length);
  assert.equal(run('outgoing', '--pre-push', tips.map(tip => `refs/heads/candidate ${tip} refs/heads/candidate ${'0'.repeat(40)}\n`).join('')).length, small.length);
  const deleted = run('deleted', '--pre-push', `(delete) ${'0'.repeat(40)} refs/heads/old ${tips[0]}\n`);
  assert.equal(deleted.length, 3);
  assert(!deleted.some(event => event.argv.includes('rev-list')));
}));
