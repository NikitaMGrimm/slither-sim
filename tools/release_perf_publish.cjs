// Default-branch code only. Artifacts are bounded data; never execute their text.
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const {execFileSync} = require('node:child_process');
const {key, canonical, config, schema, sha, tag, title} = require('./release_perf_request.cjs');
const {releasePR} = require('./release_performance.cjs');
const {bot} = require('./pr_report_comment.cjs');
const hash = /^[0-9a-f]{64}$/;
const labels = new Set(['Faster','Slower','Within','Undecided','Unavailable']);
const looks = new Set([8,12,26,...Array.from({length:11},(_,i)=>26*2**(i+1))]);
const count = x => Number.isSafeInteger(x) && x >= 0 && x <= 53248;

async function sourceRun(github, context) {
  const event = context.payload.workflow_run;
  if (context.eventName !== 'workflow_run' || !Number.isSafeInteger(event?.id)) throw Error('Expected source workflow_run');
  const {data:run} = await github.rest.actions.getWorkflowRun({...context.repo,run_id:event.id});
  const {data:workflow} = await github.rest.actions.getWorkflow({...context.repo,workflow_id:'release-perf.yml'});
  const {data:repository} = await github.rest.repos.get(context.repo);
  if (run.id !== event.id || run.run_attempt !== event.run_attempt || run.head_sha !== event.head_sha ||
      run.repository?.full_name !== `${context.repo.owner}/${context.repo.repo}` ||
      run.head_repository?.full_name !== run.repository.full_name || run.workflow_id !== workflow.id ||
      run.path !== '.github/workflows/release-perf.yml' || run.event !== 'workflow_dispatch' ||
      run.status !== 'completed' || run.head_branch !== repository.default_branch || !sha.test(run.head_sha) ||
      !Number.isSafeInteger(run.run_attempt) || run.run_attempt < 1) throw Error('Untrusted source workflow identity');
  const {data:ancestry} = await github.rest.repos.compareCommitsWithBasehead({...context.repo,
    basehead:`${run.head_sha}...${repository.default_branch}`,per_page:1});
  if (!['ahead','identical'].includes(ancestry.status)) throw Error('Harness is not a default-branch revision');
  return run;
}

async function validateRequest(github, context, run, request) {
  const m = request?.measurement;
  if (request?.schema !== schema || request.run !== run.id || request.attempt !== run.run_attempt ||
      !m || m.repository !== `${context.repo.owner}/${context.repo.repo}` || request.key !== key(m) ||
      m.protocol !== 'slither-perf-sequential-v2' || canonical(m.config) !== canonical(config) ||
      !tag.test(m.baseline?.tag) || m.harness?.commit !== run.head_sha ||
      run.display_title !== title({baseline_tag:m.baseline.tag, baseline_commit:m.baseline.commit, candidate_commit:m.candidate?.commit})) {
    throw Error('Request does not match the declared measurement inputs');
  }
  for (const label of ['baseline','candidate','harness']) {
    const pin = m[label];
    if (!sha.test(pin?.commit) || !sha.test(pin?.tree)) throw Error('Invalid source pin');
    const {data} = await github.rest.repos.getCommit({...context.repo,ref:pin.commit});
    if (data.sha !== pin.commit || data.commit?.tree?.sha !== pin.tree) throw Error('Source commit/tree mismatch');
  }
  const {data:release} = await github.rest.repos.getReleaseByTag({...context.repo,tag:m.baseline.tag});
  const {data:tagged} = await github.rest.repos.getCommit({...context.repo,ref:`refs/tags/${m.baseline.tag}`});
  if (release.draft || !release.published_at || release.id !== request.published_release_id || tagged.sha !== m.baseline.commit) {
    throw Error('Published baseline identity changed');
  }
  if (request.target !== null && (!Number.isSafeInteger(request.target?.number) || request.target.number <= 0 || !sha.test(request.target.head))) {
    throw Error('Invalid release evidence target');
  }
}

