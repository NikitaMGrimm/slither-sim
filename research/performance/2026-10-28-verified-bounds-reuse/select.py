"""Select cited aggregates; full samples, traces and receipts stay local."""
from pathlib import Path
import hashlib,json,statistics,sys
root=Path(sys.argv[1]).resolve();output=Path(sys.argv[2])
sys.path.insert(0,str(root))
from compare import same_bytes
def read(p):return json.loads(p.read_text())
def digest(value):return hashlib.sha256(json.dumps(value,sort_keys=True,separators=(',',':')).encode()).hexdigest()
def sha(p):
 with p.open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()
def stats(rows):
 assert [x['pair'] for x in rows]==[-1,0,1,2,3,4]
 values=[x['reduction'] for x in rows[1:]]
 return {'pairs':5,'medianReduction':statistics.median(values),'reductionRange':[min(values),max(values)]}
result={'machine':'a 4-vCPU Zen 5 EPYC KVM guest','access':'full samples, traces, sources, copied binaries and receipts local-only','rows':{},'phases':{},'pins':{}}
for case in ['stock','legacy','short']:
 value=read(root/'sessions'/case/'accepted.json');warm=value['warmTrace']
 assert value['forkCompleteWindowBytesEqual'] and warm['warmTraceComparedEveryByte']
 row=stats(value['pairs']);row.update({'warmTicks':warm['warmTicks'],'ticks':warm['windowTicks'],'beforeMedianInstructions':statistics.median(x['base'] for x in value['pairs'][1:]),'afterMedianInstructions':statistics.median(x['candidate'] for x in value['pairs'][1:]),'windowTraceSha256':sha(root/'sessions'/case/'proof-base-0.trace'),'endCensus':read(root/'sessions'/case/'pair-0-candidate.json')['summary']['census']})
 result['rows'][case]=row
 if case=='stock':result['matureCensus']=warm['ready']['base']['summary']['census']
 if case in ['stock','legacy']:
  row['warmBytesPerArm']=next(iter(warm['traceBytes'].values()))
  row['warmTraceSha256']=next(iter(warm['traceSha256'].values()))
result['shortRows']={case:stats(rows) for case,rows in read(root/'pairs.json').items()}
for arm in ['base-profile','candidate-profile']:
 assert same_bytes(root/'sessions/phases'/(arm+'-window.trace'),root/'sessions/stock'/('proof-'+arm.removesuffix('-profile')+'-0.trace'))
 counts=read(root/'sessions/phases'/(arm+'-window.json'))['counts'];assert counts['calls'][2]==360
 result['phases'][arm]={'ticks':360,'instructions':{str(i):counts['instructions'][i] for i in range(2,16)}}
for arm in ['base','candidate','base-profile','candidate-profile','mutation']:
 pin=read(root/arm/'pins.json')
 for name,value in pin['sources'].items():assert sha(root/arm/'source'/name)==value
 for name,value in pin['binaries'].items():assert sha(root/arm/'bin'/name)==value
 result['pins'][arm]={'commit':pin['archiveCommit'],'archiveSha256':pin['archiveSha256'],'toolchain':pin['toolchain'],'lockfileSha256':pin['lockfileSha256'],'rustSources':len(pin['sources']),'rustSourceDigest':digest(pin['sources']),'binaryDigest':digest(pin['binaries'])}
required=read(root/'required-checks-pass.json');assert required['passed']
parity=read(root/'parity.json');assert len(parity)==27
assert all(x['detected'] for x in read(root/'mutation.json'))
for case in ['mutation-stock','mutation-legacy']:
 changed=root/'sessions'/case/'mutation-window.trace'
 reference=root/'sessions'/case.removeprefix('mutation-')/'proof-candidate-0.trace'
 assert not same_bytes(changed,reference)
 def states(path):
  values=[]
  with path.open('rb') as f:
   while header:=f.read(16):
    assert len(header)==16
    tag=int.from_bytes(header[:8],'little');size=int.from_bytes(header[8:],'little');data=f.read(size);assert len(data)==size
    if tag==0:values.append(data)
  assert len(values)==2
  return values
 assert states(changed)==states(reference)
for n in [0,1]:assert read(root/'sessions'/f'stream-control-{n}'/'mismatch.json')['offset']==0
recon=read(root/'reconstruction.json');assert len(recon)==5 and all(x['allRustSourceHashesMatch'] for x in recon)
cleanup=read(root/'target-cleanup.json');assert cleanup['targetRemoved'] and not cleanup['taskWorkers']
result['validation']={'rustGroupPassed':True,'originalTracesEqual':27,'stockAndLegacyMatureBytesEqual':True,'matureMutationPhysicalStatesUnchanged':True,'streamMutationsDetected':2,'reconstructedArms':5,'targetRemoved':True,'checks':{x['label']:{'exit':x['exit'],'expectedFailure':x['expectedFailure']} for x in required['checks']}}
result['cpu']={'matureWarmSeconds':sum(sum(v['cpuSeconds'] for v in read(p)['ready'].values()) for p in (root/'sessions').glob('*/ready.json') if read(p)['warmTicks']>=193000),'hostForkAndControllerSeconds':sum(v['waitedHostAndForkCpuSeconds']+v['controllerCpuSeconds'] for p in (root/'sessions').glob('*/cpu-receipt.json') for v in [read(p)])}
output.write_text(json.dumps(result,indent=2)+'\n');print('SELECTED',output.stat().st_size,'bytes')
