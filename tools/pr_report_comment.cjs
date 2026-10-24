// Loaded only from the default branch. Measurement artifacts are untrusted data.
const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
const { execFileSync } = require('node:child_process');
const marker = '<!-- slither-report-preview -->';
const metadataMarker = /<!-- slither-report-run: (\{[^\n]+\}) -->/;
const sha = /^[0-9a-f]{40}$/;
const hash = /^[0-9a-f]{64}$/;
const bot = c => c.user?.login === 'github-actions[bot]' && c.user?.type === 'Bot';

async function sourceRun(github, context) {
  const event = context.payload.workflow_run;
  if (context.eventName !== 'workflow_run' || !Number.isSafeInteger(event?.id) || event.id <= 0) throw new Error('A measurement workflow_run is required');
  const { data: run } = await github.rest.actions.getWorkflowRun({ ...context.repo, run_id: event.id });
  const { data: workflow } = await github.rest.actions.getWorkflow({ ...context.repo, workflow_id: 'pr-report.yml' });
  if (run.id !== event.id || run.run_attempt !== event.run_attempt || run.head_sha !== event.head_sha ||
      run.repository?.full_name !== `${context.repo.owner}/${context.repo.repo}` || run.workflow_id !== workflow.id ||
      run.path !== '.github/workflows/pr-report.yml' || run.event !== 'pull_request' ||
      !['in_progress', 'completed'].includes(run.status) ||
      !Number.isSafeInteger(run.run_attempt) || run.run_attempt <= 0 || !run.head_repository?.id) {
    throw new Error('Source workflow identity, event, attempt or status is invalid');
  }
  return run;
}

function artifactSequence(artifact, attempt) {
  if (artifact.name === `pr-report-comment-${attempt}` || artifact.name === 'pr-report-comment') return 5;
  const match = artifact.name.match(/^pr-report-progress-(\d+)-([1-3])$/);
  return match && Number(match[1]) === attempt ? Number(match[2]) : 0;
}

async function selectArtifact(github, context, run) {
  const artifacts = await github.paginate(github.rest.actions.listWorkflowRunArtifacts, { ...context.repo, run_id: run.id, per_page: 100 });
  const eligible = artifacts.filter(a => !a.expired && artifactSequence(a, run.run_attempt));
  const sequence = Math.max(0, ...eligible.map(a => artifactSequence(a, run.run_attempt)));
  const matches = eligible.filter(a => artifactSequence(a, run.run_attempt) === sequence);
  if (!matches.length) return null;
  if (matches.length !== 1 || matches[0].size_in_bytes > 1_000_000) throw new Error('Expected one bounded snapshot artifact from this measurement attempt');
  return { ...matches[0], sequence };
}

async function prepare({ github, context, core }) {
  const run = await sourceRun(github, context);
  if (run.status !== 'completed' || run.conclusion !== 'success') throw new Error('Successful completed run required');
  const artifact = await selectArtifact(github, context, run);
  if (!artifact) throw new Error('Expected one bounded comment artifact');
  core.setOutput('artifact_id', artifact.id);
}

function read(directory, name, limit) {
  const filename = path.join(directory, name), info = fs.lstatSync(filename);
  if (!info.isFile() || info.size > limit) throw new Error(`Invalid report file: ${name}`);
  return fs.readFileSync(filename, 'utf8');
}

function savedComment(comment) {
  const match = comment?.body?.match(metadataMarker);
  return match ? JSON.parse(match[1]) : null;
}

function supersedes(saved, next) {
  if (!saved) return false;
  for (const key of ['run', 'attempt', 'sequence']) {
    const a = saved[key] ?? (key === 'sequence' ? 5 : 0);
    if (a !== next[key]) return a > next[key];
  }
  if (saved.state === 'incomplete' && next.state === 'complete') return false;
  return saved.state !== 'sampling' || next.state === 'sampling';
}

