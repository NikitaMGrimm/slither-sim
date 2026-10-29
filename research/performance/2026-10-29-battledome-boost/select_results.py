"""Reuse #262's canonical selector; verify the high-boost treatment and provenance."""
from pathlib import Path
import hashlib,json,runpy,sys
prior,raw,harness,output,bundle,baseline=map(Path,sys.argv[1:])
arguments=sys.argv[:];sys.argv=[str(prior),str(raw),str(harness),str(output),str(bundle)]
try:runpy.run_path(str(prior),run_name='__main__')
finally:sys.argv=arguments
d=json.loads(output.read_text());pins=json.loads((raw/'pins.json').read_text());base=json.loads(baseline.read_text());summary=json.loads((raw/'run-summary.json').read_text())
assert d['runtime']==base['runtime'] and pins['sharedPolicyUnchanged']
assert [w['bots'] for w in d['windows']]==[80,160,320]
assert all(w['boost']['aliveTickShare']>=.7 for w in d['windows'])
d['boostTreatment']=pins['boostTreatment'];d['sharedPolicySha256']=pins['sharedPolicySha256'];d['sharedPolicyUnchanged']=True
d['baselineSelectedSha256']=hashlib.file_digest(baseline.open('rb'),'sha256').hexdigest()
d['sameHostBinaryAs262']=d['pins']['binaries']['host']==base['pins']['binaries']['host']
d['observedLimits']=summary.get('observedLimits',[])
d['comparisonLimit']='One evolving high-boost world versus the earlier #262 ladder. Changed cohort sizes and world history; no paired causal speed ratio.'
d['analysisSha256']['boostSelect.py']=hashlib.file_digest(Path(__file__).open('rb'),'sha256').hexdigest()
output.write_text(json.dumps(d,indent=2)+'\n',encoding='utf-8',newline='\n')
print('Three high-boost rows pass the 70% treatment screen; shared policy unchanged')
