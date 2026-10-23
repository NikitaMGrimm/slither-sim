const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const publish = require('./pr_report_comment.cjs');

const head = 'a'.repeat(40), base = 'b'.repeat(40), merge = 'c'.repeat(40);
const marker = '<!-- slither-report-preview -->';
const bot = { login: 'github-actions[bot]', type: 'Bot' };
function fixture(t, { previous = [], liveHead = head, liveBase = base, snapshotBase = liveBase,
                      targetHistory = 'diverged', reportOverride = {}, runOverride = {},
                      fork = false, liveRepo, parents, artifacts } = {}) {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'slither-comment-test-'));
  t.after(() => fs.rmSync(directory, { recursive: true }));
  const report = { schema: 1, repository: 'owner/repo', pr: 27, head, base, merge, run: 100, attempt: 1, ...reportOverride };
  fs.writeFileSync(path.join(directory, 'report.json'), JSON.stringify(report));
  fs.writeFileSync(path.join(directory, 'report.md'), `${marker}\n## Comparison\nMeasured report\n<details><summary>Details</summary>\n\n</details>`);
  const calls = [];
  const run = { id: 100, run_attempt: 1, event: 'pull_request', status: 'completed', conclusion: 'success',
    path: '.github/workflows/pr-report.yml', workflow_id: 4, repository: { full_name: 'owner/repo' },
    head_sha: head, head_branch: 'any-branch', head_repository: { id: fork ? 2 : 1 },
    pull_requests: fork ? [] : [{ number: 27 }], ...runOverride };
  const context = { repo: { owner: 'owner', repo: 'repo' }, eventName: 'workflow_run',
    payload: { workflow_run: { id: 100, run_attempt: 1, head_sha: head } } };
  const outputs = {};
  const github = { paginate: async method => method === github.rest.issues.listComments ? previous
    : artifacts || [{ id: 5, name: 'pr-report-comment', expired: false, size_in_bytes: 1000 }], rest: {
    actions: { getWorkflowRun: async () => ({ data: run }),
      getWorkflow: async () => ({ data: { id: 4 } }), listWorkflowRunArtifacts: {} },
    pulls: { get: async () => ({ data: { state: 'open', head: { sha: liveHead, ref: 'any-branch', repo: { id: liveRepo ?? (fork ? 2 : 1) } }, base: { sha: snapshotBase, ref: 'target', repo: { full_name: 'owner/repo' } } } }) },
    repos: { getBranch: async () => ({ data: { commit: { sha: liveBase } } }),
      getCommit: async () => ({ data: { parents: parents || [{ sha: base }, { sha: head }] } }),
      compareCommitsWithBasehead: async () => ({ data: { status: targetHistory } }) },
    issues: { listComments: {},
      createComment: async x => { calls.push(['create', x]); return { data: { html_url: 'created' } }; },
      updateComment: async x => { calls.push(['update', x]); return { data: { html_url: 'updated' } }; }
    }
  } };
  return { directory, calls, outputs, args: { github, context,
    core: { info() {}, setOutput(key, value) { outputs[key] = value; } }, directory } };
}

