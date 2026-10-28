"""Select bounded public evidence after all exactness and counter gates pass."""
import hashlib,json,statistics,sys
from pathlib import Path
r=Path(sys.argv[1]).resolve();out=r/'circle-selected';output=Path(sys.argv[2])
def read(p):return json.loads(p.read_text())
def sha(p):
 with p.open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()
selection=read(r/'selection.json');assert selection['largestPilot']=='circle' and selection['plainWindowBytesEqual']
result={'environment':'a 4-vCPU Zen 5 EPYC KVM guest','access':'local-only sources, archives, copied binaries, window traces, counters, inventories and failed attempts; full warm streams compared live and not stored','selection':selection,'cohorts':{},'pins':{},'phases':{},'matureMutations':{}}
for name in ['stock','legacy','short']:
 v=read(out/'sessions'/name/'accepted.json')
 assert v['forkCompleteWindowBytesEqual'] and v['warmTrace']['warmTraceComparedEveryByte']
 assert [x['pair'] for x in v['pairs']]==[-1,0,1,2,3,4]
 result['cohorts'][name]=v
for arm in ['base','candidate','base-profile','candidate-profile','mutation']:
 p=read(out/arm/'pins.json')
 for name,digest in p['binaries'].items():assert sha(out/arm/'bin'/name)==digest
 for name,digest in p['sources'].items():assert sha(out/arm/'source'/name)==digest
 p['sources']={k:v for k,v in p['sources'].items() if k.startswith('src/') or k in ['build.rs','examples/study_host.rs','examples/study_protocols.rs','examples/perf_baseline.rs']}
 p['manifestSha256']=sha(out/arm/'source/Cargo.toml');p['lockfileSha256']=sha(out/arm/'source/Cargo.lock')
 result['pins'][arm]=p
result['cachePrototype']={'qualification':read(r/'cache-prototype-qualified.json'),'requiredChecks':read(r/'selected/required-checks-pass.json'),'candidatePin':read(r/'selected/candidate/pins.json')}
cache_pin=result['cachePrototype']['candidatePin']
cache_pin['sources']={k:v for k,v in cache_pin['sources'].items() if k.startswith('src/') or k in ['build.rs','examples/study_host.rs','examples/study_protocols.rs','examples/perf_baseline.rs']}
result['requiredChecks']=read(out/'required-checks-pass.json');assert result['requiredChecks']['passed']
result['catalogParity']=read(out/'parity.json');assert len(result['catalogParity'])==27
result['shortMutations']=read(out/'mutation.json');assert all(v['detected'] for v in result['shortMutations'])
result['shortPairs']=read(out/'pairs.json')
for rows in result['shortPairs'].values():assert [x['pair'] for x in rows]==[-1,0,1,2,3,4]
for name in ['mutation-stock','mutation-legacy']:
 v=read(out/'sessions'/name/'accepted.json');assert v['detected'] and v['physicsStateUnchanged'];result['matureMutations'][name]=v
result['streamControls']={}
for n in [0,1]:
 v=read(out/'sessions'/f'stream-control-{n}'/'mismatch.json');assert v['offset']==0;result['streamControls'][str(n)]=v
assert read(out/'sessions/phases/accepted.json')['completeWindowBytesEqualPlainReference']
for arm in ['base-profile','candidate-profile']:
 v=read(out/'sessions/phases'/(arm+'-window.json'));assert v['counts']['calls'][2]==360;result['phases'][arm]=v
result['phaseScope']={2:'complete host advance',3:'world physics',4:'food-interest advance',5:'broadcast inclusive',6:'snake coverage',7:'snake encoding/history',8:'recipient food',9:'recipient prey',10:'whole food compaction',11:'collision bounds/index construction',12:'collision query/exact tests',13:'body-block construction',14:'whole-body broadcast bounds',15:'coverage grid update'}
result['samples']={}
for name in ['sample-mature','sample-2000003']:
 s=r/'sampling-retry/sessions'/name
 assert read(s/'accepted.json')['completeWindowBytesEqualPlainReference']
 result['samples'][name]=read(s/'samples-selected.json')
result['retainedSamplerFailure']={'stage':'first optional sampling window before accepted data','error':'64 MiB perf ring mmap returned MAP_FAILED; errno not retained in original helper','sourceSha256':sha(r/'sample.c'),'librarySha256':sha(r/'libstudy.so'),'warmCpuSeconds':sum(x['cpuSeconds'] for x in read(r/'sessions/sample-mature/ready.json')['ready'].values()),'noSystemOrLimitChange':True}
result['smallSampler']={'sourceSha256':sha(r/'sample-small.c'),'librarySha256':sha(r/'libstudy-small.so'),'ringBytes':512<<10,'periods':[1000003,2000003],'probeHeader':(r/'sampling-small-probe/sample.csv.samples').read_text().splitlines()[0]}
result['traceHashes']={}
for root,label in [(r/'selected/sessions','cache-pilot'),(out/'sessions','selected'),(r/'sampling-retry/sessions','sampling')]:
 for p in root.glob('*/*.trace'):result['traceHashes'][label+'/'+p.parent.name+'/'+p.name]=sha(p)
result['warmCpuSeconds']=0.;result['warmCpuByCohort']={};result['cpuReceipts']={}
for root,label in [(r/'sessions','initial'),(r/'selected/sessions','cache-pilot'),(out/'sessions','selected'),(r/'sampling-retry/sessions','sampling')]:
 for p in root.glob('*/ready.json'):
  v=read(p)
  if v['warmTicks']>=193000:
   seconds=sum(x['cpuSeconds'] for x in v['ready'].values());result['warmCpuSeconds']+=seconds;result['warmCpuByCohort'][label+'/'+p.parent.name]=seconds
 for p in root.glob('*/cpu-receipt.json'):result['cpuReceipts'][label+'/'+p.parent.name]=read(p)
result['reconstruction']=read(out/'reconstruction.json');assert all(v['allRustSourceHashesMatch'] for v in result['reconstruction'])
result['cleanup']=read(r/'target-cleanup.json');assert result['cleanup']['targetRemoved'] and not result['cleanup']['taskWorkers']
output.write_text(json.dumps(result,indent=2)+'\n')
print('SELECTED',output.name,output.stat().st_size,'bytes')
