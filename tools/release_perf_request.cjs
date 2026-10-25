// Trusted default-branch request pinning. No builds or measurement dispatches.
const fs = require('node:fs');
const crypto = require('node:crypto');
const schema = 'slither-release-perf-v1';
const sha = /^[0-9a-f]{40}$/;
const tag = /^v[0-9]+\.[0-9]+\.[0-9]+(?:[-+][A-Za-z0-9.-]+)?$/;
const config = {runner:'ubuntu-latest', toolchain:'stable', features:['--no-default-features','--features','networking,parallel'],
  profile:'release', release_settings:{OPT_LEVEL:'3',LTO:'thin',CODEGEN_UNITS:'1',DEBUG:'false',INCREMENTAL:'false',
    OVERFLOW_CHECKS:'false',PANIC:'unwind',STRIP:'none'}, jobs:2, suite:'ci', blocks:1, job_seconds:1800, stage_seconds:1500, reserve_seconds:90};
function canonical(value) {
  if (Array.isArray(value)) return `[${value.map(canonical).join(',')}]`;
  if (value && typeof value === 'object') return `{${Object.keys(value).sort().map(k=>`${JSON.stringify(k)}:${canonical(value[k])}`).join(',')}}`;
  return JSON.stringify(value);
}
const key = value => crypto.createHash('sha256').update(canonical(value)).digest('hex');
const title = inputs => `Release perf ${inputs.baseline_tag} ${inputs.baseline_commit} ${inputs.candidate_commit}`;

async function pin({github, context, core, filename}) {
  const inputs = context.payload.inputs;
  if (!tag.test(inputs.baseline_tag) || inputs.baseline_tag.length > 80 || !sha.test(inputs.baseline_commit) || !sha.test(inputs.candidate_commit)) {
    throw Error('Supply a version tag and full lowercase baseline/candidate commit SHAs');
  }
  const {owner, repo} = context.repo;
  const {data: repository} = await github.rest.repos.get(context.repo);
  if (context.ref !== `refs/heads/${repository.default_branch}`) throw Error('Dispatch only on the default branch');
  const {data: published} = await github.rest.repos.getReleaseByTag({...context.repo, tag:inputs.baseline_tag});
  if (published.draft || !published.published_at) throw Error('Baseline must be a published release');
  const commit = async ref => {
    const {data} = await github.rest.repos.getCommit({...context.repo, ref});
    if (!sha.test(data.sha) || !sha.test(data.commit?.tree?.sha)) throw Error('Missing immutable commit/tree identity');
    return {commit:data.sha, tree:data.commit.tree.sha};
  };
  const baseline = await commit(`refs/tags/${inputs.baseline_tag}`);
  if (baseline.commit !== inputs.baseline_commit) throw Error('Published tag differs from the explicitly supplied baseline commit');
  const candidate = await commit(inputs.candidate_commit), harness = await commit(context.sha);
  if (candidate.commit !== inputs.candidate_commit || harness.commit !== context.sha) throw Error('Commit pin mismatch');
  const measurement = {repository:`${owner}/${repo}`, baseline:{tag:inputs.baseline_tag,...baseline}, candidate, harness,
    protocol:'slither-perf-sequential-v2', config};
  const requestKey = key(measurement);
  // Only suppress an overlapping older declaration. Equality of this title plus
  // harness SHA fixes every measurement input above; never inspect result labels.
  // Completed runs are deliberately not reused, even if their comment still exists.
  for (const status of ['queued','in_progress']) {
    const {data} = await github.rest.actions.listWorkflowRuns({...context.repo, workflow_id:'release-perf.yml',
      event:'workflow_dispatch', status, per_page:100});
    const duplicate = data.workflow_runs.find(r => r.id < context.runId && r.head_sha === harness.commit &&
      r.display_title === title(inputs) && r.head_branch === repository.default_branch && r.repository?.full_name === measurement.repository);
    if (duplicate) {
      core.info(`Overlapping identical request ${requestKey} suppressed; original run ${duplicate.id}. No evidence reused.`);
      core.setOutput('duplicate', 'true');
      return;
    }
  }
  const open = await github.paginate(github.rest.pulls.list, {...context.repo, state:'open', base:repository.default_branch, per_page:100});
  const {releasePR} = require('./release_performance.cjs');
  const targets = open.filter(pr => releasePR(pr, measurement.repository));
  const target = targets.length === 1 ? {number:targets[0].number, head:targets[0].head.sha} : null;
  const request = {schema, key:requestKey, measurement, target,
    run:context.runId, attempt:Number(process.env.GITHUB_RUN_ATTEMPT), published_release_id:published.id};
  fs.writeFileSync(filename, JSON.stringify(request, null, 2)+'\n');
  core.setOutput('key', requestKey);
  core.setOutput('harness', harness.commit);
  core.setOutput('baseline', baseline.commit);
  core.setOutput('candidate', candidate.commit);
  core.setOutput('duplicate', 'false');
}
module.exports = {pin, canonical, key, config, schema, sha, tag, title};
