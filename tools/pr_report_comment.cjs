// Loaded only from the default branch by the workflow_run publisher.
// PR artifacts are data; no artifact-provided scripts or paths are executed.
const fs = require('node:fs');
const path = require('node:path');
const marker = '<!-- slither-report-preview -->';

async function sourceRun(github, context) {
  const { owner, repo } = context.repo;
  const repository = `${owner}/${repo}`;
  const event = context.payload.workflow_run;
  if (context.eventName !== 'workflow_run' || !Number.isSafeInteger(event?.id) || event.id <= 0) {
    throw new Error('A completed measurement workflow is required');
  }
  const { data: run } = await github.rest.actions.getWorkflowRun({ owner, repo, run_id: event.id });
  const { data: workflow } = await github.rest.actions.getWorkflow({ owner, repo, workflow_id: 'pr-report.yml' });
  if (run.id !== event.id || run.run_attempt !== event.run_attempt || run.head_sha !== event.head_sha ||
      run.repository?.full_name !== repository || run.workflow_id !== workflow.id ||
      run.path !== '.github/workflows/pr-report.yml' || run.event !== 'pull_request' ||
      run.status !== 'completed' || run.conclusion !== 'success' ||
      !Number.isSafeInteger(run.run_attempt) || run.run_attempt <= 0 || !run.head_repository?.id) {
    throw new Error('Source workflow identity, event, attempt or conclusion is invalid');
  }
  return run;
}

async function prepare({ github, context, core }) {
  const run = await sourceRun(github, context);
  const artifacts = await github.paginate(github.rest.actions.listWorkflowRunArtifacts,
    { ...context.repo, run_id: run.id, per_page: 100 });
  const matches = artifacts.filter(a => a.name === 'pr-report-comment' && !a.expired);
  if (matches.length !== 1 || matches[0].size_in_bytes > 1_000_000) {
    throw new Error('Expected one bounded comment artifact from this measurement run');
  }
  core.setOutput('artifact_id', matches[0].id);
}

function read(directory, name, limit) {
  const filename = path.join(directory, name);
  const info = fs.lstatSync(filename);
  if (!info.isFile() || info.size > limit) throw new Error(`Invalid report file: ${name}`);
  return fs.readFileSync(filename, 'utf8');
}

async function publish({ github, context, core, directory }) {
  const run = await sourceRun(github, context);
  const { owner, repo } = context.repo;
  const repository = `${owner}/${repo}`;
  const report = JSON.parse(read(directory, 'report.json', 1_000_000));
  let body = read(directory, 'report.md', 50000);
  const sha = /^[0-9a-f]{40}$/;
  if (report.schema !== 1 || !Number.isSafeInteger(report.pr) || report.pr <= 0 ||
      report.repository !== repository || report.head !== run.head_sha ||
      report.run !== run.id || report.attempt !== run.run_attempt ||
      !sha.test(report.base) || !sha.test(report.merge) ||
      !body.startsWith(marker + '\n## Comparison\n') || body.includes('<!-- slither-report-run:')) {
    throw new Error('Report identity, marker or size does not match this PR run');
  }
  // Fork payloads can omit pull_requests. Include closed PRs in the fallback:
  // a run from a closed PR must not migrate to another PR reusing its branch.
  let associated = run.pull_requests || [];
  if (!associated.length) {
    associated = (await github.paginate(github.rest.pulls.list, {
      owner, repo, state: 'all', per_page: 100,
      head: `${run.head_repository.full_name.split('/')[0]}:${run.head_branch}`,
    })).filter(pr => pr.head.repo?.id === run.head_repository.id && pr.head.ref === run.head_branch);
  }
  if (associated.length !== 1 || associated[0].number !== report.pr) {
    throw new Error('Report PR is not associated with this workflow run');
  }
  const { data: merge } = await github.rest.repos.getCommit({ owner, repo, ref: report.merge });
  if (merge.parents.length !== 2 || merge.parents[0].sha !== report.base || merge.parents[1].sha !== report.head) {
    throw new Error('Comparison merge parents do not match the measured base and PR head');
  }
  let currentTarget = report.base;
  const fresh = async () => {
    const { data: pr } = await github.rest.pulls.get({ owner, repo, pull_number: report.pr });
    if (pr.state !== 'open' || pr.head.sha !== report.head ||
        pr.base.repo.full_name !== repository || pr.head.repo?.id !== run.head_repository.id ||
        pr.head.ref !== run.head_branch) return false;
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
  const comments = await github.paginate(github.rest.issues.listComments, { owner, repo, issue_number: report.pr, per_page: 100 });
  const previous = comments.find(c => c.user?.login === 'github-actions[bot]' && c.user?.type === 'Bot' && c.body?.includes(marker));
  if (previous) {
    const metadata = previous.body.match(/<!-- slither-report-run: (\{[^\n]+\}) -->/);
    if (metadata) {
      const saved = JSON.parse(metadata[1]);
      if (saved.run > run.id || (saved.run === run.id && saved.attempt >= run.run_attempt)) {
        core.info('Skipping an older or already-published run attempt');
        return;
      }
    }
  }
  // Untrusted diagnostics must not notify arbitrary GitHub users or teams.
  body = body.replace(/@/g, '@\u200b');
  body += `\n<!-- slither-report-run: ${JSON.stringify({ run: run.id, attempt: run.run_attempt, head: report.head })} -->\n`;
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
    : await github.rest.issues.createComment({ ...data, issue_number: report.pr });
  core.info(`Published ${response.data.html_url}`);
}

module.exports = publish;
module.exports.prepare = prepare;
