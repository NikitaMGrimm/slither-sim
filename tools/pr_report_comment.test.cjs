const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const publish = require('./pr_report_comment.cjs');

const head = 'a'.repeat(40), base = 'b'.repeat(40), merge = 'c'.repeat(40);
const marker = '<!-- slither-report-preview -->';
const bot = { login: 'github-actions[bot]', type: 'Bot' };
function fixture(t, { previous = [], liveHead = head, liveBase = base, snapshotBase = liveBase, reportOverride = {}, fork = false } = {}) {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'slither-comment-test-'));
  t.after(() => fs.rmSync(directory, { recursive: true }));
  const report = { schema: 1, repository: 'owner/repo', pr: 27, head, base, merge, ...reportOverride };
  fs.writeFileSync(path.join(directory, 'report.json'), JSON.stringify(report));
  fs.writeFileSync(path.join(directory, 'report.md'), `${marker}\n## Comparison\nMeasured report`);
  const calls = [];
  const context = { repo: { owner: 'owner', repo: 'repo' }, sha: merge, payload: { pull_request: {
    number: 27, head: { sha: head, ref: 'ci/report-preview', repo: { full_name: fork ? 'fork/repo' : 'owner/repo' } }
  } } };
  const github = { paginate: async () => previous, rest: {
    pulls: { get: async () => ({ data: { state: 'open', head: { sha: liveHead, ref: 'ci/report-preview', repo: { full_name: 'owner/repo' } }, base: { sha: snapshotBase, ref: 'target' } } }) },
    repos: { getBranch: async () => ({ data: { commit: { sha: liveBase } } }) },
    issues: { listComments: {},
      createComment: async x => { calls.push(['create', x]); return { data: { html_url: 'created' } }; },
      updateComment: async x => { calls.push(['update', x]); return { data: { html_url: 'updated' } }; }
    }
  } };
  process.env.GITHUB_RUN_ID = '100';
  process.env.GITHUB_RUN_ATTEMPT = '1';
  return { directory, calls, args: { github, context, core: { info() {} }, directory } };
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
test('rejects fork, forged target PR and mismatched merge identities', async t => {
  for (const options of [{ fork: true }, { reportOverride: { pr: 1 } }, { reportOverride: { merge: base } }]) {
    const f = fixture(t, options);
    await assert.rejects(publish(f.args));
    assert.equal(f.calls.length, 0);
  }
});
test('does not overwrite a newer head or changed target', async t => {
  for (const options of [{ liveHead: base }, { liveBase: head }]) {
    const f = fixture(t, options);
    await publish(f.args);
    assert.equal(f.calls.length, 0);
  }
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
  const f = fixture(t, { previous: [{ id: 42, user: bot, body: `${marker}\n<!-- slither-report-run: {"run":100,"attempt":1} -->` }] });
  process.env.GITHUB_RUN_ATTEMPT = '2';
  await publish(f.args);
  assert.equal(f.calls[0][0], 'update');
});
