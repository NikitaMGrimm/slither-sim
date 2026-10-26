// Cheap consolidation only. No builds, dispatches, PR body edits or release writes.
const { marker: reportMarker, savedComment, bot } = require('./pr_report_comment.cjs');
const marker = '<!-- slither-release-performance -->';
const sha = /^[0-9a-f]{40}$/;
const labels = new Set(['Faster', 'Slower', 'Within', 'Undecided', 'Unavailable', 'Redefined']);

function releasePR(pr, repository) {
  return pr.state === 'open' && pr.user?.login === 'github-actions[bot]' &&
    pr.head.repo?.full_name === repository && pr.head.ref.startsWith('release-plz-') && pr.base.ref === 'main';
}

function metricText(metric) {
  if (!metric || !labels.has(metric.label)) return 'Unavailable';
  if (metric.label === 'Redefined') return 'Redefined (head only; see source)';
  const interval = metric.interval;
  if (!Number.isFinite(metric.change) || !Array.isArray(interval) || interval.length !== 2 || !interval.every(Number.isFinite)) {
    return `${metric.label} (interval unavailable)`;
  }
  return `${metric.label}: ${metric.change.toFixed(2)}% [${interval.map(x => x.toFixed(2) + '%').join(', ')}], n=${metric.n}`;
}

function evidence(pr, comments) {
  const comment = comments.find(c => bot(c) && c.body?.includes(reportMarker));
  const saved = savedComment(comment);
  if (!saved) return { pr, status: 'Missing report', metrics: [], url: pr.html_url };
  if (saved.head !== pr.head.sha || !sha.test(saved.merge) || !sha.test(saved.base)) {
    return { pr, status: 'Unavailable: report is for an older head or lacks pinned provenance', metrics: [], url: comment.html_url };
  }
  const state = saved.state || 'legacy';
  const outcomes = saved.outcomes || [];
  const counts = [...labels].map(label => `${label} ${outcomes.filter(r => r.label === label).length}`).join(', ');
  return { pr, saved, metrics: state === 'complete' ? saved.metrics || [] : [], url: comment.html_url,
    status: state === 'complete' ? `Preview: ${counts}${saved.errors ? '; other results incomplete' : ''}` : state === 'sampling' ? 'Sampling (provisional)' : state === 'incomplete' ? 'Failed/incomplete; interim claims withdrawn' : 'Historical report; follow source link' };
}

function render({ baseline, candidate, rows, unavailable }) {
  const lines = [marker, '## Release performance evidence', '',
    `Published baseline: [${baseline.tag_name}](${baseline.html_url}) (\`${baseline.commit}\`) · pinned main candidate: \`${candidate}\`.`, '',
    'Advisory evidence from original PR merge previews since that release. Each row has its own baseline and uncertainty; percentages are never summed. These rows do not measure the accumulated shipped-tree change.', '',
    '| PR | Evidence | Host / historical canary | official-100-bot |', '| --- | --- | --- | --- |'];
  for (const row of rows) {
    const text = name => metricText(row.metrics.find(m => m.case === name));
    const host = row.metrics.find(m => m.case === 'host-official-500');
    const legacy = row.metrics.find(m => m.case === 'step-default');
    const headline = host && host.label !== 'Unavailable' ? host : legacy && legacy.label !== 'Unavailable' ? legacy : host || legacy;
    const unit = headline?.metric === 'cpuSeconds' ? 'CPU' : headline?.metric === 'wallSeconds' ? 'wall' : 'historical metric';
    const hostText = headline ? `\`${headline.case}\` (${unit}): ${metricText(headline)}` : 'Unavailable';
    lines.push(`| [#${row.pr.number}](${row.pr.html_url}) | [${row.status}](${row.url}) | ${hostText} | ${text('official-100-bot')} |`);
  }
  if (!rows.length) lines.push('| — | No merged PR comparisons in this release range | — | — |');
  if (unavailable) lines.push('', `⚠️ ${unavailable}`);
  lines.push('', 'Faster / Slower / Within ±5% / Undecided / Unavailable / Redefined retain their source meaning. Host rows replace the historical step canary; their workload and CPU/wall unit remain explicit. Missing and failed results are not zero changes. Hosted labels remain conditional on the timing model; calibration is not established.', '',
    'A direct previous-published-tag versus pinned release-candidate comparison requires an explicit on-demand measurement; this consolidation starts no measurements.', '',
    `<!-- slither-release-inputs: ${JSON.stringify({ baseline: baseline.tag_name, candidate, reports: rows.map(r => [r.pr.number, r.saved?.run ?? null, r.saved?.attempt ?? null, r.saved?.sequence ?? null, r.saved?.state ?? 'missing']) })} -->`, '');
  return lines.join('\n');
}

