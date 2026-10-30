"""Combine qualified/failed runs without pooling their observations."""
from pathlib import Path
import argparse,json
p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('inputs',nargs='+',type=Path);a=p.parse_args()
inputs=[json.loads(path.read_text()) for path in a.inputs]
assert 1<=len(inputs)<=2 and len({v['runtime'] for v in inputs})==1
assert len({json.dumps(v['pins']['binaries'],sort_keys=True) for v in inputs})==1
runs=[]
for i,value in enumerate(inputs,1):
 result=value['result'].copy();result.pop('phaseMeanCpuMsPerTick',None)
 runs.append({'run':i,'matureScreen':value['matureScreen'],'result':result})
out={'runtime':inputs[0]['runtime'],'build':inputs[0]['pins'],'platform':{'host':'c7a.medium','hostVcpus':1,'generator':'c8a.2xlarge','generatorVcpus':8,'clientWorkers':6,'region':'Frankfurt','market':'Spot','privateSameAvailabilityZone':True},'runs':runs,'twoRunAcceptancePassed':len(runs)==2 and all(v['result']['passes'] for v in runs),'rawAccess':'Local-only, outside repositories; raw logs, binaries and cloud identities are not public downloads.'}
a.output.write_text(json.dumps(out,indent=2)+'\n')
