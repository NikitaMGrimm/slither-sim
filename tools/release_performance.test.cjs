const { test } = require('node:test');
const assert = require('node:assert/strict');
const release = require('./release_performance.cjs');
const bot = {login:'github-actions[bot]',type:'Bot'};
const head='a'.repeat(40), base='b'.repeat(40), merge='c'.repeat(40), candidate='d'.repeat(40);
const pr = {number:27,head:{sha:head},merged_at:'2026-10-25',merge_commit_sha:merge,html_url:'https://example.invalid/pr/27'};
function comment(saved) {
  return {id:42,user:bot,html_url:'https://example.invalid/comment/42',body:`<!-- slither-report-preview -->\n<!-- slither-report-run: ${JSON.stringify({run:100,attempt:1,sequence:5,head,base,merge,...saved})} -->`};
}
test('consolidation preserves missing, failed, undecided and every outcome label',()=>{
  assert.equal(release.evidence(pr,[]).status,'Missing report');
  assert.match(release.evidence(pr,[comment({state:'incomplete'})]).status,/claims withdrawn/);
  assert.match(release.evidence(pr,[comment({head:base,state:'complete'})]).status,/older head/);
  const row=release.evidence(pr,[comment({state:'complete',outcomes:[{case:'x',label:'Slower'},{case:'y',label:'Undecided'}],
    metrics:[{case:'step-default',label:'Undecided',n:26,change:2,interval:[-8,12]}]})]);
  const body=release.render({baseline:{tag_name:'v0.3.1',html_url:'https://example.invalid/release',commit:base},candidate,rows:[row]});
  assert.match(body,/Slower 1/);
  assert.match(body,/Undecided 1/);
  assert.match(body,/Undecided: 2.00% \[-8.00%, 12.00%\], n=26/);
  assert.match(body,/percentages are never summed/);
  assert.match(body,/do not measure the accumulated shipped-tree change/);
});
test('bot-generated release refresh updates only its separately owned comment and deduplicates',async()=>{
  const target={number:30,state:'open',user:bot,head:{sha:candidate,repo:{full_name:'owner/repo'},ref:'release-plz-main'},base:{ref:'main'}};
  const own=[],calls=[];
  const github={paginate:async(method,args)=>{
    if(method===github.rest.pulls.list)return args.state==='open'?[target]:[pr];
    if(method===github.rest.repos.listReleases)return [{tag_name:'v0.3.1',html_url:'https://example.invalid/release',published_at:'2026-10-24',draft:false}];
    if(method===github.rest.issues.listComments)return args.issue_number===27?[comment({state:'incomplete'})]:own;
    throw Error('unexpected API');
  },rest:{pulls:{list:{},get:async()=>({data:target})},repos:{listReleases:{},getCommit:async()=>({data:{sha:base}}),getBranch:async()=>({data:{commit:{sha:candidate}}}),
    compareCommitsWithBasehead:async args=>{assert.equal(args.basehead,`${base}...${candidate}`);return {data:{status:'ahead',total_commits:1,commits:[{sha:merge}]}};}},
    issues:{listComments:{},createComment:async args=>{calls.push(args);own.push({id:40,user:bot,body:args.body});},updateComment:async args=>{calls.push(args);own[0].body=args.body;}}}};
  const args={github,context:{repo:{owner:'owner',repo:'repo'}},core:{info(){}}};
  await release.summarize(args);
  await release.summarize(args);
  assert.equal(calls.length,1);
  assert.equal(calls[0].issue_number,30);
  assert.match(calls[0].body,/Failed\/incomplete/);
  assert.doesNotMatch(calls[0].body,/release-plz .*compatible/);
  target.head.sha=head;
  const original=github.rest.pulls.get;
  github.rest.pulls.get=async()=>({data:{...(await original()).data,head:{...target.head,sha:base}}});
  await release.summarize(args);
  assert.equal(calls.length,1);
});
