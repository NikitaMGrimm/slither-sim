"""All original catalog/host traces and stock/13 mutation controls."""
import importlib.util,json,os,sys
from pathlib import Path
root=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('shared',root/'shared/reproduce.py');shared=importlib.util.module_from_spec(spec);spec.loader.exec_module(shared)
os.sched_setaffinity(0,{0})
assert json.loads((root/'required-checks-pass.json').read_text())['passed']
shared.matrix(root)
rows=[]
for case in ['official500','official500-legacy']:
 row=shared.trace(root,'mutation','study_protocols',[case],'trace-mutation-'+case)
 assert not shared.compare(root/('trace-'+case+'-candidate.gz'),root/('trace-mutation-'+case+'.gz'))
 rows.append(dict(case=case,detected=True,**row));(root/'mutation.json').write_text(json.dumps(rows,indent=2)+'\n')
print('CATALOG/HOST PARITY AND MUTATION PASS',flush=True)
