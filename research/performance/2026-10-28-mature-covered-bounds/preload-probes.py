"""Small target-only preload validation before any long retry."""
import json,os,subprocess,time
from pathlib import Path
r=Path(__file__).resolve().parent;out=r/'preload-launcher-probes';out.mkdir();os.sched_setaffinity(0,{2});rows=[]
for i in range(12):
 arm='base' if i%2==0 else 'candidate';path=out/f'{i}.json';csv=out/f'{i}.csv'
 command=['taskset','-c','0','env','LD_PRELOAD='+str(r/'libstudy.so'),'STUDY_LIBM='+str(csv),str(r/arm/'bin/study_host'),'0','0','1','0']
 start=time.monotonic()
 with (out/f'{i}.stdout').open('xb') as stdout,(out/f'{i}.stderr').open('xb') as stderr:
  p=subprocess.run(command,input=f'parent {path}\n'.encode(),stdout=stdout,stderr=stderr)
 assert p.returncode==0,(i,p.returncode)
 d=json.loads(path.read_text());assert d['counts']['calls'][2]==1
 header=csv.read_text().splitlines()[0];assert 'probes=33,33,33' in header
 rows.append({'arm':arm,'exit':0,'calibration':header,'wallSeconds':time.monotonic()-start})
 (out/'results.json').write_text(json.dumps(rows,indent=2)+'\n')
print('all 12 target-only preload launch/calibration probes pass',flush=True)
