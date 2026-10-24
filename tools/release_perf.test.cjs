const {test} = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const pinning = require('./release_perf_request.cjs');
const publisher = require('./release_perf_publish.cjs');
const a='a'.repeat(40), b='b'.repeat(40), c='c'.repeat(40), d='d'.repeat(40), e='e'.repeat(40), f='f'.repeat(40);
function request() {
  const measurement={repository:'owner/repo',baseline:{tag:'v0.3.1',commit:a,tree:b},candidate:{commit:c,tree:d},harness:{commit:e,tree:f},
    protocol:'slither-perf-sequential-v2',config:structuredClone(pinning.config)};
  return {schema:pinning.schema,key:pinning.key(measurement),measurement,run:42,attempt:1,published_release_id:7,target:{number:90,head:'1'.repeat(40)}};
}
function fixture() {
  const r=request(), writes=[], outputs={};
  const inputs={baseline_tag:'v0.3.1',baseline_commit:a,candidate_commit:c};
  const context={repo:{owner:'owner',repo:'repo'},runId:42,sha:e,ref:'refs/heads/main',eventName:'workflow_run',
    payload:{inputs,workflow_run:{id:42,run_attempt:1,head_sha:e}}};
  const run={id:42,run_attempt:1,head_sha:e,head_branch:'main',repository:{full_name:'owner/repo'},head_repository:{full_name:'owner/repo'},
    workflow_id:8,path:'.github/workflows/release-perf.yml',event:'workflow_dispatch',status:'completed',conclusion:'success',display_title:pinning.title(inputs)};
  const target={number:90,state:'open',user:{login:'github-actions[bot]'},head:{sha:r.target.head,repo:{full_name:'owner/repo'},ref:'release-plz-main'},base:{ref:'main'}};
  const active=[], comments=[];
  const github={paginate:async(method,args)=>{
    if(method===github.rest.pulls.list)return [target];
    if(method===github.rest.issues.listComments)return comments;
    if(method===github.rest.actions.listWorkflowRunArtifacts)return [];
    throw Error('Unexpected paginate');
  },rest:{repos:{get:async()=>({data:{default_branch:'main'}}),getReleaseByTag:async()=>({data:{id:7,draft:false,published_at:'2026-10-25'}}),
    getCommit:async({ref})=>{const found=ref.startsWith('refs/tags/')?r.measurement.baseline:Object.values(r.measurement).find(p=>p?.commit===ref);assert.ok(found,ref);return {data:{sha:found.commit,commit:{tree:{sha:found.tree}}}};},
    compareCommitsWithBasehead:async()=>({data:{status:'ahead'}})},
  actions:{getWorkflowRun:async()=>({data:run}),getWorkflow:async()=>({data:{id:8}}),listWorkflowRuns:async({status})=>({data:{workflow_runs:active.filter(r=>r.status===status)}}),listWorkflowRunArtifacts:{}},
  pulls:{list:{},get:async()=>({data:target})},issues:{listComments:{},createComment:async x=>writes.push(x),updateComment:async x=>writes.push(x)}}};
  const core={info(){},setOutput:(name,value)=>outputs[name]=value};
  return {r,context,run,target,github,core,active,writes,outputs,comments};
}
function complete(r) {
  const inputs={'examples/perf_baseline.rs':'0'.repeat(64)};
  return {schema:pinning.schema,request:structuredClone(r),state:'complete',errors:[],workloads:[{name:'one'}],traces:[{case:'one',reading:'Changed'}],inputs,
    identities:Object.fromEntries(['baseline','candidate'].map(label=>[label,{...r.measurement[label],label,overlay_inputs:inputs}])),
    performance_plan_sha256:'5'.repeat(64),performance:[{case:'one',label:'Undecided',n_used:8,n_complete:10,
      result:{n:8,label:'Undecided',change_percent:-2,interval_percent:[-8,12],baseline_ms_per_operation:1,candidate_ms_per_operation:.98}}]};
}
test('pinning validates published tag and both exact SHAs, no merge-parent assumptions',async()=>{
  const x=fixture(), directory=fs.mkdtempSync(path.join(os.tmpdir(),'slither-pin-test-'));
  const old=process.env.GITHUB_RUN_ATTEMPT;process.env.GITHUB_RUN_ATTEMPT='1';
  try {
    const filename=path.join(directory,'request.json');
    await pinning.pin({...x,filename});
    const saved=JSON.parse(fs.readFileSync(filename));
    assert.deepEqual(saved,x.r);
    assert.equal(x.outputs.key,x.r.key);
    assert.equal(x.outputs.duplicate,'false');
    x.context.payload.inputs.baseline_commit=b;
    await assert.rejects(pinning.pin({...x,filename}),/differs/);
  } finally {if(old===undefined)delete process.env.GITHUB_RUN_ATTEMPT;else process.env.GITHUB_RUN_ATTEMPT=old;fs.rmSync(directory,{recursive:true,force:true});}
});
test('overlap suppression keys all declared inputs and never reuses completed evidence',async()=>{
  for(const change of ['none','candidate','harness','completed']){
    const x=fixture(), directory=fs.mkdtempSync(path.join(os.tmpdir(),'slither-dedupe-test-'));
    const older={...x.run,id:40,status:'in_progress'};
    if(change==='candidate')older.display_title=older.display_title.replace(c,d);
    if(change==='harness')older.head_sha=f;
    if(change==='completed')older.status='completed';
    x.active.push(older);
    try {
      await pinning.pin({...x,filename:path.join(directory,'request.json')});
      assert.equal(x.outputs.duplicate,change==='none'?'true':'false');
      assert.equal(fs.existsSync(path.join(directory,'request.json')),change!=='none');
    }finally{fs.rmSync(directory,{recursive:true,force:true});}
  }
});
test('source validation rejects wrong workflow, branch, event, attempt or ancestry',async()=>{
  const x=fixture();assert.equal(await publisher.sourceRun(x.github,x.context),x.run);
  for(const [field,value]of [['head_branch','untrusted'],['event','pull_request'],['workflow_id',10],['run_attempt',2],['head_sha',c]]){
    const original=x.run[field];x.run[field]=value;
    await assert.rejects(publisher.sourceRun(x.github,x.context),/Untrusted/);x.run[field]=original;
  }
  x.github.rest.repos.compareCommitsWithBasehead=async()=>({data:{status:'diverged'}});
  await assert.rejects(publisher.sourceRun(x.github,x.context),/default-branch/);
});
test('published request identity is independently verified before publication',async()=>{
  const x=fixture();await publisher.validateRequest(x.github,x.context,x.run,x.r);
  const changed=structuredClone(x.r);changed.measurement.candidate.commit=b;
  await assert.rejects(publisher.validateRequest(x.github,x.context,x.run,changed),/declared/);
  x.github.rest.repos.getReleaseByTag=async()=>({data:{id:7,draft:true,published_at:'2026-10-25'}});
  await assert.rejects(publisher.validateRequest(x.github,x.context,x.run,x.r),/baseline identity/);
});
test('report boundary rejects missing pins, parity, unbounded rows, bad intervals and resurrected failure claims',()=>{
  const r=request();publisher.validateReport(complete(r),r);
  const mutations=[x=>x.request.key='bad',x=>x.performance.push(...Array(10).fill(x.performance[0])),
    x=>x.performance[0].case='[mention](@user)',x=>x.performance[0].n_used=9,x=>x.performance[0].result.interval_percent=[NaN,4],
    x=>x.performance[0].result.interval_percent=[5,-5],x=>x.state='unavailable',x=>x.traces=[],
    x=>x.identities.baseline.commit=c,x=>x.identities.candidate.overlay_inputs={},x=>x.performance_plan_sha256='bad',x=>x.workloads.push({name:'hidden'})];
  for(const mutate of mutations){const report=complete(r);mutate(report);assert.throws(()=>publisher.validateReport(report,r));}
});
test('trusted renderer keeps exact source identity, original run attempt and conditional limits',()=>{
  const r=request(), body=publisher.render(r,complete(r),{id:42,run_attempt:1},'owner','repo');
  for(const value of [a,b,c,d,e,f,r.key,'/runs/42/attempts/1','Undecided','Changed','8 / 10','interval budgets are not combined','later shipped tree','hosted calibration is not established'])assert.ok(body.includes(value),value);
  assert.match(body,/^> \[!NOTE\]\n> 🤖 \*\*GPT 6.1 Sol writing on behalf of owner/);
  assert.ok(!body.includes('slither-report-preview'));
});
test('a suppressed request has no publication, write or result-reuse path',async()=>{
  const x=fixture();await publisher.publish(x);assert.equal(x.writes.length,0);
});
function withArtifacts(x) {
  const paginate=x.github.paginate;
  x.github.paginate=async(method,args)=>method===x.github.rest.actions.listWorkflowRunArtifacts?
    ['request','report'].map((kind,i)=>({id:i+1,name:`release-perf-${kind}-1`,size_in_bytes:100,expired:false})):paginate(method,args);
  x.readArtifact=async(_github,_context,_artifact,kind)=>kind==='request'?structuredClone(x.r):complete(x.r);
  return x;
}
test('terminal evidence posts only to the pinned release PR and ignores untrusted Markdown',async()=>{
  const x=withArtifacts(fixture());
  await publisher.publish(x);
  assert.equal(x.writes.length,1);assert.equal(x.writes[0].issue_number,90);
  assert.ok(x.writes[0].body.includes('Direct release performance'));
  assert.ok(!Object.hasOwn(x.writes[0],'title'));
});
test('cancellation publishes Unavailable with original pins and never reads partial timing data',async()=>{
  const x=withArtifacts(fixture());x.run.conclusion='cancelled';
  const original=x.readArtifact;
  x.readArtifact=async(...args)=>{assert.equal(args[3],'request');return original(...args);};
  await publisher.publish(x);
  assert.equal(x.writes.length,1);assert.match(x.writes[0].body,/Unavailable/);
  assert.ok(!x.writes[0].body.includes('Undecided'));
  assert.ok(x.writes[0].body.includes(c));
});
test('stale release head, a different owner, malformed result and newer original attempt prevent writes',async()=>{
  for(const change of ['stale','owner','malformed','newer']) {
    const x=withArtifacts(fixture());
    if(change==='stale')x.target.head.sha=a;
    if(change==='owner')x.target.user.login='human';
    if(change==='malformed'){
      const original=x.readArtifact;
      x.readArtifact=async(...args)=>{const report=await original(...args);if(args[3]==='report')report.performance[0].case='@pwn';return report;};
      await assert.rejects(publisher.publish(x));
    } else {
      if(change==='newer')x.comments.push({id:10,user:{login:'github-actions[bot]',type:'Bot'},body:
        `<!-- slither-release-perf-direct:${x.r.key}:42 -->\n<!-- slither-release-perf-run: {"run":42,"attempt":2} -->`});
      await publisher.publish(x);
    }
    assert.equal(x.writes.length,0);
  }
});
test('distinct source runs retain their own terminal comments regardless of source ordering',async()=>{
  const comments=[], writes=[];
  const requests=[44,42,43].map(id=>{
    const x=withArtifacts(fixture());
    x.run.id=id;x.r.run=id;x.context.runId=id;x.context.payload.workflow_run.id=id;
    if(id===43)x.run.conclusion='cancelled';
    const paginate=x.github.paginate;
    x.github.paginate=async(method,args)=>method===x.github.rest.issues.listComments?comments:paginate(method,args);
    x.github.rest.issues.createComment=async args=>{
      writes.push(args);comments.push({id,user:{login:'github-actions[bot]',type:'Bot'},body:args.body});
    };
    return x;
  });
  await Promise.all(requests.map(x=>publisher.publish(x)));
  assert.equal(writes.length,3);
  for(const x of requests)assert.ok(comments.some(c=>c.body.includes(`<!-- slither-release-perf-direct:${x.r.key}:${x.run.id} -->`)));
  assert.ok(comments.find(c=>c.id===43).body.includes('Unavailable'));
  const workflow=fs.readFileSync(path.join(__dirname,'../.github/workflows/release-perf-comment.yml'),'utf8');
  assert.match(workflow,/group: release-perf-comment-\$\{\{ github\.event\.workflow_run\.id \}\}/);
});

test('the declared runner is the measurement runner and cancellation reaches the collector',()=>{
  const workflow=fs.readFileSync(path.join(__dirname,'../.github/workflows/release-perf.yml'),'utf8');
  const measure=workflow.slice(workflow.indexOf('\n  measure:'));
  assert.equal(measure.match(/^    runs-on: (\S+)$/m)?.[1],pinning.config.runner);
  assert.match(measure,/^        run: exec python3 -B tools\/release_perf\.py /m);
});