function validateReport(report, request) {
  if (report?.schema !== schema || canonical(report.request) !== canonical(request) ||
      !['complete','unavailable'].includes(report.state) || !Array.isArray(report.performance) || report.performance.length > 10 ||
      !Array.isArray(report.traces) || report.traces.length > 10 || !Array.isArray(report.errors) || report.errors.length > 100) {
    throw Error('Invalid terminal report');
  }
  if (report.state === 'unavailable' && report.performance.length) throw Error('Unavailable report contains timing claims');
  if (report.state === 'complete') {
    if (!Array.isArray(report.workloads) || !report.workloads.length || report.workloads.length > 10 ||
        new Set(report.workloads.map(w=>w.name)).size !== report.workloads.length ||
        canonical(report.workloads.map(w=>w.name).sort()) !== canonical(report.performance.map(r=>r.case).sort()) ||
        canonical(report.workloads.map(w=>w.name).sort()) !== canonical(report.traces.map(r=>r.case).sort())) {
      throw Error('Missing or selectively omitted workload evidence');
    }
    for (const label of ['baseline','candidate']) {
      const identity = report.identities?.[label], pin = request.measurement[label];
      if (!identity || identity.commit !== pin.commit || identity.tree !== pin.tree || identity.label !== label ||
          !report.inputs || !Object.keys(report.inputs).length || canonical(identity.overlay_inputs) !== canonical(report.inputs)) {
        throw Error('Build or shared harness identity mismatch');
      }
    }
  }
  const names = new Set();
  for (const row of report.performance) {
    if (!/^[a-zA-Z0-9_-]{1,80}$/.test(row.case) || names.has(row.case) || !labels.has(row.label) ||
        !count(row.n_used) || !count(row.n_complete) || row.n_used > row.n_complete ||
        (row.n_used && !looks.has(row.n_used)) || (row.label === 'Unavailable' && (row.result !== null || row.n_used !== 0))) throw Error('Invalid performance row');
    names.add(row.case);
    if (row.label !== 'Unavailable') {
      const r = row.result;
      if (!r || !row.n_used || r.n !== row.n_used || !Number.isFinite(r.change_percent) ||
          r.label !== row.label || !['baseline_ms_per_operation','candidate_ms_per_operation'].every(k=>Number.isFinite(r[k]) && r[k]>0)) throw Error('Invalid timing estimate');
      const interval = r.interval_percent;
      if (interval === null) {
        if (row.label !== 'Undecided') throw Error('Resolved result without interval');
      } else if (!Array.isArray(interval) || interval.length !== 2 || !interval.every(Number.isFinite) || interval[0] > interval[1]) {
        throw Error('Invalid interval');
      }
    }
  }
  const traces = new Set();
  for (const row of report.traces) {
    if (!/^[a-zA-Z0-9_-]{1,80}$/.test(row.case) || traces.has(row.case) || !['Equal','Changed','Unavailable'].includes(row.reading)) throw Error('Invalid parity row');
    traces.add(row.case);
  }
  if (report.performance.some(r=>r.label !== 'Unavailable' && !report.traces.some(t=>t.case===r.case && t.reading!=='Unavailable'))) throw Error('Timing claim without parity evidence');
  if (report.state === 'complete' && report.performance.length && !hash.test(report.performance_plan_sha256)) throw Error('Missing frozen plan identity');
}

function render(request, report, run, owner, repo) {
  const m = request.measurement, url = `https://github.com/${owner}/${repo}/actions/runs/${run.id}/attempts/${run.run_attempt}`;
  const marker = `<!-- slither-release-perf-direct:${request.key}:${run.id} -->`;
  const lines = ['> [!NOTE]', `> 🤖 **GPT 6.1 Sol writing on behalf of ${owner}**`, '', marker, '## Direct release performance', '',
    `Published **${m.baseline.tag}**: \`${m.baseline.commit}\` (tree \`${m.baseline.tree}\`).`,
    `Pinned candidate: \`${m.candidate.commit}\` (tree \`${m.candidate.tree}\`).`,
    `Shared harness: \`${m.harness.commit}\` (tree \`${m.harness.tree}\`).`, '',
    `[Original run, plan, raw attempts and regression evidence](${url}) · request \`${request.key}\`.`, '',
    'This direct comparison is separate from the consolidated PR preview evidence. Percentages and interval budgets are not combined. It measures these exact revisions; it makes no claim about a later shipped tree.', '',
    '| Workload | Change [conditional interval] | Result | Parity | Pairs used / complete |',
    '| --- | ---: | --- | --- | ---: |'];
  for (const row of report.performance) {
    const r = row.result;
    const value = !r ? '—' : `${r.change_percent.toFixed(2)}% ` + (r.interval_percent === null ? '[interval unavailable]' : `[${r.interval_percent.map(x=>x.toFixed(2)+'%').join(', ')}]`);
    lines.push(`| ${row.case} | ${value} | ${row.label} | ${report.traces.find(t=>t.case===row.case)?.reading || 'Unavailable'} | ${row.n_used} / ${row.n_complete} |`);
  }
  if (!report.performance.length) lines.push('| — | — | Unavailable | Unavailable | 0 / 0 |');
  lines.push('', `State: ${report.state}; ${report.errors.length} diagnostic(s) in artifacts. A build/catalog incompatibility or failed attempt is Unavailable and receives no asymmetric adaptation.`, '',
    'Protocol v2: Windows, one block per fresh process, at most ten workloads and fourteen covered looks; 1,800 s job / 1,500 s stage / 90 s reserve. Negative change means less operation wall time. Coverage is conditional on independent normal constant-variance log errors with constant change and order effects; hosted calibration is not established. Changed parity compares different behavior.', '',
    `<!-- slither-release-perf-run: ${JSON.stringify({run:run.id,attempt:run.run_attempt,key:request.key,candidate:m.candidate.commit,baseline:m.baseline.commit})} -->`);
  return lines.join('\n');
}

