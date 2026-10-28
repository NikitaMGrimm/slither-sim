"""Reduce retained current-thread counters; no private paths in selected output."""
import argparse, gzip, json, statistics, hashlib
from pathlib import Path
if not __debug__:raise SystemExit('run without -O: evidence assertions are required')
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('root',type=Path);parser.add_argument('output',type=Path)
parser.add_argument('--partial',action='store_true',help='inspect incomplete collection without publication acceptance')
args=parser.parse_args();root=args.root;out=args.output
stages={9:'broadcast',10:'decode',12:'frame',13:'policy',20:'metadata',21:'hud'}
def profile(folder):
 rows=json.loads((root/folder/'rows.json').read_text())
 assert len(rows)==200 and [r['decision'] for r in rows]==list(range(1000,1200))
 assert all((r['running']/r['enabled'])>=.99 for r in rows)
 n=sum(r['phases']['calls'][12] for r in rows)
 d={name:sum(r['phases']['instructions'][i] for r in rows)/n for i,name in stages.items()}
 d['botDecisions']=n
 d['botStages']=sum(d[name] for name in ['broadcast','decode','frame','policy'])
 d['nextFrame']=sum(r['instructions'] for r in rows)/n
 d['aliveRange']=[min(r['alive'] for r in rows),max(r['alive'] for r in rows)]
 d['pmuMinimumScheduled']=min(r['running']/r['enabled'] for r in rows)
 d['rowsSha256']=hashlib.sha256((root/folder/'rows.json').read_bytes()).hexdigest()
 return d
result={'baseline':'0aba6f97070a06c73b604869761d4c5c14965a78','pilot':{}}
for folder in ['pilot-500','fine-500','candidate-pilot-500','food-profile-500']:
 if (root/folder/'rows.json').exists():
  d=profile(folder)
  rows=json.loads((root/folder/'rows.json').read_text())
  if folder in ['fine-500','food-profile-500']:
   for stage in [22,23]:d[f'diagnosticStage{stage}']=sum(r['phases']['instructions'][stage] for r in rows)/d['botDecisions']
  result['pilot'][folder]=d
for population in [500,2000]:
 key=str(population)
 if not all((root/f'profile-{population}-{arm}/rows.json').exists() for arm in ['base','candidate']):continue
 data={arm:profile(f'profile-{population}-{arm}') for arm in ['base','candidate']}
 assert data['base']['botDecisions']==data['candidate']['botDecisions']
 data['pairs']=[]
 for repeat in range(-1,5):
  pair={'repeat':repeat,'warmup':repeat==-1,'order':['base','candidate'] if repeat%2 else ['candidate','base']}
  for arm in ['base','candidate']:
   path=root/f'bots-{population}-{repeat}-{arm}/rows.json'
   if not path.exists():break
   rows=json.loads(path.read_text());assert len(rows)==200
   assert all(r['running']/r['enabled']>=.99 for r in rows)
   pair[arm]={'instructions':sum(r['instructions'] for r in rows),
              'perBotDecision':sum(r['instructions'] for r in rows)/data[arm]['botDecisions'],
              'pmuMinimumScheduled':min(r['running']/r['enabled'] for r in rows),
              'rowsSha256':hashlib.sha256(path.read_bytes()).hexdigest()}
   endpoint=(path.parent/'world.json').read_bytes()
   assert endpoint==(root/f'verified-trace-{population}-{arm}/world.json').read_bytes()
  if 'base' in pair and 'candidate' in pair:
   pair['ratio']=pair['candidate']['instructions']/pair['base']['instructions'];data['pairs'].append(pair)
 measured=[p for p in data['pairs'] if not p['warmup']]
 if len(measured)==5:
  data['medianRatio']=statistics.median(p['ratio'] for p in measured)
  data['ratioRange']=[min(p['ratio'] for p in measured),max(p['ratio'] for p in measured)]
  data['perBotDecisionMedians']={arm:statistics.median(p[arm]['perBotDecision'] for p in measured) for arm in ['base','candidate']}
 result[key]=data
 if (root/f'verified-trace-{population}.json').exists():data['trace']=json.loads((root/f'verified-trace-{population}.json').read_text())
if (root/'matrix.json').exists():
 result['matrix']=json.loads((root/'matrix.json').read_text())
 assert all(row['base']==row['candidate'] for row in result['matrix'])
if (root/'trace-validation-500.json').exists():result['traceValidation']=json.loads((root/'trace-validation-500.json').read_text())
for arm in ['base','base-profile','candidate','candidate-profile','trace-base','trace-candidate']:
 path=root/arm/'pins.json'
 if path.exists():
  pins=json.loads(path.read_text())
  for name,digest in pins['binaries'].items():assert hashlib.sha256((root/arm/name).read_bytes()).hexdigest()==digest
  result.setdefault('builds',{})[arm]={k:pins[k] for k in ['runtimeCommit','archiveSha256','donorArchiveSha256','driverSha256','toolchain','binaries']}
  result['builds'][arm]['generatedRustSourceHashes']={name:pins['sources'][name] for name in ['src/client/protocol.rs','src/model.rs','src/client/simulation.rs','src/client/simulation/study_bots.rs','src/study_counter.rs','examples/perf_baseline.rs','examples/support/runtime.rs','examples/support/state.rs','build.rs']}
if not args.partial:
 assert 'medianRatio' in result['500'] and len(result['500']['pairs'])==6,'incomplete paired comparison'
 assert len(result['matrix'])==23 and len({r['case'] for r in result['matrix']})==23,'incomplete matrix'
 catalog=json.loads((root/'catalog.json').read_text())
 expected={row['name']:row['traceIterations'] for row in catalog['workloads'] if row['available'] and not row['name'].startswith('host-')}
 assert {row['case']:row['iterations'] for row in result['matrix']}==expected,'matrix coverage differs from catalog'
 assert result['traceValidation']['complete'] and result['traceValidation']['byteIdentical'],'trace not accepted'
 for row in result['matrix']:
  digest=hashlib.sha256()
  with gzip.open(root/f"matrix-{row['case']}-base.jsonl.gz",'rb') as left,gzip.open(root/f"matrix-{row['case']}-candidate.jsonl.gz",'rb') as right:
   while True:
    chunk=left.read(1024*1024)
    assert chunk==right.read(1024*1024),'matrix bytes differ'
    if not chunk:break
    digest.update(chunk)
  assert digest.hexdigest()==row['base'],'retained matrix hash mismatch'
 result['matrixBytesRechecked']=True
 if not result['pilot']:result.pop('pilot')
 result['complete']=True
out.write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result,indent=2))