async function publish({ github, context, core, directory, expectedSequence, unavailableReason }) {
  const run = await sourceRun(github, context), { owner, repo } = context.repo, repository = `${owner}/${repo}`;
  const report = JSON.parse(read(directory, 'report.json', 1_000_000));
  let body = read(directory, 'report.md', 50_000);
  const snapshot = report.snapshot || { sequence: 5, state: 'complete' };
  if (report.schema !== 1 || !Number.isSafeInteger(report.pr) || report.pr <= 0 || report.repository !== repository ||
      report.head !== run.head_sha || report.run !== run.id || report.attempt !== run.run_attempt ||
      !sha.test(report.base) || !sha.test(report.merge) || !body.startsWith(marker + '\n## Comparison\n') ||
      body.includes('<!-- slither-report-run:') || !['sampling', 'complete'].includes(snapshot.state) ||
      !Number.isInteger(snapshot.sequence) || (snapshot.state === 'sampling' ? snapshot.sequence < 1 || snapshot.sequence > 3 : snapshot.sequence !== 5) ||
      (report.snapshot && report.snapshot.schema !== 'slither-pr-progress-v1') ||
      (snapshot.state === 'sampling' && !hash.test(report.performance_plan_sha256)) ||
      (expectedSequence !== undefined && snapshot.sequence !== expectedSequence)) throw new Error('Report identity, snapshot, marker or size does not match this PR run');
  if (snapshot.state === 'complete' && run.status !== 'completed') return;
  // A progress artifact selected before successful completion is still interim.
  // Leave terminal precedence for the final artifact or an explicit withdrawal.
  if (snapshot.state === 'sampling' && run.status === 'completed' && run.conclusion === 'success') return;
  let associated = run.pull_requests || [];
  if (!associated.length) {
    associated = (await github.paginate(github.rest.pulls.list, { owner, repo, state: 'all', per_page: 100,
      head: `${run.head_repository.full_name.split('/')[0]}:${run.head_branch}`,
    })).filter(pr => pr.head.repo?.id === run.head_repository.id && pr.head.ref === run.head_branch);
  }
  if (associated.length !== 1 || associated[0].number !== report.pr) throw new Error('Report PR is not associated with this workflow run');
  const { data: merge } = await github.rest.repos.getCommit({ owner, repo, ref: report.merge });
  if (merge.parents.length !== 2 || merge.parents[0].sha !== report.base || merge.parents[1].sha !== report.head) throw new Error('Comparison merge parents do not match the measured base and PR head');
  let currentTarget = report.base, mergedNote = '', actualMerge = null, sameTree = null;
  const fresh = async () => {
    const live = await sourceRun(github, context);
    if (live.status !== run.status || live.conclusion !== run.conclusion) return false;
    const { data: pr } = await github.rest.pulls.get({ owner, repo, pull_number: report.pr });
    if (pr.head.sha !== report.head || pr.base.repo.full_name !== repository || pr.head.repo?.id !== run.head_repository.id || pr.head.ref !== run.head_branch) return false;
    if (pr.state === 'closed') {
      if (!pr.merged || !sha.test(pr.merge_commit_sha)) return false;
      actualMerge = pr.merge_commit_sha;
      const { data: actual } = await github.rest.repos.getCommit({ owner, repo, ref: actualMerge });
      const { data: ancestry } = await github.rest.repos.compareCommitsWithBasehead({ owner, repo, basehead: `${report.base}...${actualMerge}`, per_page: 1 });
      if (!['ahead', 'identical'].includes(ancestry.status) || !sha.test(actual.commit?.tree?.sha) || !sha.test(merge.commit?.tree?.sha)) return false;
      sameTree = actual.commit.tree.sha === merge.commit.tree.sha;
      mergedNote = `Measured preview \`${report.merge.slice(0, 8)}\`; actual merge \`${actualMerge.slice(0, 8)}\` ` +
        (sameTree ? 'has the same tree.' : '**has a different tree**. These results do not measure the shipped tree.');
      return true;
    }
    if (pr.state !== 'open') return false;
    const { data: target } = await github.rest.repos.getBranch({ owner, repo, branch: pr.base.ref });
    currentTarget = target.commit.sha;
    if (currentTarget === report.base) return true;
    const { data: comparison } = await github.rest.repos.compareCommitsWithBasehead({ owner, repo, basehead: `${report.base}...${currentTarget}`, per_page: 1 });
    return comparison.status === 'ahead';
  };
  if (!(await fresh())) { core.info('Skipping stale head, unmerged closure or unverifiable merge identity'); return; }
  const state = unavailableReason ? 'incomplete' : run.status === 'completed' ? run.conclusion === 'success' && snapshot.state === 'complete' ? 'complete' : 'incomplete' : 'sampling';
  const next = { run: run.id, attempt: run.run_attempt, head: report.head, base: report.base, merge: report.merge,
    sequence: state === 'incomplete' ? 5 : snapshot.sequence, state, plan: report.performance_plan_sha256 || null,
    actualMerge, sameTree, metrics: state === 'complete' ? headlines(report.performance || []) : [],
    outcomes: state === 'complete' ? (report.performance || []).map(r => ({ case: r.case, label: r.label })) : [],
    errors: state === 'complete' && Boolean(report.errors?.length) };
  const comments = await github.paginate(github.rest.issues.listComments, { owner, repo, issue_number: report.pr, per_page: 100 });
  const previous = comments.find(c => bot(c) && c.body?.includes(marker)), saved = savedComment(previous);
  if (saved && saved.run === next.run && saved.attempt === next.attempt && saved.plan && next.plan && saved.plan !== next.plan) throw new Error('Snapshot plan changed within one run attempt');
  if (supersedes(saved, next)) { core.info('Skipping older or repeated snapshot'); return; }
  if (state === 'incomplete') body = `${marker}\n## Comparison\n\n⚠️ **Incomplete comparison:** ${unavailableReason || 'source run ' + run.conclusion}. All interim performance claims are withdrawn. [Run and retained attempts](https://github.com/${repository}/actions/runs/${run.id}).\n`;
  if (!(await fresh())) { core.info('Source or PR changed while preparing the comment; next event will reconcile'); return; }
  next.actualMerge = actualMerge;
  next.sameTree = sameTree;
  if (mergedNote) body += `\n${mergedNote}\n`;
  else if (currentTarget !== report.base) body = body.replace('\n</details>', `\n- Target advanced to \`${currentTarget.slice(0, 8)}\` during this run; results use base \`${report.base.slice(0, 8)}\`.\n\n</details>`);
  body = body.replace(/@/g, '@\u200b');
  body += `\n<!-- slither-report-run: ${JSON.stringify(next)} -->\n`;
  const data = { owner, repo, body };
  const response = previous ? await github.rest.issues.updateComment({ ...data, comment_id: previous.id }) : await github.rest.issues.createComment({ ...data, issue_number: report.pr });
  core.info(`Published ${response.data.html_url}`);
}