async function summarize({ github, context, core }) {
  const { owner, repo } = context.repo, repository = `${owner}/${repo}`;
  const open = await github.paginate(github.rest.pulls.list, { owner, repo, state: 'open', base: 'main', per_page: 100 });
  const release = open.filter(pr => releasePR(pr, repository));
  if (release.length !== 1) { core.info('No unique rolling release PR; no summary written'); return; }
  const target = release[0];
  const releases = await github.paginate(github.rest.repos.listReleases, { owner, repo, per_page: 100 });
  const baseline = releases.filter(r => !r.draft && r.published_at).sort((a, b) => b.published_at.localeCompare(a.published_at))[0];
  if (!baseline) { core.info('No published release baseline; summary unavailable'); return; }
  const { data: tagged } = await github.rest.repos.getCommit({ owner, repo, ref: baseline.tag_name });
  baseline.commit = tagged.sha;
  if (!sha.test(baseline.commit)) throw new Error('Published release tag has no pinned commit');
  const { data: branch } = await github.rest.repos.getBranch({ owner, repo, branch: 'main' });
  const candidate = branch.commit.sha;
  const commits = new Set();
  let unavailable = null;
  // Bound API work. A longer range is disclosed, never silently presented as complete.
  for (let page = 1; page <= 10; page++) {
    const { data } = await github.rest.repos.compareCommitsWithBasehead({ owner, repo,
      basehead: `${baseline.commit}...${candidate}`, per_page: 100, page });
    if (!['ahead', 'identical'].includes(data.status)) { unavailable = 'Published tag is not an ancestor of this candidate; release range unavailable.'; break; }
    if (data.total_commits > 1000) { unavailable = 'Release range exceeds the 1,000-commit consolidation limit; evidence listing unavailable.'; break; }
    for (const commit of data.commits) commits.add(commit.sha);
    if (data.commits.length < 100) break;
  }
  const closed = unavailable ? [] : await github.paginate(github.rest.pulls.list, { owner, repo, state: 'closed', base: 'main', per_page: 100 });
  const merged = closed.filter(pr => pr.merged_at && commits.has(pr.merge_commit_sha)).sort((a, b) => a.number - b.number);
  const rows = [];
  for (const pr of merged) {
    const comments = await github.paginate(github.rest.issues.listComments, { owner, repo, issue_number: pr.number, per_page: 100 });
    rows.push(evidence(pr, comments));
  }
  const body = render({ baseline, candidate, rows, unavailable }).replace(/@/g, '@\u200b');
  if (body.length > 60_000) throw new Error('Release evidence exceeds bounded comment size; no partial summary published');
  const comments = await github.paginate(github.rest.issues.listComments, { owner, repo, issue_number: target.number, per_page: 100 });
  const previous = comments.find(c => bot(c) && c.body?.includes(marker));
  const { data: fresh } = await github.rest.pulls.get({ owner, repo, pull_number: target.number });
  const { data: main } = await github.rest.repos.getBranch({ owner, repo, branch: 'main' });
  if (!releasePR(fresh, repository) || fresh.head.sha !== target.head.sha || main.commit.sha !== candidate) {
    core.info('Release PR or candidate advanced while consolidating; next trigger refreshes it'); return;
  }
  if (previous?.body === body) { core.info('Release evidence inputs unchanged'); return; }
  const data = { owner, repo, body };
  if (previous) await github.rest.issues.updateComment({ ...data, comment_id: previous.id });
  else await github.rest.issues.createComment({ ...data, issue_number: target.number });
}

module.exports = { summarize, render, evidence, metricText, releasePR };