test('creates the first comment and ignores a human marker impersonation', async t => {
  const f = fixture(t, { previous: [{ id: 4, user: { login: 'human', type: 'User' }, body: marker }] });
  await publish(f.args);
  assert.equal(f.calls[0][0], 'create');
  assert.equal(f.calls[0][1].issue_number, 27);
  assert.match(f.calls[0][1].body, /slither-report-run:.*"run":100/);
});
test('updates the existing bot comment rather than creating another', async t => {
  const f = fixture(t, { previous: [{ id: 42, user: bot, body: `${marker}\n<!-- slither-report-run: {"run":99,"attempt":1} -->` }] });
  await publish(f.args);
  assert.equal(f.calls[0][0], 'update');
  assert.equal(f.calls[0][1].comment_id, 42);
});
test('rejects forged target PR, run and mismatched merge parents', async t => {
  for (const options of [{ reportOverride: { pr: 1 } }, { reportOverride: { run: 99 } },
    { reportOverride: { attempt: 2 } }, { parents: [{ sha: head }, { sha: base }] }]) {
    const f = fixture(t, options);
    await assert.rejects(publish(f.args));
    assert.equal(f.calls.length, 0);
  }
});
test('does not overwrite a newer head or rewritten target', async t => {
  for (const options of [{ liveHead: base }, { liveBase: head }]) {
    const f = fixture(t, options);
    await publish(f.args);
    assert.equal(f.calls.length, 0);
  }
});
test('publishes a pinned comparison when the target advances and records its identity', async t => {
  const f = fixture(t, { liveBase: merge, targetHistory: 'ahead' });
  await publish(f.args);
  assert.equal(f.calls.length, 1);
  assert.match(f.calls[0][1].body, /Target advanced to `cccccccc` during this run; results use base `bbbbbbbb`/);
  const details = f.calls[0][1].body.split('<details>')[1].split('</details>')[0];
  assert.match(details, /Target advanced/);
});
test('rejects targets rewound behind the measured base', async t => {
  const f = fixture(t, { liveBase: merge, targetHistory: 'behind' });
  await publish(f.args);
  assert.equal(f.calls.length, 0);
});
test('publishes when the live target matches despite an older PR base snapshot', async t => {
  const f = fixture(t, { snapshotBase: head });
  await publish(f.args);
  assert.equal(f.calls.length, 1);
  assert.equal(f.calls[0][0], 'create');
});
test('older runs, attempts and duplicate execution do not overwrite results', async t => {
  for (const saved of [{ run: 101, attempt: 1 }, { run: 100, attempt: 2 }, { run: 100, attempt: 1 }]) {
    const f = fixture(t, { previous: [{ id: 42, user: bot, body: `${marker}\n<!-- slither-report-run: ${JSON.stringify(saved)} -->` }] });
    await publish(f.args);
    assert.equal(f.calls.length, 0);
  }
});
test('newer rerun attempt updates the same comment', async t => {
  const f = fixture(t, { previous: [{ id: 42, user: bot, body: `${marker}\n<!-- slither-report-run: {"run":100,"attempt":1} -->` }],
    runOverride: { run_attempt: 2 }, reportOverride: { attempt: 2 } });
  f.args.context.payload.workflow_run.run_attempt = 2;
  await publish(f.args);
  assert.equal(f.calls[0][0], 'update');
});

test('supports fork runs with an empty PR association and any same-repository branch', async t => {
  for (const fork of [true, false]) {
    const f = fixture(t, { fork });
    await publish(f.args);
    assert.equal(f.calls.length, 1);
    assert.equal(f.calls[0][1].issue_number, 27);
  }
});

test('fork head repository mismatch cannot publish on another PR', async t => {
  const f = fixture(t, { fork: true, liveRepo: 9 });
  await publish(f.args);
  assert.equal(f.calls.length, 0);
});

test('rejects other workflows, repositories, events, conclusions and obsolete attempts', async t => {
  for (const runOverride of [{ workflow_id: 8 }, { path: '.github/workflows/other.yml' },
    { repository: { full_name: 'other/repo' } }, { event: 'push' }, { status: 'in_progress' },
    { conclusion: 'failure' }, { run_attempt: 2 }, { head_sha: base }]) {
    const f = fixture(t, { runOverride });
    await assert.rejects(publish.prepare(f.args));
    assert.equal(f.calls.length, 0);
  }
});

test('selects exactly one bounded source artifact before downloading', async t => {
  const f = fixture(t);
  await publish.prepare(f.args);
  assert.equal(f.outputs.artifact_id, 5);
  for (const artifacts of [[], [{ name: 'pr-report-comment', expired: true }],
    [{ name: 'pr-report-comment', size_in_bytes: 1_000_001 }],
    [{ name: 'pr-report-comment' }, { name: 'pr-report-comment' }]]) {
    const invalid = fixture(t, { artifacts });
    await assert.rejects(publish.prepare(invalid.args));
  }
});

test('rechecks PR freshness immediately before posting', async t => {
  const f = fixture(t);
  const original = f.args.github.rest.pulls.get;
  let checks = 0;
  f.args.github.rest.pulls.get = async args => {
    const response = await original(args);
    if (++checks === 2) response.data.head.sha = base;
    return response;
  };
  await publish(f.args);
  assert.equal(checks, 2);
  assert.equal(f.calls.length, 0);
});

test('rejects oversized data and forged publisher metadata; suppresses mentions', async t => {
  for (const text of ['x'.repeat(50001), `${marker}\n## Comparison\n<!-- slither-report-run: {} -->`]) {
    const f = fixture(t);
    fs.writeFileSync(path.join(f.directory, 'report.md'), text);
    await assert.rejects(publish(f.args));
    assert.equal(f.calls.length, 0);
  }
  const f = fixture(t);
  fs.appendFileSync(path.join(f.directory, 'report.md'), '\n@someone');
  await publish(f.args);
  assert.match(f.calls[0][1].body, /@\u200bsomeone/);
});
