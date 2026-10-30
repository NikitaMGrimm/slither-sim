"""Synthetic reduction controls; no timing or outside connections."""
from pathlib import Path
import importlib.util,json,sys,copy,math
r=Path(sys.argv[1]).resolve();sys.argv=[sys.argv[0],str(r)]
spec=importlib.util.spec_from_file_location('analysis',r/'analyze.py');a=importlib.util.module_from_spec(spec);spec.loader.exec_module(a)
cs={t:{'unix':t*.007} for t in range(0,2001,100)};bs={}
for t in range(100,2001,100):
 flags=[a.due(k) for k in range(t-99,t+1)]
 phases=[[0]*16 for _ in flags]
 for i,flag in enumerate(flags):phases[i][8]=6_000_000 if flag else 0
 bs[t]={'rows':[[7_000_000 if flag else 1_000_000,2_000_000,0] for flag in flags],'phaseRows':phases,'foodFlags':flags,'stages':[sum(v[i] for v in phases) for i in range(16)]}
rows=a.rows_in(cs,bs,0,14,True);out=a.describe(rows)
assert out['all']['ticks']==2000 and out['all']['worldCpuMsPerTick']['p50']==3 and out['all']['worldCpuMsPerTick']['p99']==9
assert out['slowestOnePercent']['ticks']==20 and out['slowestOnePercent']['maintenanceFraction']==1
assert out['maintenance']['phaseMeanCpuMsPerTick']['broadcast']==6
receipts=[]
for label in ['food-flag','phase-sum','primary-detail-contamination']:
 mutated=copy.deepcopy(bs)
 if label=='food-flag':mutated[100]['foodFlags'][0]=not mutated[100]['foodFlags'][0]
 if label=='phase-sum':mutated[100]['phaseRows'][0][0]+=1
 try:a.rows_in(cs,mutated,0,14,label!='primary-detail-contamination')
 except AssertionError:receipts.append({'control':label,'detected':True})
 else:raise AssertionError('mutation not detected: '+label)
gate=json.loads((r/'isolation-gate.json').read_text());threshold=30*(gate['idleEnvelopeRate']+gate['excessRateAllowance'])+gate['endpointQuantumCount']*gate['counterQuantumSeconds']
assert threshold==gate['exampleThirtySecondAllowanceSeconds']
(r/'analysis-controls.json').write_text(json.dumps({'synthetic':True,'knownPeriodicTailPassed':True,'phaseAndWorldArithmeticPassed':True,'frozenGateFormulaPassed':True,'mutations':receipts},indent=2)+'\n')
print('REDUCTION CONTROLS PASSED')