function headlines(rows) {
  return ['step-default', 'official-100-bot'].map(name => {
    const row = rows.find(r => r.case === name);
    return { case: name, label: row?.label || 'Unavailable', n: row?.n_used || 0,
      change: row?.result?.change_percent ?? null, interval: row?.result?.interval_percent ?? null };
  });
}

async function observe({ github, context, core, sleep = ms => new Promise(resolve => setTimeout(resolve, ms)), now = () => Date.now(), download = downloadReport }) {
  const deadline = now() + 31 * 60 * 1000;
  let lastArtifact = null, completedPolls = 0, terminalFailures = 0;
  while (now() < deadline) {
    const run = await sourceRun(github, context);
    if (run.status === 'completed' && run.conclusion !== 'success') {
      await withdrawPrior({ github, context, core, run });
      return;
    }
    let artifact, processedTerminal = false;
    try {
      artifact = await selectArtifact(github, context, run);
      const revision = `${artifact?.id}:${run.status}:${run.conclusion}`;
      if (artifact && revision !== lastArtifact) {
        const directory = await download(github, context, artifact);
        try { await publish({ github, context, core, directory, expectedSequence: artifact.sequence }); }
        finally { fs.rmSync(directory, { recursive: true, force: true }); }
        lastArtifact = revision;
        processedTerminal = artifact.sequence === 5;
      } else if (artifact?.sequence === 5 && revision === lastArtifact) {
        processedTerminal = true;
      }
    } catch (error) {
      core.warning(`Snapshot unavailable: ${error.message}`);
      const live = await sourceRun(github, context);
      if (live.status === 'completed') {
        if (live.conclusion !== 'success' || ++terminalFailures >= 3) {
          await withdrawPrior({ github, context, core, run: live, reason: live.conclusion === 'success'
            ? 'terminal artifact download or validation failed after three attempts' : `source run ${live.conclusion}` });
          return;
        }
      }
    }
    const current = await sourceRun(github, context);
    if (current.status === 'completed' && run.status === 'completed') {
      if (processedTerminal) return;
      if (++completedPolls >= 3) {
        await withdrawPrior({ github, context, core, run: current, reason: 'source completed without a terminal report' });
        return;
      }
    }
    await sleep(30_000);
  }
  const final = await sourceRun(github, context);
  if (final.status === 'completed') await withdrawPrior({ github, context, core, run: final,
    reason: 'observer deadline reached without a processed terminal report' });
  core.warning('Bounded observer ended; the completed event handles the terminal result');
}