async function download(github, context, artifact, mode) {
  if (artifact.size_in_bytes > 1_000_000 || artifact.expired) throw Error('Invalid bounded artifact');
  const {data} = await github.rest.actions.downloadArtifact({...context.repo,artifact_id:artifact.id,archive_format:'zip'});
  const bytes = Buffer.from(data);
  if (bytes.length > 1_000_000) throw Error('Oversized archive');
  const directory = fs.mkdtempSync(path.join(os.tmpdir(),'slither-direct-'));
  try {
    const archive = path.join(directory,'input.zip'), output = path.join(directory,'data');
    fs.writeFileSync(archive,bytes);
    execFileSync('python3',['tools/release_perf_archive.py',mode,archive,output],{timeout:10_000});
    return JSON.parse(fs.readFileSync(path.join(output,mode==='request'?'request.json':'report.json'),'utf8'));
  } finally { fs.rmSync(directory,{recursive:true,force:true}); }
}

async function publish({github, context, core, readArtifact = download}) {
  const run = await sourceRun(github,context);
  const artifacts = await github.paginate(github.rest.actions.listWorkflowRunArtifacts,{...context.repo,run_id:run.id,per_page:100});
  if (!artifacts.some(a=>a.name===`release-perf-request-${run.run_attempt}` && !a.expired)) {
    core.info('No pinned request: duplicate suppressed or pinning failed; no evidence published');
    return;
  }
  const one = name => {
    const matches = artifacts.filter(a=>a.name===name && !a.expired);
    if (matches.length!==1) throw Error(`Expected exactly one ${name} artifact`);
    return matches[0];
  };
  const request = await readArtifact(github,context,one(`release-perf-request-${run.run_attempt}`),'request');
  await validateRequest(github,context,run,request);
  if (!request.target) {core.info('No unique rolling release PR was pinned; evidence remains in the run');return;}
  let report = {state:'unavailable',performance:[],traces:[],errors:['Source run failed or report unavailable']};
  if (run.conclusion === 'success') {
    report = await readArtifact(github,context,one(`release-perf-report-${run.run_attempt}`),'report');
    validateReport(report,request);
  }
  const {owner,repo} = context.repo;
  const fresh = async () => {
    const {data:pr} = await github.rest.pulls.get({...context.repo,pull_number:request.target.number});
    return releasePR(pr,`${owner}/${repo}`) && pr.head.sha===request.target.head;
  };
  if (!await fresh()) {core.info('Pinned release PR head advanced or closed; retain original run evidence without attaching to a different candidate');return;}
  const body = render(request,report,run,owner,repo);
  const marker = `<!-- slither-release-perf-direct:${request.key}:${run.id} -->`;
  const comments = await github.paginate(github.rest.issues.listComments,{...context.repo,issue_number:request.target.number,per_page:100});
  const previous = comments.find(c=>bot(c) && c.body?.includes(marker));
  const saved = previous?.body?.match(/<!-- slither-release-perf-run: (\{[^\n]+\}) -->/);
  if (saved) {
    const old = JSON.parse(saved[1]);
    if (old.run>run.id || (old.run===run.id && old.attempt>=run.run_attempt)) return;
  }
  if (!await fresh()) return;
  if (previous) await github.rest.issues.updateComment({...context.repo,comment_id:previous.id,body});
  else await github.rest.issues.createComment({...context.repo,issue_number:request.target.number,body});
}
module.exports = {publish, sourceRun, validateRequest, validateReport, render};
