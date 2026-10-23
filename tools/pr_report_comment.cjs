// This publisher runs only for the named same-repository experiment branch.
// Artifacts are data; no artifact-provided scripts or paths are executed.
const fs = require('node:fs');
const path = require('node:path');
const marker = '<!-- slither-report-preview -->';

async function publish({ github, context, core, directory }) {
  const event = context.payload.pull_request;
  const { owner, repo } = context.repo;
  const repository = `${owner}/${repo}`;
  if (!event || event.head.repo.full_name !== repository || event.head.ref !== 'ci/report-preview') {
    throw new Error('Only the same-repository experiment PR may publish');
  }
  const report = JSON.parse(fs.readFileSync(path.join(directory, 'report.json'), 'utf8'));
  let body = fs.readFileSync(path.join(directory, 'report.md'), 'utf8');
  const sha = /^[0-9a-f]{40}$/;
  if (report.schema !== 1 || report.pr !== event.number || report.repository !== repository ||
      report.head !== event.head.sha || !sha.test(report.base) || !sha.test(report.merge) ||
      report.merge !== context.sha || !body.includes(marker) || Buffer.byteLength(body, 'utf8') > 50000) {
    throw new Error('Report identity, marker or size does not match this PR run');
  }
  const run = Number(process.env.GITHUB_RUN_ID);
  const attempt = Number(process.env.GITHUB_RUN_ATTEMPT);
  if (!Number.isSafeInteger(run) || run <= 0 || !Number.isSafeInteger(attempt) || attempt <= 0) {
    throw new Error('Invalid workflow run identity');
  }
  let currentTarget = report.base;
  const fresh = async () => {
    const { data: pr } = await github.rest.pulls.get({ owner, repo, pull_number: event.number });
    if (pr.state !== 'open' || pr.head.sha !== report.head || pr.head.repo.full_name !== repository ||
        pr.head.ref !== 'ci/report-preview') return false;
    // PR base.sha can retain an older snapshot after the target branch moves.
    const { data: target } = await github.rest.repos.getBranch({ owner, repo, branch: pr.base.ref });
    currentTarget = target.commit.sha;
    if (currentTarget === report.base) return true;
    // Results stay pinned to the measured base when concurrent work advances it.
    // A rewritten or diverged target no longer belongs to this comparison.
    const { data: comparison } = await github.rest.repos.compareCommitsWithBasehead({
      owner, repo, basehead: `${report.base}...${currentTarget}`, per_page: 1,
    });
    return comparison.status === 'ahead';
  };
  if (!(await fresh())) {
    core.info('Skipping stale comparison: the PR head or target changed, or PR closed');
    return;
  }
  const comments = await github.paginate(github.rest.issues.listComments, { owner, repo, issue_number: event.number, per_page: 100 });
  const previous = comments.find(c => c.user?.login === 'github-actions[bot]' && c.user?.type === 'Bot' && c.body?.includes(marker));
  if (previous) {
    const metadata = previous.body.match(/<!-- slither-report-run: (\{[^\n]+\}) -->/);
    if (metadata) {
      const saved = JSON.parse(metadata[1]);
      if (saved.run > run || (saved.run === run && saved.attempt >= attempt)) {
        core.info('Skipping an older or already-published run attempt');
        return;
      }
    }
  }
  body += `\n<!-- slither-report-run: ${JSON.stringify({ run, attempt, head: report.head })} -->\n`;
  if (!(await fresh())) {
    core.info('Skipping comparison superseded while preparing the comment');
    return;
  }
  if (currentTarget !== report.base) {
    body = body.replace('\n</details>',
      `\n- Target advanced to \`${currentTarget.slice(0, 8)}\` during this run; results use base \`${report.base.slice(0, 8)}\`.\n\n</details>`);
  }
  const data = { owner, repo, body };
  const response = previous
    ? await github.rest.issues.updateComment({ ...data, comment_id: previous.id })
    : await github.rest.issues.createComment({ ...data, issue_number: event.number });
  core.info(`Published ${response.data.html_url}`);
}

module.exports = publish;