async function withdrawPrior({ github, context, core, run, reason }) {
  // No surviving artifact is necessary to retract a previously trusted snapshot.
  // Its pinned identity came from this publisher, never a human marker.
  let associated = run.pull_requests || [];
  if (!associated.length) associated = (await github.paginate(github.rest.pulls.list, {
    ...context.repo, state: 'all', per_page: 100,
    head: `${run.head_repository.full_name.split('/')[0]}:${run.head_branch}`,
  })).filter(pr => pr.head.repo?.id === run.head_repository.id && pr.head.ref === run.head_branch);
  if (associated.length !== 1) return;
  const pr = associated[0].number;
  const comments = await github.paginate(github.rest.issues.listComments, { ...context.repo, issue_number: pr, per_page: 100 });
  const previous = comments.find(c => bot(c) && c.body?.includes(marker));
  const originalBody = previous?.body, saved = savedComment(previous);
  if (!saved || saved.state !== 'sampling' || saved.run !== run.id || saved.head !== run.head_sha ||
      !Number.isSafeInteger(saved.attempt) || saved.attempt < 1 || saved.attempt > run.run_attempt ||
      !Number.isInteger(saved.sequence) || saved.sequence < 1 || saved.sequence > 3 ||
      !sha.test(saved.base) || !sha.test(saved.merge) || !hash.test(saved.plan)) return;
  const { data: preview } = await github.rest.repos.getCommit({ ...context.repo, ref: saved.merge });
  if (preview.parents.length !== 2 || preview.parents[0].sha !== saved.base || preview.parents[1].sha !== saved.head) {
    throw new Error('Saved comparison parents do not match the source provenance');
  }
  const { data: currentPR } = await github.rest.pulls.get({ ...context.repo, pull_number: pr });
  if (currentPR.base.repo.full_name !== `${context.repo.owner}/${context.repo.repo}`) return;
  // Withdrawal removes an owned provisional claim. The current head, closure
  // and target history may have changed; those gates apply only to new claims.
  const live = await sourceRun(github, context);
  if (live.status !== 'completed' || live.run_attempt !== run.run_attempt || live.conclusion !== run.conclusion) return;
  const latest = await github.paginate(github.rest.issues.listComments, { ...context.repo, issue_number: pr, per_page: 100 });
  const owned = latest.find(c => bot(c) && c.body?.includes(marker));
  if (owned?.id !== previous.id || owned.body !== originalBody) {
    core.info('Skipping withdrawal superseded by a newer comment'); return;
  }
  const next = { ...saved, attempt: run.run_attempt, sequence: 5, state: 'incomplete',
    withdrawn_attempt: saved.attempt, metrics: [], outcomes: [], errors: true };
  const body = `${marker}\n## Comparison\n\n⚠️ **Incomplete comparison:** ${reason || 'source run ' + run.conclusion}. ` +
    `All interim performance claims are withdrawn for measured head \`${saved.head}\` (attempt ${saved.attempt}). ` +
    `[Run and retained attempts](https://github.com/${context.repo.owner}/${context.repo.repo}/actions/runs/${run.id}).\n` +
    `\n<!-- slither-report-run: ${JSON.stringify(next)} -->\n`;
  await github.rest.issues.updateComment({ ...context.repo, comment_id: previous.id, body });
}

async function downloadReport(github, context, artifact) {
  const { data } = await github.rest.actions.downloadArtifact({ ...context.repo, artifact_id: artifact.id, archive_format: 'zip' });
  const buffer = Buffer.from(data);
  if (buffer.length > 1_000_000) throw new Error('Oversized artifact archive');
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'slither-snapshot-'));
  const archive = `${directory}.zip`;
  fs.rmdirSync(directory);
  try {
    fs.writeFileSync(archive, buffer);
    execFileSync('python3', ['tools/pr_report_archive.py', archive, directory], { timeout: 10_000 });
    return directory;
  } catch (error) { fs.rmSync(directory, { recursive: true, force: true }); throw error; }
  finally { fs.rmSync(archive, { force: true }); }
}

module.exports = publish;
Object.assign(module.exports, { prepare, observe, sourceRun, selectArtifact, savedComment, marker, bot, headlines });
